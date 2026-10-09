use super::*;

pub(super) fn api_slot(request: &ApiRequestRecord, field: &str, pointer: &str) -> bool {
    let json = match field {
        "url" => {
            return pointer.is_empty()
                && (request.url.contains("<redacted>")
                    || request.url.to_ascii_lowercase().contains("%3credacted%3e"))
        }
        "auth" => serde_json::from_str::<Value>(&request.auth_json).ok(),
        "headers" => serde_json::to_value(&request.headers).ok(),
        "query" => serde_json::to_value(&request.query).ok(),
        _ => None,
    };
    json.and_then(|v| v.pointer(pointer).map(|leaf| leaf == "<redacted>"))
        .unwrap_or(false)
}
pub(super) fn mark_api_slot_ready(
    request: &mut ApiRequestRecord,
    field: &str,
    pointer: &str,
) -> AppResult<()> {
    if field == "url" {
        request.url = "https://restored.invalid".into();
        return Ok(());
    }
    let mut value = match field {
        "auth" => serde_json::from_str::<Value>(&request.auth_json)?,
        "headers" => serde_json::to_value(&request.headers)?,
        "query" => serde_json::to_value(&request.query)?,
        _ => return Err(invalid()),
    };
    *value.pointer_mut(pointer).ok_or_else(invalid)? = json!("restored");
    match field {
        "auth" => request.auth_json = serde_json::to_string(&value)?,
        "headers" => request.headers = serde_json::from_value(value)?,
        "query" => request.query = serde_json::from_value(value)?,
        _ => unreachable!(),
    }
    Ok(())
}
pub(super) fn collect_api_secrets(
    id: &str,
    field: &str,
    pointer: &str,
    original: &Value,
    safe: &Value,
    out: &mut Vec<BackupSecret>,
) {
    match original {
        Value::String(value)
            if safe == "<redacted>"
                && !value.is_empty()
                && value != "<redacted>"
                && (!value.contains("{{") || value.starts_with("{{@unfour-secret:")) =>
        {
            out.push(BackupSecret {
                kind: "api-secret".into(),
                targets: vec![id.into()],
                field: field.into(),
                pointer: pointer.into(),
                value: value.clone(),
            });
        }
        Value::Object(map) => {
            for (key, value) in map {
                collect_api_secrets(
                    id,
                    field,
                    &format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1")),
                    value,
                    &safe[key],
                    out,
                );
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                collect_api_secrets(
                    id,
                    field,
                    &format!("{pointer}/{index}"),
                    value,
                    &safe[index],
                    out,
                );
            }
        }
        _ => {}
    }
}
