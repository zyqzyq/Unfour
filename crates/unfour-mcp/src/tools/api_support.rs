use super::*;

pub(super) fn resolve_workspace_id(
    command_bus: &dyn CommandBusAdapter,
    arguments: &Map<String, Value>,
) -> Result<String, ToolCallError> {
    match parse_optional_string(arguments, "workspaceId")? {
        Some(id) => Ok(id),
        None => {
            let ws_result = command_bus
                .execute_read(ReadCommand::CurrentWorkspace)
                .map_err(|e| ToolCallError::Execution {
                    code: e.code,
                    message: e.message,
                })?;
            let ReadCommandResult::CurrentWorkspace(ws) = ws_result else {
                return Err(unexpected_result());
            };
            Ok(ws.workspace_id)
        }
    }
}

pub(super) fn get_saved_request(
    command_bus: &dyn CommandBusAdapter,
    request_id: &str,
) -> Result<ApiSavedRequest, ToolCallError> {
    let result = command_bus
        .execute_read(ReadCommand::ApiGetRequest {
            request_id: request_id.to_string(),
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let ReadCommandResult::ApiRequest(detail) = result else {
        return Err(unexpected_result());
    };
    Ok(detail.request)
}

pub(super) fn parse_parent_folder_id(
    arguments: &Map<String, Value>,
) -> Result<Option<String>, ToolCallError> {
    parse_optional_string(arguments, "parentFolderId")
        .and_then(|value| Ok(value.or(parse_optional_string(arguments, "parentId")?)))
}

pub(super) fn parse_auth_json(
    arguments: &Map<String, Value>,
) -> Result<Option<String>, ToolCallError> {
    if let Some(raw) = parse_optional_string(arguments, "authJson")? {
        return Ok(Some(raw));
    }
    match arguments.get("auth") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            serde_json::to_string(value)
                .map(Some)
                .map_err(|_| ToolCallError::Execution {
                    code: "TOOL_RESULT_SERIALIZATION_FAILED",
                    message: "The tool result could not be serialized.",
                })
        }
    }
}

pub(super) fn parse_optional_body(
    arguments: &Map<String, Value>,
    existing: Option<String>,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get("body") {
        None => Ok(existing),
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `body` must be a string or null".to_string(),
        )),
    }
}

pub(super) fn parse_key_values(value: Option<&Value>) -> Result<Vec<KeyValue>, ToolCallError> {
    match value {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Object(object)) => Ok(object
            .iter()
            .map(|(key, value)| KeyValue {
                key: key.clone(),
                value: value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string()),
                enabled: true,
            })
            .collect()),
        Some(Value::Array(_)) => serde_json::from_value::<Vec<KeyValue>>(
            value.cloned().unwrap_or_else(|| Value::Array(vec![])),
        )
        .map_err(|_| {
            ToolCallError::InvalidArguments(
                "headers/query arrays must contain { key, value, enabled } objects".to_string(),
            )
        }),
        Some(_) => Err(ToolCallError::InvalidArguments(
            "headers/query must be an object or array".to_string(),
        )),
    }
}

pub(super) fn safe_request_summary(saved: &ApiSavedRequest) -> Value {
    let header_count = serde_json::from_str::<Vec<Value>>(&saved.headers_json)
        .map(|headers| headers.len())
        .unwrap_or(0);
    json!({
        "id": saved.id,
        "workspaceId": saved.workspace_id,
        "collectionId": saved.collection_id,
        "parentId": saved.parent_folder_id,
        "name": saved.name,
        "method": saved.method,
        "url": redact_url_query(&saved.url),
        "hasBody": saved.body.as_ref().is_some_and(|body| !body.is_empty()),
        "headerCount": header_count,
        "updatedAt": saved.updated_at
    })
}

pub(super) fn parse_optional_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a string",
            key
        ))),
    }
}

pub(super) fn parse_required_string(
    arguments: &Map<String, Value>,
    key: &str,
    tool_name: &str,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        Some(Value::String(_)) => Err(ToolCallError::InvalidArguments(format!(
            "{} argument `{}` cannot be empty",
            tool_name, key
        ))),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "{} requires argument `{}`",
            tool_name, key
        ))),
    }
}

pub(super) fn parse_optional_bool(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<bool>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a boolean",
            key
        ))),
    }
}

pub(super) const DEFAULT_HISTORY_LIMIT: i64 = 50;
pub(super) const MAX_HISTORY_LIMIT: i64 = 200;

pub(super) fn parse_optional_history_limit(
    arguments: &Map<String, Value>,
) -> Result<Option<i64>, ToolCallError> {
    match arguments.get("limit") {
        None => Ok(Some(DEFAULT_HISTORY_LIMIT)),
        Some(Value::Number(n)) => {
            let value = n.as_i64().ok_or_else(|| {
                ToolCallError::InvalidArguments(
                    "argument `limit` must be a positive integer".to_string(),
                )
            })?;
            Ok(Some(value.clamp(1, MAX_HISTORY_LIMIT)))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `limit` must be a number".to_string(),
        )),
    }
}

pub(super) fn parse_optional_timeout(
    arguments: &Map<String, Value>,
) -> Result<Option<u64>, ToolCallError> {
    match arguments.get("timeoutMs") {
        None => Ok(Some(60_000)),
        Some(Value::Number(n)) => {
            let ms = n.as_u64().ok_or_else(|| {
                ToolCallError::InvalidArguments(
                    "argument `timeoutMs` must be a non-negative integer".to_string(),
                )
            })?;
            Ok(Some(ms))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `timeoutMs` must be a number".to_string(),
        )),
    }
}

#[cfg(test)]
mod timeout_tests {
    use super::*;

    #[test]
    fn omitted_timeout_uses_the_mcp_safety_default() {
        assert_eq!(parse_optional_timeout(&Map::new()).unwrap(), Some(60_000));
    }

    #[test]
    fn zero_is_unlimited_and_positive_values_are_not_capped() {
        let zero = serde_json::from_value::<Map<String, Value>>(json!({ "timeoutMs": 0 })).unwrap();
        let long = serde_json::from_value::<Map<String, Value>>(json!({
            "timeoutMs": 120_000
        }))
        .unwrap();
        assert_eq!(parse_optional_timeout(&zero).unwrap(), Some(0));
        assert_eq!(parse_optional_timeout(&long).unwrap(), Some(120_000));
    }

    #[test]
    fn invalid_timeouts_are_rejected() {
        for value in [json!(-1), json!(1.5), json!("1000")] {
            let arguments = serde_json::from_value::<Map<String, Value>>(json!({
                "timeoutMs": value
            }))
            .unwrap();
            assert!(parse_optional_timeout(&arguments).is_err());
        }
    }
}

pub(super) fn guess_body_type(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if serde_json::from_str::<Value>(trimmed).is_ok() {
            return "json".to_string();
        }
    }
    if trimmed.starts_with('<') {
        return "xml".to_string();
    }
    "text".to_string()
}

pub(super) fn serialize_safe(value: impl serde::Serialize) -> Result<Value, ToolCallError> {
    serde_json::to_value(value).map_err(|_| ToolCallError::Execution {
        code: "TOOL_RESULT_SERIALIZATION_FAILED",
        message: "The tool result could not be serialized.",
    })
}

pub(super) fn unexpected_result() -> ToolCallError {
    ToolCallError::Execution {
        code: "COMMAND_BUS_RESULT_MISMATCH",
        message: "The command-bus returned an unexpected result.",
    }
}
