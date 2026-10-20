//! Schema 1 checks known data locations, not the secrecy of arbitrary text.
//! Validation never rewrites the definition. The engine owns reference scope
//! and graph availability; this module checks literal/reference boundaries.
use serde_json::Value;
use unfour_core::redaction::is_sensitive_flow_name as sensitive;

use crate::SyncError;

#[derive(Clone, Copy)]
pub(super) enum References<'a> {
    Runtime,
    // Input defaults are data, not expressions.
    Literal,
    // Only templates present in the original wire text can supply values.
    Encoded(&'a str),
}

pub(super) fn safe_value(value: &Value, refs: References<'_>) -> Result<(), SyncError> {
    match value {
        Value::Object(map) => {
            if matches!(refs, References::Runtime) && map.contains_key("$ref") {
                return require_reference(value, refs);
            }
            let pair = map.contains_key("value")
                && ["key", "name"].iter().any(|key| {
                    map.get(*key).is_some_and(|v| {
                        v.as_str()
                            .is_none_or(|name| sensitive(name) || dynamic(name, refs))
                    })
                });
            for (key, value) in map {
                local_key(key)?;
                if sensitive(key) || dynamic(key, refs) || (pair && key == "value") {
                    require_reference(value, refs)?;
                }
                // A named API pair can identify a forbidden device-local field.
                if map.contains_key("value") && ["key", "name"].contains(&key.as_str()) {
                    if let Some(name) = value.as_str() {
                        local_key(name)?;
                    }
                }
                safe_value(value, refs)?;
            }
            Ok(())
        }
        Value::Array(items) => items.iter().try_for_each(|v| safe_value(v, refs)),
        Value::String(text) => safe_text(text, refs),
        _ => Ok(()),
    }
}

fn require_reference(value: &Value, refs: References<'_>) -> Result<(), SyncError> {
    let valid = match (value, refs) {
        (Value::Object(map), References::Runtime) if map.len() == 1 => map
            .get("$ref")
            .and_then(Value::as_str)
            .is_some_and(valid_pointer),
        (Value::String(text), References::Runtime) => auth_value(text)
            .strip_prefix("${")
            .and_then(|s| s.strip_suffix('}'))
            .is_some_and(valid_pointer),
        (Value::String(text), References::Encoded(marker)) => auth_value(text) == marker,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(SyncError::FlowUnsafeDefinition)
    }
}

fn auth_value(text: &str) -> &str {
    text.split_once(' ')
        .filter(|(scheme, _)| *scheme == "Bearer")
        .map_or(text, |(_, value)| value)
}

fn dynamic(text: &str, refs: References<'_>) -> bool {
    match refs {
        References::Runtime => text.contains("${"),
        References::Encoded(marker) => text.contains(marker),
        References::Literal => false,
    }
}

fn valid_pointer(value: &str) -> bool {
    let root = value.split('/').nth(1);
    if !value.starts_with('/')
        || !matches!(root, Some("inputs" | "steps" | "probe"))
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, '{' | '}'))
    {
        return false;
    }
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}

fn safe_text(text: &str, refs: References<'_>) -> Result<(), SyncError> {
    local_text(text)?;
    if matches!(refs, References::Runtime) {
        // Mask before decoding JSON/percent escapes. Otherwise a literal
        // "\u0024{...}" or "%24%7B...%7D" could masquerade as a runtime value.
        // Unpredictable per-validation tokens also prevent escaped literals
        // from manufacturing the placeholder after JSON/percent decoding.
        let mut marker = format!("__unfour_runtime_{}__", uuid::Uuid::new_v4());
        while text.contains(&marker) {
            marker.push('_');
        }
        let masked = mask_templates(text, &marker)?;
        inspect_text(&masked, References::Encoded(&marker))
    } else {
        inspect_text(text, refs)
    }
}

