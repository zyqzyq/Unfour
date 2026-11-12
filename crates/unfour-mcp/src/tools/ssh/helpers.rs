use serde_json::{json, Map, Value};
use unfour_core::models::SshConnection;

use super::super::ssh_risk::{parse_optional_u64, redact_command_display};
use super::super::ToolCallError;
use super::MAX_DIAGNOSTIC_TIMEOUT_MS;
pub(super) fn ssh_command_result(
    result: unfour_core::models::SshDiagnosticResult,
    source: &str,
) -> Value {
    let mut value = json!({
        "connectionId": result.connection_id,
        "command": redact_command_display(&result.command),
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exitCode": result.exit_status,
        "exitStatus": result.exit_status,
        "truncated": result.truncated,
        "source": source
    });
    insert_activity_warnings(&mut value, &result.warnings);
    value
}

pub(super) fn insert_activity_warnings(value: &mut Value, warnings: &[String]) {
    if !warnings.is_empty() {
        value["warnings"] = json!(warnings);
    }
}

pub(super) fn safe_connection_summary(connection: &SshConnection) -> Value {
    json!({
        "connectionId": connection.id,
        "id": connection.id,
        "workspaceId": connection.workspace_id,
        "name": connection.name,
        "host": connection.host,
        "port": connection.port,
        "username": connection.username,
        "authKind": connection.auth_kind
    })
}

pub(super) fn parse_optional_port(
    arguments: &Map<String, Value>,
) -> Result<Option<u16>, ToolCallError> {
    let Some(value) = parse_optional_u64(arguments, "port")? else {
        return Ok(None);
    };
    if !(1..=u16::MAX as u64).contains(&value) {
        return Err(ToolCallError::InvalidArguments(
            "argument `port` must be between 1 and 65535".to_string(),
        ));
    }
    Ok(Some(value as u16))
}

pub(super) fn parse_optional_secret(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a string"
        ))),
    }
}

pub(super) fn parse_match_count(stdout: &str) -> Option<u64> {
    stdout.lines().find_map(|line| {
        line.strip_prefix("__UNFOUR_MATCH_COUNT__")
            .and_then(|rest| rest.trim().parse::<u64>().ok())
    })
}

pub(super) fn parse_find_entries(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.to_string();
            let kind = match parts.next().unwrap_or_default() {
                "d" => "directory",
                "f" => "file",
                "l" => "symlink",
                other => other,
            };
            let size = parts
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            let modified_at = parts.next().unwrap_or_default();
            Some(json!({
                "name": name,
                "type": kind,
                "size": size,
                "modifiedAt": modified_at
            }))
        })
        .collect()
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

pub(super) fn parse_required_raw_string(
    arguments: &Map<String, Value>,
    key: &str,
    tool_name: &str,
    allow_empty: bool,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(s)) if allow_empty || !s.is_empty() => Ok(s.clone()),
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

pub(super) fn parse_optional_timeout(
    arguments: &Map<String, Value>,
) -> Result<Option<u64>, ToolCallError> {
    match arguments.get("timeoutMs") {
        None => Ok(None),
        Some(Value::Number(n)) => {
            let ms = n.as_u64().ok_or_else(|| {
                ToolCallError::InvalidArguments(
                    "argument `timeoutMs` must be a positive number".to_string(),
                )
            })?;
            Ok(Some(ms.min(MAX_DIAGNOSTIC_TIMEOUT_MS)))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `timeoutMs` must be a number".to_string(),
        )),
    }
}
