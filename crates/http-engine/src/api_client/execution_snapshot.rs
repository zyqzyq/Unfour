use super::*;
use unfour_core::models::ApiExecutedRequestSnapshot;
use unfour_core::redaction::REDACTED_VALUE;

/// Use the built request, not the input: reqwest adds auth/content-type and
/// normalizes URLs. Transport headers and later redirect hops are not captured.
pub(super) fn executed_input(
    input: &ApiRequestInput,
    request: &reqwest::Request,
) -> ApiRequestInput {
    let mut executed = input.clone();
    executed.method = request.method().to_string();
    let mut url = request.url().clone();
    executed.query = url
        .query_pairs()
        .map(|(key, value)| KeyValue {
            key: key.into_owned(),
            value: value.into_owned(),
            enabled: true,
        })
        .collect();
    url.set_query(None);
    url.set_fragment(None);
    executed.url = url.to_string();
    executed.headers = request
        .headers()
        .iter()
        .map(|(key, value)| KeyValue {
            key: key.to_string(),
            value: value.to_str().unwrap_or(REDACTED_VALUE).to_owned(),
            enabled: true,
        })
        .collect();
    if request.body().is_none() {
        executed.body = None;
        executed.body_kind = "none".into();
    } else if input.body_kind == unfour_core::models::MULTIPART_BODY_KIND {
        // Streaming file bytes and local paths are never a display/history body.
        // Keep the portable multipart definition for history compatibility;
        // snapshot() hides it because it is not the streaming wire body.
    } else if input.body_kind != "form-urlencoded"
        && input
            .body
            .as_deref()
            .is_some_and(|body| serde_json::from_str::<serde_json::Value>(body).is_err())
    {
        // Opaque text/XML/binary cannot be structurally redacted reliably.
        executed.body = Some(REDACTED_VALUE.into());
    }
    executed
}

pub(super) fn snapshot(
    input: &ApiRequestInput,
    executed_url: &reqwest::Url,
    secrets: &[String],
) -> AppResult<ApiExecutedRequestSnapshot> {
    let safe = super::history_redaction::sanitize_with_secrets(input, &[], "", secrets)?;
    let query: Vec<KeyValue> = serde_json::from_str(&safe.query)?;
    // Build the complete URL before scrubbing, so hidden path/host values cannot
    // make the sanitized URL unparsable. Query is also available as decoded rows.
    let mut complete = input.clone();
    let mut url = executed_url.clone();
    url.set_fragment(None);
    complete.url = url.to_string();
    let url = super::history_redaction::sanitize_with_secrets(&complete, &[], "", secrets)?.url;
    Ok(ApiExecutedRequestSnapshot {
        method: input.method.clone(),
        url,
        headers: serde_json::from_str(&safe.headers)?,
        query,
        body: if input.body_kind == unfour_core::models::MULTIPART_BODY_KIND {
            Some(REDACTED_VALUE.into())
        } else {
            safe.body
        },
        body_kind: input.body_kind.clone(),
    })
}

/// Defense in depth for all callers, including saved/MCP/Flow execution.
/// A redacted export or history is not an executable credential source.
pub(super) fn validate_replay_values(input: &ApiRequestInput) -> AppResult<()> {
    let encoded = serde_json::json!({
        "url": input.url, "headers": input.headers, "query": input.query,
        "body": input.body, "auth": input.auth_json,
    })
    .to_string();
    if contains_marker(&encoded) {
        return Err(AppError::Validation("API_REDACTED_REQUEST".into()));
    }
    Ok(())
}

fn contains_marker(value: &str) -> bool {
    let mut value = value.to_ascii_lowercase();
    while value.contains(r"\\") {
        value = value.replace(r"\\", r"\");
    }
    value.contains(REDACTED_VALUE)
        || value.contains("[mask ")
        || value.contains("%5bmask")
        || value.contains("%3credacted%3e")
        || value.contains("%253credacted%253e")
        || value.contains(r"\u003credacted\u003e")
}

/// Scripts can derive signatures under arbitrary names; value provenance is
/// lost after JS. Hide changed fields conservatively rather than guess their
/// safety from names. These values live only for this execution.
pub fn script_mutation_secret_values(
    original: &ApiRequestInput,
    mutated: &ApiRequestInput,
    resolved: &ApiRequestInput,
) -> Vec<String> {
    let mut secrets = Vec::new();
    if original.url != mutated.url {
        secrets.push(resolved.url.clone());
        if let Ok(mut url) = reqwest::Url::parse(&resolved.url) {
            if url.path() != "/" {
                secrets.push(url.path().to_owned());
            }
            secrets.extend(url.query_pairs().map(|(_, value)| value.into_owned()));
            url.set_query(None);
            url.set_fragment(None);
            secrets.push(url.to_string());
        }
    }
    if original.body != mutated.body {
        secrets.extend(resolved.body.iter().cloned());
        if let Some(value) = resolved
            .body
            .as_deref()
            .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        {
            collect_script_body_values(&value, &mut secrets);
        }
    }
    for (index, header) in mutated.headers.iter().enumerate() {
        if header.enabled && !original.headers.contains(header) {
            if let Some(resolved) = resolved.headers.get(index) {
                secrets.push(resolved.value.clone());
            }
        }
    }
    secrets.retain(|value| !value.is_empty());
    secrets
}

fn collect_script_body_values(value: &serde_json::Value, secrets: &mut Vec<String>) {
    match value {
        serde_json::Value::String(value) => secrets.push(value.clone()),
        serde_json::Value::Object(fields) => {
            for value in fields.values() {
                collect_script_body_values(value, secrets);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_script_body_values(value, secrets);
            }
        }
        _ => {}
    }
}
