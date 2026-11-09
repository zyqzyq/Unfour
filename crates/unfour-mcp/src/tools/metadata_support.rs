//! Shared schema and argument helpers for local metadata tools.
use super::{
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
    ToolHandler,
};
use crate::command_bus_adapter::CommandBusAdapterError;
use serde_json::{json, Map, Value};

pub(super) fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
pub(super) fn result_schema(properties: Value) -> Value {
    let required: Vec<_> = properties
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    object(properties.clone(), &required)
}
pub(super) fn tool(
    name: &'static str,
    description: &'static str,
    properties: Value,
    required: &[&str],
    output: Value,
    annotations: ToolAnnotations,
    handler: ToolHandler,
) -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name,
            title: name,
            description,
            input_schema: object(properties, required),
            output_schema: output,
            annotations,
        },
        handler,
    }
}
pub(super) fn args(value: Value, fields: &[&str]) -> Result<Map<String, Value>, ToolCallError> {
    let a = object_with_allowed_keys(value, fields)?;
    if a.contains_key("workspaceId") {
        required(&a, "workspaceId")?;
    }
    for key in ["confirm"] {
        if a.get(key).is_some_and(|v| !v.is_boolean()) {
            return Err(invalid(key));
        }
    }
    for key in ["confirmationText", "confirmation_text"] {
        if a.get(key).is_some_and(|v| !v.is_string()) {
            return Err(invalid(key));
        }
    }
    Ok(a)
}
pub(super) fn required(a: &Map<String, Value>, key: &str) -> Result<String, ToolCallError> {
    a.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(key))
}
pub(super) fn optional(a: &Map<String, Value>, key: &str) -> Result<Option<String>, ToolCallError> {
    match a.get(key) {
        None | Some(Value::Null) => Ok(None),
        _ => required(a, key).map(Some),
    }
}
pub(super) fn nullable_required(
    a: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    if !a.contains_key(key) {
        return Err(invalid(key));
    }
    optional(a, key)
}
pub(super) fn invalid(key: &str) -> ToolCallError {
    ToolCallError::InvalidArguments(format!("Invalid or missing argument `{key}`."))
}
pub(super) fn error(e: CommandBusAdapterError) -> ToolCallError {
    ToolCallError::Execution {
        code: e.code,
        message: e.message,
    }
}
pub(super) fn missing() -> ToolCallError {
    ToolCallError::Execution {
        code: "NOT_FOUND",
        message: "The record was not found in this workspace.",
    }
}
pub(super) fn safe_label(s: &str) -> String {
    super::ssh_risk::redact_command_display(s)
}
pub(super) fn confirmation_properties(properties: &mut Value) {
    properties["confirm"] = json!({"type":"boolean"});
    properties["confirmation_text"] = json!({"type":"string"});
    properties["confirmationText"] = json!({"type":"string"});
}
