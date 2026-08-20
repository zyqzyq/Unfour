use serde_json::{json, Value};

const POLICY_PAYLOAD_FIELDS: &[&str] = &[
    "blocked",
    "reason",
    "workspaceId",
    "workspaceName",
    "environmentType",
    "mcpPolicy",
    "resolvedPolicy",
    "capability",
    "risk",
];

pub fn structured_tool_result(
    tool: &str,
    environment: &str,
    risk_level: &str,
    duration_ms: u128,
    value: Value,
) -> Value {
    let text = serde_json::to_string(&value).expect("serializing a JSON value cannot fail");

    json!({
        "content": [
            {
                "type": "text",
                "text": text,
            }
        ],
        "structuredContent": value,
        "_meta": call_meta(tool, environment, risk_level, duration_ms),
        "isError": false,
    })
}

pub fn structured_tool_error(
    tool: &str,
    environment: &str,
    risk_level: &str,
    duration_ms: u128,
    code: &str,
    message: &str,
) -> Value {
    error_tool_result(
        tool,
        environment,
        risk_level,
        duration_ms,
        json!({
            "error": {
                "code": code,
                "message": message,
                "details": {}
            }
        }),
    )
}

pub fn structured_policy_error(
    tool: &str,
    environment: &str,
    risk_level: &str,
    duration_ms: u128,
    value: Value,
) -> Value {
    let code = value
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
        .unwrap_or("PERMISSION_DENIED");
    let message = value
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("The MCP tool call was blocked by policy.");

    let mut payload = json!({
        "error": {
            "code": code,
            "message": message,
            "details": value,
        }
    });
    copy_object_fields(&mut payload, &value, POLICY_PAYLOAD_FIELDS);

    error_tool_result(tool, environment, risk_level, duration_ms, payload)
}

pub fn structured_confirmation_required(
    tool: &str,
    environment: &str,
    risk_level: &str,
    duration_ms: u128,
    value: Value,
) -> Value {
    let reason = value
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("This MCP tool call requires confirmation.");
    let confirmation_text = value
        .get("confirmationText")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let confirmation_hint = value
        .get("confirmationHint")
        .and_then(Value::as_str)
        .unwrap_or("Re-run with confirm=true and the exact confirmation_text.");
    let details = value.get("details").cloned().unwrap_or_else(|| json!({}));

    error_tool_result(
        tool,
        environment,
        risk_level,
        duration_ms,
        json!({
            "error": {
                "code": "CONFIRMATION_REQUIRED",
                "message": reason,
                "details": details
            },
            "reason": reason,
            "confirmation_text": confirmation_text,
            "confirmation_hint": confirmation_hint,
            "requires_confirmation": true
        }),
    )
}

fn error_tool_result(
    tool: &str,
    environment: &str,
    risk_level: &str,
    duration_ms: u128,
    payload: Value,
) -> Value {
    let text = serde_json::to_string(&payload).expect("serializing a JSON value cannot fail");

    json!({
        "content": [
            {
                "type": "text",
                "text": text,
            }
        ],
        "_meta": call_meta(tool, environment, risk_level, duration_ms),
        "isError": true,
    })
}

fn call_meta(tool: &str, environment: &str, risk_level: &str, duration_ms: u128) -> Value {
    json!({
        "tool": tool,
        "environment": environment,
        "riskLevel": risk_level,
        "durationMs": duration_ms as u64,
    })
}

fn copy_object_fields(target: &mut Value, source: &Value, keys: &[&str]) {
    let Some(target) = target.as_object_mut() else {
        return;
    };
    let Some(source) = source.as_object() else {
        return;
    };
    for key in keys {
        if let Some(value) = source.get(*key) {
            target
                .entry((*key).to_string())
                .or_insert_with(|| value.clone());
        }
    }
}

#[cfg(test)]
pub(crate) fn content_json(result: &Value) -> Value {
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("MCP result is missing content[0].text: {result}"));
    serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("MCP content[0].text is not JSON ({error}): {text}"))
}

#[cfg(test)]
pub(crate) fn error_json(result: &Value) -> Value {
    assert_eq!(
        result.get("isError"),
        Some(&Value::Bool(true)),
        "expected an error MCP result: {result}"
    );
    assert!(
        result.get("structuredContent").is_none(),
        "error MCP results must omit structuredContent: {result}"
    );
    content_json(result)
}

#[cfg(test)]
pub(crate) fn assert_call_meta(result: &Value, environment: &str, risk_level: &str) {
    let meta = result
        .get("_meta")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("MCP result is missing _meta: {result}"));
    assert_eq!(
        meta.get("environment").and_then(Value::as_str),
        Some(environment)
    );
    assert_eq!(
        meta.get("riskLevel").and_then(Value::as_str),
        Some(risk_level)
    );
    assert!(
        meta.get("durationMs").and_then(Value::as_u64).is_some(),
        "expected _meta.durationMs: {result}"
    );
    assert!(
        meta.get("tool").and_then(Value::as_str).is_some(),
        "expected _meta.tool: {result}"
    );
}

#[cfg(test)]
pub(crate) fn assert_success_payload_is_business_value(result: &Value, expected: &Value) {
    assert_eq!(
        result.get("isError"),
        Some(&Value::Bool(false)),
        "expected a successful MCP result: {result}"
    );
    assert_eq!(result.get("structuredContent"), Some(expected));
    assert_eq!(&content_json(result), expected);
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod response_tests;
