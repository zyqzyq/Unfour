use std::collections::BTreeMap;

use serde_json::{Map, Value};
use unfour_core::models::{SshTaskDetail, SshTaskStepInput};

use crate::command_bus_adapter::CommandBusAdapterError;

use super::super::ToolCallError;

pub(super) fn parse_steps(value: Option<&Value>) -> Result<Vec<SshTaskStepInput>, ToolCallError> {
    let Some(Value::Array(_)) = value else {
        return Err(ToolCallError::InvalidArguments(
            "argument `steps` must be an array".to_string(),
        ));
    };
    serde_json::from_value(value.cloned().unwrap_or_default()).map_err(|_| {
        ToolCallError::InvalidArguments(
            "argument `steps` contains an invalid SSH task step".to_string(),
        )
    })
}

pub(super) fn parse_inputs(
    value: Option<&Value>,
) -> Result<BTreeMap<String, String>, ToolCallError> {
    match value {
        None => Ok(BTreeMap::new()),
        Some(Value::Object(_)) => serde_json::from_value(value.cloned().unwrap_or_default())
            .map_err(|_| {
                ToolCallError::InvalidArguments(
                    "argument `inputs` must map strings to strings".to_string(),
                )
            }),
        _ => Err(ToolCallError::InvalidArguments(
            "argument `inputs` must be an object".to_string(),
        )),
    }
}

pub(super) fn parse_string_array(
    value: Option<&Value>,
    key: &str,
) -> Result<Vec<String>, ToolCallError> {
    let Some(Value::Array(values)) = value else {
        return Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be an array of non-empty strings"
        )));
    };
    values
        .iter()
        .map(|value| match value {
            Value::String(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
            _ => Err(ToolCallError::InvalidArguments(format!(
                "argument `{key}` must be an array of non-empty strings"
            ))),
        })
        .collect()
}

pub(super) fn required_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a non-empty string"
        ))),
    }
}

pub(super) fn optional_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Ok(Some(value.trim().to_string()))
        }
        _ => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a non-empty string when provided"
        ))),
    }
}

pub(super) fn parse_save_description(
    arguments: &Map<String, Value>,
    existing: Option<&SshTaskDetail>,
) -> Result<String, ToolCallError> {
    match arguments.get("description") {
        None => Ok(existing
            .map(|detail| detail.task.description.clone())
            .unwrap_or_default()),
        Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value.clone()),
        _ => Err(ToolCallError::InvalidArguments(
            "argument `description` must be a string or null when provided".to_string(),
        )),
    }
}

pub(super) fn parse_save_default_connection_id(
    arguments: &Map<String, Value>,
    existing: Option<&SshTaskDetail>,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get("defaultConnectionId") {
        None => Ok(existing
            .and_then(|detail| detail.local_binding.as_ref())
            .and_then(|binding| binding.default_connection_id.clone())),
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Ok(Some(value.trim().to_string()))
        }
        _ => Err(ToolCallError::InvalidArguments(
            "argument `defaultConnectionId` must be a non-empty string or null when provided"
                .to_string(),
        )),
    }
}

pub(super) fn confirmation_keys(id: &'static str) -> Vec<&'static str> {
    vec![
        "workspaceId",
        id,
        "confirm",
        "confirmationText",
        "confirmation_text",
    ]
}

pub(super) fn execution_error(error: CommandBusAdapterError) -> ToolCallError {
    ToolCallError::Execution {
        code: error.code,
        message: error.message,
    }
}
