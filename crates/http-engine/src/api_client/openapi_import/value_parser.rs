use serde_json::{Map, Value};
use unfour_core::models::KeyValue;
use unfour_core::redaction::{is_sensitive_key, REDACTED_VALUE};
use unfour_core::{AppError, AppResult};

use super::super::DEFAULT_AUTH_JSON;

pub(super) fn parse_parameters(
    path_item: &Map<String, Value>,
    operation: &Map<String, Value>,
) -> AppResult<(Vec<KeyValue>, Vec<KeyValue>)> {
    let mut headers = Vec::new();
    let mut query = Vec::new();
    let mut cookie_names = Vec::new();
    for owner in [path_item, operation] {
        let parameters = match owner.get("parameters") {
            None => continue,
            Some(Value::Array(parameters)) => parameters,
            Some(_) => {
                return Err(import_validation(
                    "collection import parameters must be an array",
                ));
            }
        };
        for parameter in parameters {
            let parameter = parameter.as_object().ok_or_else(|| {
                import_validation("collection import contains an invalid parameter")
            })?;
            let name = required_string(
                parameter,
                "name",
                "collection import parameter name cannot be empty",
            )?;
            let location = required_string(
                parameter,
                "in",
                "collection import parameter location cannot be empty",
            )?;
            let redacted = parameter
                .get("x-unfour-redacted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || import_sensitive_key(&name);
            let item = KeyValue {
                key: name.clone(),
                value: if redacted {
                    String::new()
                } else {
                    parameter_value(parameter.get("example").or_else(|| {
                        parameter
                            .get("schema")
                            .and_then(|schema| schema.get("example"))
                    }))
                },
                enabled: !redacted,
            };
            match location.as_str() {
                "header" => upsert_item(&mut headers, item),
                "query" => upsert_item(&mut query, item),
                "cookie" => {
                    cookie_names.retain(|existing| existing != &name);
                    cookie_names.push(name);
                }
                "path" => {}
                _ => {
                    return Err(import_validation(
                        "collection import contains an unsupported parameter location",
                    ));
                }
            }
        }
    }
    if !cookie_names.is_empty() {
        headers.push(KeyValue {
            key: "Cookie".to_string(),
            value: cookie_names
                .into_iter()
                .map(|name| format!("{name}="))
                .collect::<Vec<_>>()
                .join("; "),
            enabled: false,
        });
    }
    Ok((headers, query))
}

pub(super) fn parse_request_body(
    operation: &Map<String, Value>,
) -> AppResult<(Option<String>, String, Option<String>)> {
    let Some(request_body) = operation.get("requestBody") else {
        return Ok((None, "none".to_string(), None));
    };
    let request_body = request_body
        .as_object()
        .ok_or_else(|| import_validation("collection import request body is invalid"))?;
    let content = object_field(request_body, "content")?;
    let (content_type, media) = content
        .iter()
        .next()
        .ok_or_else(|| import_validation("collection import request body has no content"))?;
    let media = media
        .as_object()
        .ok_or_else(|| import_validation("collection import request body content is invalid"))?;
    let body_kind = non_empty_string(request_body.get("x-unfour-body-kind"))
        .map(str::to_string)
        .unwrap_or_else(|| infer_body_kind(content_type).to_string());
    let example = media.get("example").or_else(|| {
        media
            .get("examples")
            .and_then(Value::as_object)
            .and_then(|examples| examples.values().next())
            .and_then(|example| example.get("value").or(Some(example)))
    });
    let body = if is_form_body_kind(&body_kind) {
        let fields = example
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|object| object.iter())
            .map(|(key, value)| {
                let redacted = import_sensitive_key(key) || value.as_str() == Some(REDACTED_VALUE);
                KeyValue {
                    key: key.clone(),
                    value: if redacted {
                        String::new()
                    } else {
                        parameter_value(Some(value))
                    },
                    enabled: !redacted,
                }
            })
            .collect::<Vec<_>>();
        serde_json::to_string(&fields)?
    } else if let Some(value) = example.and_then(Value::as_str) {
        value.to_string()
    } else if let Some(example) = example {
        serde_json::to_string(example)?
    } else if content_type.to_ascii_lowercase().contains("json") {
        "{}".to_string()
    } else {
        String::new()
    };
    Ok((Some(body), body_kind, Some(content_type.clone())))
}

