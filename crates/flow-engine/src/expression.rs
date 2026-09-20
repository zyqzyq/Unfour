use serde_json::Value;
use unfour_core::{models::*, AppError, AppResult};

pub fn resolve(value: &Value, context: &Value) -> AppResult<Value> {
    match value {
        Value::Object(map) if map.contains_key("$ref") => {
            if map.len() != 1 {
                return Err(invalid("FLOW_INVALID_REFERENCE"));
            }
            lookup(
                map["$ref"]
                    .as_str()
                    .ok_or_else(|| invalid("FLOW_INVALID_REFERENCE"))?,
                context,
            )
        }
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| Ok((k.clone(), resolve(v, context)?)))
            .collect(),
        Value::Array(items) => items.iter().map(|v| resolve(v, context)).collect(),
        Value::String(text) => {
            let mut rest = text.as_str();
            let mut output = String::new();
            while let Some(start) = rest.find("${") {
                output.push_str(&rest[..start]);
                rest = &rest[start + 2..];
                let end = rest
                    .find('}')
                    .ok_or_else(|| invalid("FLOW_INVALID_REFERENCE"))?;
                let value = lookup(&rest[..end], context)?;
                output.push_str(&text_value(&value));
                rest = &rest[end + 1..];
            }
            output.push_str(rest);
            Ok(Value::String(output))
        }
        _ => Ok(value.clone()),
    }
}

fn lookup(pointer: &str, context: &Value) -> AppResult<Value> {
    if !pointer.starts_with('/') {
        return Err(invalid("FLOW_INVALID_REFERENCE"));
    }
    context
        .pointer(pointer)
        .cloned()
        .ok_or_else(|| invalid("FLOW_MISSING_REFERENCE"))
}
pub fn text_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
pub fn predicate(predicate: &FlowPredicate, context: &Value) -> AppResult<bool> {
    let left = resolve(&predicate.left, context)?;
    let right = resolve(&predicate.right, context)?;
    Ok(match predicate.op {
        FlowOperator::Eq => left == right,
        FlowOperator::Ne => left != right,
        FlowOperator::In => right
            .as_array()
            .ok_or_else(|| invalid("FLOW_EXPECTED_ARRAY"))?
            .contains(&left),
        op => {
            let a = left
                .as_f64()
                .ok_or_else(|| invalid("FLOW_EXPECTED_NUMBER"))?;
            let b = right
                .as_f64()
                .ok_or_else(|| invalid("FLOW_EXPECTED_NUMBER"))?;
            match op {
                FlowOperator::Gt => a > b,
                FlowOperator::Ge => a >= b,
                FlowOperator::Lt => a < b,
                FlowOperator::Le => a <= b,
                _ => unreachable!(),
            }
        }
    })
}
pub fn invalid(code: &str) -> AppError {
    AppError::Validation(code.into())
}

/// Persistence is a redacted view; execution retains original values in memory.
pub fn redact(value: &mut Value) {
    redact_inner(value, false);
}
pub(crate) fn redact_definition(value: &mut Value) {
    // Names, descriptions and secret flags describe inputs; only their default
    // payloads contain values. Avoid treating e.g. "Deployment password" as a secret.
    let mut inputs = value["inputs"].take();
    redact_inner(value, true);
    if let Some(inputs) = inputs.as_array_mut() {
        for input in inputs {
            if let Some(default) = input.get_mut("default") {
                redact_inner(default, true);
            }
        }
    }
    value["inputs"] = inputs;
}
fn redact_inner(value: &mut Value, preserve_refs: bool) {
    if preserve_refs && is_reference(value) {
        return;
    }
    match value {
        Value::Object(map) => {
            let sensitive_pair = map.get("type").and_then(Value::as_str) == Some("api-key")
                || map
                    .get("key")
                    .and_then(Value::as_str)
                    .is_some_and(sensitive);
            for (key, value) in map {
                if sensitive_pair && key == "key" {
                    continue;
                }
                if (sensitive(key) || (sensitive_pair && key == "value"))
                    && !(preserve_refs && is_reference(value))
                {
                    *value = Value::String("<redacted>".into());
                } else {
                    redact_inner(value, preserve_refs);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|v| redact_inner(v, preserve_refs)),
        Value::String(text) => {
            if let Ok(mut parsed) = serde_json::from_str::<Value>(text) {
                if parsed.is_object() || parsed.is_array() {
                    let original = parsed.clone();
                    redact_inner(&mut parsed, preserve_refs);
                    if parsed != original {
                        *text = parsed.to_string();
                    }
                }
            } else if unfour_core::redaction::is_sensitive_log_line(text) {
                *text = "<redacted>".into();
            }
        }
        _ => {}
    }
}
pub(crate) fn sensitive(key: &str) -> bool {
    unfour_core::redaction::is_sensitive_key(key)
        || matches!(
            key.to_ascii_lowercase().as_str(),
            "passphrase" | "privatekey" | "set-cookie"
        )
}

// A definition can refer to a secret supplied at run time without storing it.
fn is_reference(value: &Value) -> bool {
    if let Some(map) = value.as_object() {
        return map.len() == 1
            && map
                .get("$ref")
                .and_then(Value::as_str)
                .is_some_and(|s| s.starts_with('/'));
    }
    value.as_str().is_some_and(|text| {
        let text = text.strip_prefix("Bearer ").unwrap_or(text);
        text.starts_with("${/") && text.ends_with('}') && text.matches("${").count() == 1
    })
}
