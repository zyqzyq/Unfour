use super::model::*;
use super::source::export_sensitive_key;
use std::collections::{BTreeMap, HashSet};
use unfour_core::models::{ApiHistoryDetail, ApiSavedRequest, KeyValue};
use unfour_core::redaction::{redact_sensitive_lines, REDACTED_VALUE};

pub(super) fn build_parameters(
    prepared: &PreparedRequest<'_>,
    headers: &[KeyValue],
    query: &[KeyValue],
) -> Vec<OpenApiParameter> {
    let mut parameters = Vec::new();
    let mut seen = HashSet::new();
    for name in &prepared.path_parameters {
        push_parameter(
            &mut parameters,
            &mut seen,
            name,
            "path",
            true,
            &format!("{{{{{name}}}}}"),
        );
    }
    for item in query
        .iter()
        .filter(|item| item.enabled && !item.key.trim().is_empty())
    {
        push_parameter(
            &mut parameters,
            &mut seen,
            item.key.trim(),
            "query",
            false,
            &item.value,
        );
    }
    for header in headers
        .iter()
        .filter(|item| item.enabled && !item.key.trim().is_empty())
    {
        if header.key.eq_ignore_ascii_case("content-type") {
            continue;
        }
        if header.key.eq_ignore_ascii_case("cookie") {
            for cookie in header.value.split(';') {
                let Some((name, value)) = cookie.trim().split_once('=') else {
                    continue;
                };
                push_parameter(
                    &mut parameters,
                    &mut seen,
                    name.trim(),
                    "cookie",
                    false,
                    value.trim(),
                );
            }
            continue;
        }
        push_parameter(
            &mut parameters,
            &mut seen,
            header.key.trim(),
            "header",
            false,
            &header.value,
        );
    }
    parameters
}

fn push_parameter(
    parameters: &mut Vec<OpenApiParameter>,
    seen: &mut HashSet<(String, String)>,
    name: &str,
    location: &str,
    required: bool,
    value: &str,
) {
    if name.is_empty() || !seen.insert((location.to_string(), name.to_ascii_lowercase())) {
        return;
    }
    let redacted = export_sensitive_key(name);
    let example = if redacted {
        serde_json::json!(REDACTED_VALUE)
    } else {
        value_to_example(value)
    };
    let extensions = redacted
        .then(|| BTreeMap::from([("x-unfour-redacted".to_string(), serde_json::json!(true))]))
        .unwrap_or_default();
    parameters.push(OpenApiParameter {
        name: name.to_string(),
        location: location.to_string(),
        required,
        schema: schema_for_value(&example),
        example: Some(example),
        extensions,
    });
}

pub(super) fn append_url_query(url: &str, query: &mut Vec<KeyValue>) {
    let Some(raw_query) = url
        .split('#')
        .next()
        .and_then(|value| value.split_once('?').map(|(_, query)| query))
    else {
        return;
    };
    let existing = query
        .iter()
        .map(|item| item.key.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    for pair in raw_query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if !key.is_empty() && !existing.contains(&key.to_ascii_lowercase()) {
            query.push(KeyValue {
                key: key.to_string(),
                value: value.to_string(),
                enabled: true,
            });
        }
    }
}