fn mask_templates(text: &str, marker: &str) -> Result<String, SyncError> {
    let json = text.trim_start().starts_with(['{', '[']);
    let mut quoted = false;
    let mut escaped = false;
    let mut rest = text;
    let mut output = String::new();
    while let Some(start) = rest.find("${") {
        let prefix = &rest[..start];
        for ch in prefix.chars() {
            if !escaped && ch == '"' {
                quoted = !quoted;
            }
            escaped = !escaped && ch == '\\';
        }
        // Interpolation immediately after a JSON escape is ambiguous without
        // the runtime value. Supply the whole body by reference instead.
        if json && escaped {
            return Err(SyncError::FlowUnsafeDefinition);
        }
        output.push_str(prefix);
        rest = &rest[start + 2..];
        let end = rest.find('}').ok_or(SyncError::FlowUnsafeDefinition)?;
        if !valid_pointer(&rest[..end]) {
            return Err(SyncError::FlowUnsafeDefinition);
        }
        if json && !quoted {
            output.push('"');
            output.push_str(marker);
            output.push('"');
        } else {
            output.push_str(marker);
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn inspect_text(text: &str, refs: References<'_>) -> Result<(), SyncError> {
    let trimmed = text.trim();
    if trimmed.starts_with(['{', '[']) {
        // JSON-looking data that cannot be inspected is explicit refusal, not
        // a bypass into the opaque-text policy. No sanitization or truncation.
        let value: Value =
            serde_json::from_str(trimmed).map_err(|_| SyncError::FlowUnsafeDefinition)?;
        return safe_value(&value, refs);
    }
    if let Ok(url) = url::Url::parse(text) {
        if let Some(password) = url.password() {
            let decoded: String = url::form_urlencoded::parse(format!("v={password}").as_bytes())
                .next()
                .ok_or(SyncError::FlowUnsafeDefinition)?
                .1
                .into_owned();
            require_reference(&Value::String(decoded), refs)?;
        }
    }
    // Also inspect relative/templated URLs whose base is supplied at runtime.
    if let Some((_, query)) = text.split_once('?') {
        encoded_pairs(query.split('#').next().unwrap_or(""), refs)?;
    }
    Ok(())
}

pub(super) fn api_arguments(arguments: &Value) -> Result<(), SyncError> {
    if let Some(body) = arguments.get("body").and_then(Value::as_str) {
        if !body.trim_start().starts_with(['{', '[']) && body.contains('=') {
            let mut marker = format!("__unfour_form_{}__", uuid::Uuid::new_v4());
            while body.contains(&marker) {
                marker.push('_');
            }
            let masked = mask_templates(body, &marker)?;
            encoded_pairs(&masked, References::Encoded(&marker))?;
        }
    }
    Ok(())
}

fn encoded_pairs(text: &str, refs: References<'_>) -> Result<(), SyncError> {
    for (key, value) in url::form_urlencoded::parse(text.as_bytes()) {
        local_key(&key)?;
        let value = Value::String(value.into_owned());
        if sensitive(&key) || dynamic(&key, refs) {
            require_reference(&value, refs)?;
        }
        safe_value(&value, refs)?;
    }
    Ok(())
}

fn local_key(key: &str) -> Result<(), SyncError> {
    let normalized = key.to_ascii_lowercase().replace(['_', '-', ' '], "");
    if [
        "credentialref",
        "secretref",
        "secretstore",
        "keychain",
        "privatekeypath",
        "localpath",
        "runtimevariables",
        "temporaryvariables",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
    {
        return Err(SyncError::FlowUnsafeDefinition);
    }
    local_text(key)
}

fn local_text(value: &str) -> Result<(), SyncError> {
    let lower = value.to_ascii_lowercase();
    if [
        "@unfour-secret:",
        "keychain:",
        "secret-store:",
        "secretstore:",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || lower.match_indices("unfour:").any(|(start, _)| {
            // SecretStore uses unfour:<workspace>:<kind>:<record>. A URL
            // hostname such as https://unfour:443 is ordinary parameter data.
            let mut parts = lower[start + "unfour:".len()..].splitn(3, ':');
            [parts.next(), parts.next(), parts.next()]
                .iter()
                .all(|part| {
                    part.is_some_and(|s| !s.is_empty() && !s.starts_with(char::is_whitespace))
                })
        })
        || lower.match_indices("file:").any(|(start, _)| {
            start == 0
                || !lower[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_alphanumeric())
        })
        || (value.contains("-----BEGIN ") && value.contains("PRIVATE KEY"))
        || value.contains("PuTTY-User-Key-File-")
    {
        return Err(SyncError::FlowUnsafeDefinition);
    }
    Ok(())
}

pub(super) fn reject_local_material(value: &Value) -> Result<(), SyncError> {
    match value {
        Value::Object(map) => map.iter().try_for_each(|(key, value)| {
            local_key(key)?;
            reject_local_material(value)
        }),
        Value::Array(items) => items.iter().try_for_each(reject_local_material),
        Value::String(text) => local_text(text),
        _ => Ok(()),
    }
}