pub(super) fn parse_auth_json(
    root: &Map<String, Value>,
    operation: &Map<String, Value>,
) -> AppResult<String> {
    let security = match operation.get("security") {
        Some(security) => Some(security),
        None => root.get("security"),
    };
    let Some(security) = security else {
        return Ok(DEFAULT_AUTH_JSON.to_string());
    };
    let requirements = security
        .as_array()
        .ok_or_else(|| import_validation("collection import security must be an array"))?;
    let Some(requirement) = requirements.first() else {
        return Ok(DEFAULT_AUTH_JSON.to_string());
    };
    let scheme_name = requirement
        .as_object()
        .and_then(|requirement| requirement.keys().next())
        .ok_or_else(|| import_validation("collection import security requirement is invalid"))?;
    let Some(schemes) = root
        .get("components")
        .and_then(Value::as_object)
        .and_then(|components| components.get("securitySchemes"))
        .and_then(Value::as_object)
    else {
        return Ok(DEFAULT_AUTH_JSON.to_string());
    };
    let Some(scheme) = schemes.get(scheme_name).and_then(Value::as_object) else {
        return Ok(DEFAULT_AUTH_JSON.to_string());
    };
    let auth = match non_empty_string(scheme.get("type")) {
        Some("http") if non_empty_string(scheme.get("scheme")) == Some("bearer") => {
            serde_json::json!({ "type": "bearer", "token": "" })
        }
        Some("http") if non_empty_string(scheme.get("scheme")) == Some("basic") => {
            serde_json::json!({ "type": "basic", "username": "", "password": "" })
        }
        Some("apiKey") => serde_json::json!({
            "type": "api-key",
            "addTo": non_empty_string(scheme.get("in")).unwrap_or("header"),
            "key": non_empty_string(scheme.get("name")).unwrap_or("X-API-Key"),
            "value": ""
        }),
        _ => {
            return Ok(DEFAULT_AUTH_JSON.to_string());
        }
    };
    Ok(serde_json::to_string(&auth)?)
}

fn object_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> AppResult<&'a Map<String, Value>> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| import_validation(format!("collection import {key} must be an object")))
}

fn required_string(object: &Map<String, Value>, key: &str, error: &str) -> AppResult<String> {
    non_empty_string(object.get(key))
        .map(str::to_string)
        .ok_or_else(|| import_validation(error))
}

fn non_empty_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parameter_value(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(value) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn upsert_item(items: &mut Vec<KeyValue>, item: KeyValue) {
    if let Some(existing) = items
        .iter_mut()
        .find(|existing| existing.key.eq_ignore_ascii_case(&item.key))
    {
        *existing = item;
    } else {
        items.push(item);
    }
}

fn infer_body_kind(content_type: &str) -> &'static str {
    let content_type = content_type.to_ascii_lowercase();
    if content_type.contains("application/x-www-form-urlencoded") {
        "form-urlencoded"
    } else if content_type.starts_with("text/") {
        "text"
    } else {
        "json"
    }
}

fn import_sensitive_key(value: &str) -> bool {
    if is_sensitive_key(value) {
        return true;
    }
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    normalized.contains("password")
        || normalized.contains("passphrase")
        || normalized.contains("secret")
        || normalized.ends_with("token")
        || normalized.ends_with("api_key")
}

fn is_form_body_kind(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "form-urlencoded" | "x-www-form-urlencoded" | "urlencoded"
    )
}

fn import_validation(message: impl Into<String>) -> AppError {
    AppError::Validation(message.into())
}