pub(super) fn build_request_body(
    request: &ApiSavedRequest,
    headers: &[KeyValue],
) -> Option<OpenApiRequestBody> {
    let body = request.body.as_deref()?.trim();
    if body.is_empty() || request.body_kind.eq_ignore_ascii_case("none") {
        return None;
    }
    let content_type = headers
        .iter()
        .find(|header| header.enabled && header.key.eq_ignore_ascii_case("content-type"))
        .map(|header| {
            header
                .value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| match request.body_kind.to_ascii_lowercase().as_str() {
            "form-urlencoded" | "x-www-form-urlencoded" | "urlencoded" => {
                "application/x-www-form-urlencoded".to_string()
            }
            "text" | "raw:text" => "text/plain".to_string(),
            _ => "application/json".to_string(),
        });
    let (example, schema) = request_body_example(body, &content_type, &request.body_kind);
    Some(OpenApiRequestBody {
        content: BTreeMap::from([(
            content_type,
            OpenApiMediaType {
                schema,
                example: Some(example),
            },
        )]),
        extensions: BTreeMap::from([(
            "x-unfour-body-kind".to_string(),
            serde_json::json!(request.body_kind),
        )]),
    })
}

fn request_body_example(
    body: &str,
    content_type: &str,
    body_kind: &str,
) -> (serde_json::Value, OpenApiSchema) {
    if content_type.contains("json") || body_kind.eq_ignore_ascii_case("json") {
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(body) {
            redact_export_value(&mut value);
            let schema = schema_for_value(&value);
            return (value, schema);
        }
    }
    if content_type == "application/x-www-form-urlencoded" {
        if let Ok(items) = serde_json::from_str::<Vec<KeyValue>>(body) {
            let value = serde_json::Value::Object(
                items
                    .into_iter()
                    .filter(|item| item.enabled && !item.key.trim().is_empty())
                    .map(|item| {
                        let value = if export_sensitive_key(&item.key) {
                            serde_json::json!(REDACTED_VALUE)
                        } else {
                            value_to_example(&item.value)
                        };
                        (item.key, value)
                    })
                    .collect(),
            );
            let schema = schema_for_value(&value);
            return (value, schema);
        }
    }
    let (redacted, _) = redact_sensitive_lines(body);
    let value = serde_json::json!(redacted);
    let schema = schema_for_value(&value);
    (value, schema)
}

pub(super) fn build_responses(
    request: &ApiSavedRequest,
    histories: &[ApiHistoryDetail],
) -> BTreeMap<String, OpenApiResponse> {
    let mut responses = BTreeMap::new();
    for history in histories
        .iter()
        .filter(|history| history_matches_request(history, request))
    {
        let Some(status) = history.status.filter(|status| (100..=599).contains(status)) else {
            continue;
        };
        responses
            .entry(status.to_string())
            .or_insert_with(|| response_from_history(history, status));
    }
    if responses.is_empty() {
        responses.insert(
            "default".to_string(),
            OpenApiResponse {
                description: "No saved response is available for this request.".to_string(),
                headers: BTreeMap::new(),
                content: BTreeMap::new(),
                extensions: BTreeMap::new(),
            },
        );
    }
    responses
}

fn history_matches_request(history: &ApiHistoryDetail, request: &ApiSavedRequest) -> bool {
    history.workspace_id == request.workspace_id
        && history.name.as_deref() == Some(request.name.as_str())
        && history.method.eq_ignore_ascii_case(&request.method)
        && history.url == request.url
        && json_text_equal(&history.request_headers_json, &request.headers_json)
        && json_text_equal(&history.request_query_json, &request.query_json)
        && history.request_body == request.body
}

fn json_text_equal(left: &str, right: &str) -> bool {
    match (
        serde_json::from_str::<serde_json::Value>(left),
        serde_json::from_str::<serde_json::Value>(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn response_from_history(history: &ApiHistoryDetail, status: i64) -> OpenApiResponse {
    let (headers, raw_headers) = parse_key_values(&history.response_headers_json);
    let content_type = headers
        .iter()
        .find(|header| header.enabled && header.key.eq_ignore_ascii_case("content-type"))
        .map(|header| {
            header
                .value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if history
                .response_body_preview
                .as_deref()
                .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
                .is_some()
            {
                "application/json".to_string()
            } else {
                "text/plain".to_string()
            }
        });
    let response_headers = headers
        .iter()
        .filter(|header| header.enabled && !header.key.eq_ignore_ascii_case("content-type"))
        .map(|header| {
            let redacted = export_sensitive_key(&header.key);
            let example = if redacted {
                serde_json::json!(REDACTED_VALUE)
            } else {
                value_to_example(&header.value)
            };
            (
                header.key.clone(),
                OpenApiHeader {
                    schema: schema_for_value(&example),
                    example: Some(example),
                    extensions: redacted
                        .then(|| {
                            BTreeMap::from([(
                                "x-unfour-redacted".to_string(),
                                serde_json::json!(true),
                            )])
                        })
                        .unwrap_or_default(),
                },
            )
        })
        .collect();
    let content = history
        .response_body_preview
        .as_deref()
        .filter(|body| !body.is_empty())
        .map(|body| {
            let (example, schema) = request_body_example(body, &content_type, &content_type);
            BTreeMap::from([(
                content_type,
                OpenApiMediaType {
                    schema,
                    example: Some(example),
                },
            )])
        })
        .unwrap_or_default();
    let description = u16::try_from(status)
        .ok()
        .and_then(|status| reqwest::StatusCode::from_u16(status).ok())
        .and_then(|status| status.canonical_reason().map(str::to_string))
        .unwrap_or_else(|| format!("HTTP {status}"));
    let mut extensions = BTreeMap::from([
        (
            "x-unfour-history-id".to_string(),
            serde_json::json!(history.id),
        ),
        (
            "x-unfour-response-body-preview".to_string(),
            serde_json::json!(true),
        ),
    ]);
    if let Some(raw) = raw_headers {
        extensions.insert(
            "x-unfour-raw-headers-json".to_string(),
            serde_json::json!(raw),
        );
    }
    OpenApiResponse {
        description,
        headers: response_headers,
        content,
        extensions,
    }
}

pub(super) fn parse_key_values(value: &str) -> (Vec<KeyValue>, Option<String>) {
    match serde_json::from_str::<Vec<KeyValue>>(value) {
        Ok(items) => (items, None),
        Err(_) => (Vec::new(), Some(value.to_string())),
    }
}

fn value_to_example(value: &str) -> serde_json::Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        serde_json::json!(true)
    } else if trimmed.eq_ignore_ascii_case("false") {
        serde_json::json!(false)
    } else if let Ok(value) = trimmed.parse::<i64>() {
        serde_json::json!(value)
    } else if let Ok(value) = trimmed.parse::<f64>() {
        serde_json::json!(value)
    } else {
        serde_json::json!(value)
    }
}

fn schema_for_value(value: &serde_json::Value) -> OpenApiSchema {
    match value {
        serde_json::Value::Null => OpenApiSchema {
            schema_type: Some("null".to_string()),
            ..OpenApiSchema::default()
        },
        serde_json::Value::Bool(_) => OpenApiSchema {
            schema_type: Some("boolean".to_string()),
            ..OpenApiSchema::default()
        },
        serde_json::Value::Number(number) => OpenApiSchema {
            schema_type: Some(if number.is_i64() || number.is_u64() {
                "integer".to_string()
            } else {
                "number".to_string()
            }),
            ..OpenApiSchema::default()
        },
        serde_json::Value::String(_) => OpenApiSchema {
            schema_type: Some("string".to_string()),
            ..OpenApiSchema::default()
        },
        serde_json::Value::Array(items) => OpenApiSchema {
            schema_type: Some("array".to_string()),
            items: items.first().map(|item| Box::new(schema_for_value(item))),
            ..OpenApiSchema::default()
        },
        serde_json::Value::Object(properties) => OpenApiSchema {
            schema_type: Some("object".to_string()),
            properties: properties
                .iter()
                .map(|(key, value)| (key.clone(), schema_for_value(value)))
                .collect(),
            ..OpenApiSchema::default()
        },
    }
}

fn redact_export_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(properties) => {
            for (key, value) in properties {
                if export_sensitive_key(key) {
                    *value = serde_json::json!(REDACTED_VALUE);
                } else {
                    redact_export_value(value);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_export_value(item);
            }
        }
        _ => {}
    }
}
