use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::SshConnection;

use crate::command_bus_adapter::CommandBusAdapter;

use super::super::ssh_risk::{
    parse_optional_u64, redact_command_display, shell_quote,
};
use super::super::ToolCallError;
use super::{MAX_DIAGNOSTIC_TIMEOUT_MS, MAX_ONE_SHOT_COMMAND_CHARS};
#[derive(Debug, Clone)]
pub(super) struct WorkspaceContext {
    pub(super) workspace_id: String,
    pub(super) environment_type: String,
}

pub(super) fn resolve_workspace(
    command_bus: &dyn CommandBusAdapter,
    arguments: &Map<String, Value>,
) -> Result<WorkspaceContext, ToolCallError> {
    match parse_optional_string(arguments, "workspaceId")? {
        Some(id) => {
            let result = command_bus
                .execute_read(ReadCommand::ListWorkspaces)
                .map_err(|error| ToolCallError::Execution {
                    code: error.code,
                    message: error.message,
                })?;
            let ReadCommandResult::Workspaces(workspaces) = result else {
                return Err(ToolCallError::Execution {
                    code: "COMMAND_BUS_RESULT_MISMATCH",
                    message: "The command-bus returned an unexpected result.",
                });
            };
            let workspace = workspaces
                .workspaces
                .into_iter()
                .find(|workspace| workspace.id == id)
                .ok_or(ToolCallError::Execution {
                    code: "WORKSPACE_NOT_FOUND",
                    message: "The requested workspace was not found.",
                })?;
            Ok(WorkspaceContext {
                workspace_id: workspace.id,
                environment_type: workspace.environment_type,
            })
        }
        None => {
            let result = command_bus
                .execute_read(ReadCommand::CurrentWorkspace)
                .map_err(|error| ToolCallError::Execution {
                    code: error.code,
                    message: error.message,
                })?;
            let ReadCommandResult::CurrentWorkspace(workspace) = result else {
                return Err(ToolCallError::Execution {
                    code: "COMMAND_BUS_RESULT_MISMATCH",
                    message: "The command-bus returned an unexpected result.",
                });
            };
            Ok(WorkspaceContext {
                workspace_id: workspace.workspace_id,
                environment_type: workspace.environment_type,
            })
        }
    }
}

pub(super) fn ssh_command_result(
    result: unfour_core::models::SshDiagnosticResult,
    source: &str,
) -> Value {
    json!({
        "connectionId": result.connection_id,
        "command": redact_command_display(&result.command),
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exitCode": result.exit_status,
        "exitStatus": result.exit_status,
        "truncated": result.truncated,
        "source": source
    })
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

pub(super) fn python_write_file_command(
    path: &str,
    content: &[u8],
    python_mode: &str,
) -> Result<String, ToolCallError> {
    let script = "from pathlib import Path; import sys; data=bytes.fromhex(sys.argv[3]); f=Path(sys.argv[1]).open(sys.argv[2]); f.write(data); f.close()";
    ensure_one_shot_command_length(format!(
        "python3 -c {} -- {} {} {}",
        shell_quote(script),
        shell_quote(path),
        shell_quote(python_mode),
        shell_quote(&hex_encode(content))
    ))
}

pub(super) fn python_patch_file_command(
    path: &str,
    search: &str,
    replace: &str,
    allow_multiple: &str,
) -> Result<String, ToolCallError> {
    let script = "import sys; from pathlib import Path; p=Path(sys.argv[1]); text=p.read_text(); search=bytes.fromhex(sys.argv[2]).decode(); replace=bytes.fromhex(sys.argv[3]).decode(); allow=sys.argv[4]=='1'; count=text.count(search); print('__UNFOUR_MATCH_COUNT__', count); sys.exit(3) if count == 0 or (count > 1 and not allow) else None; replacements=count if allow else 1; p.write_text(text.replace(search, replace, replacements)); print('__UNFOUR_PATCHED__', replacements)";
    ensure_one_shot_command_length(format!(
        "python3 -c {} -- {} {} {} {}",
        shell_quote(script),
        shell_quote(path),
        shell_quote(&hex_encode(search.as_bytes())),
        shell_quote(&hex_encode(replace.as_bytes())),
        shell_quote(allow_multiple)
    ))
}

pub(super) fn ensure_one_shot_command_length(command: String) -> Result<String, ToolCallError> {
    if command.chars().count() > MAX_ONE_SHOT_COMMAND_CHARS {
        return Err(ToolCallError::Execution {
            code: "SSH_COMMAND_TOO_LARGE",
            message: "The generated SSH command exceeds the one-shot command length limit.",
        });
    }
    Ok(command)
}

pub(super) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
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
                return Err(ToolCallError::Execution {
                    code: "COMMAND_BUS_RESULT_MISMATCH",
                    message: "The command-bus returned an unexpected result.",
                });
            };
            Ok(ws.workspace_id)
        }
    }
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
