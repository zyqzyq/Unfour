use super::{
    decode_query_component, is_auth_plain_field, is_form_urlencoded, split_url_query,
    userinfo_password_span,
};
use serde_json::Value;
use unfour_core::{
    models::{ApiRequestInput, KeyValue},
    redaction::is_sensitive_key,
    AppResult,
};

#[cfg(test)]
mod tests;

/// Collect credential values from the final resolved, auth-materialized request.
/// This is runtime provenance for diagnostic scrubbing, never persisted metadata.
/// Snapshot/export and normal API history behavior remain independent.
pub fn runtime_request_secret_values(request: &ApiRequestInput) -> AppResult<Vec<String>> {
    let mut secrets = Vec::new();
    let auth: Value = serde_json::from_str(request.auth_json.as_deref().unwrap_or("{}"))?;
    collect_json(&auth, false, true, &mut secrets);
    let custom_key = (auth["type"] == "api-key")
        .then(|| auth["key"].as_str())
        .flatten();
    let query_auth = auth["addTo"] == "query";
    for (rows, query) in [(&request.headers, false), (&request.query, true)] {
        for row in rows.iter().filter(|row| row.enabled) {
            let custom = query == query_auth
                && custom_key.is_some_and(|key| {
                    if query {
                        row.key == key
                    } else {
                        row.key.eq_ignore_ascii_case(key)
                    }
                });
            if sensitive(&row.key) || custom {
                secrets.push(row.value.clone());
                if !query
                    && matches!(
                        row.key.trim().to_ascii_lowercase().as_str(),
                        "authorization" | "proxy-authorization"
                    )
                {
                    if let Some((_, credential)) = row.value.split_once(' ') {
                        secrets.push(credential.to_owned());
                    }
                }
            }
        }
    }
    let password = reqwest::Url::parse(&request.url)
        .ok()
        .and_then(|url| url.password().map(str::to_owned))
        .or_else(|| {
            userinfo_password_span(&request.url)
                .map(|(start, end)| request.url[start..end].to_owned())
        });
    if let Some(raw) = password {
        secrets.push(raw.clone());
        // Unlike form data, '+' in URL userinfo is a literal plus.
        if let Some((_, decoded)) = decode_query_component(&format!(
            "password={}",
            raw.replace('+', "%2B").replace('&', "%26")
        )) {
            secrets.push(decoded);
        }
    }
    if let Some((_, query, _)) = split_url_query(&request.url) {
        collect_pairs(query, custom_key.filter(|_| query_auth), &mut secrets);
    }
    if let Some(body) = &request.body {
        if is_form_urlencoded(&request.body_kind) {
            if let Ok(rows) = serde_json::from_str::<Vec<KeyValue>>(body) {
                secrets.extend(
                    rows.into_iter()
                        .filter(|row| row.enabled && sensitive(&row.key))
                        .map(|row| row.value),
                );
            } else {
                collect_pairs(body, None, &mut secrets);
            }
        } else if let Ok(value) = serde_json::from_str::<Value>(body) {
            collect_json(&value, false, false, &mut secrets);
        }
    }
    secrets.retain(|value| !value.is_empty());
    // Replace longer overlapping credentials first; a prefix must not expose a suffix.
    secrets.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    secrets.dedup();
    Ok(secrets)
}

fn sensitive(key: &str) -> bool {
    // Reuse the snapshot classifier, with equivalent camelCase/compact spellings
    // for runtime JSON fields (without changing persisted API snapshots/history).
    is_sensitive_key(key)
        || matches!(
            key.trim().to_ascii_lowercase().as_str(),
            "apikey" | "accesstoken" | "refreshtoken" | "privatekey" | "licensekey" | "passphrase"
        )
}

fn collect_pairs(pairs: &str, custom_key: Option<&str>, secrets: &mut Vec<String>) {
    for component in pairs.split('&') {
        if let Some((key, value)) = decode_query_component(component) {
            if sensitive(&key) || custom_key == Some(key.as_str()) {
                secrets.push(value);
                if let Some((_, raw)) = component.split_once('=') {
                    secrets.push(raw.to_owned());
                }
            }
        }
    }
}

fn collect_json(value: &Value, secret: bool, auth: bool, secrets: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                collect_json(
                    value,
                    secret
                        || if auth {
                            !is_auth_plain_field(key)
                        } else {
                            sensitive(key)
                        },
                    auth,
                    secrets,
                );
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_json(value, secret, auth, secrets);
            }
        }
        Value::String(value) if secret => secrets.push(value.clone()),
        Value::Number(_) | Value::Bool(_) if secret && !auth => secrets.push(value.to_string()),
        _ => {}
    }
}
