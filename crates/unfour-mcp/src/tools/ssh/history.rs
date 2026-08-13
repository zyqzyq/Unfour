use serde_json::{json, Map, Value};
use unfour_core::models::{SshCommandHistoryEntry, SshCommandHistoryQuery, SshConnection};

use crate::command_bus_adapter::CommandBusAdapter;

use super::super::policy::ToolPolicyEvaluation;
use super::super::ssh_risk::redact_command_display;
use super::super::{
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
};
use super::helpers::parse_optional_string;

const DEFAULT_HISTORY_LIMIT: i64 = 50;
const MAX_HISTORY_LIMIT: i64 = 200;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![RegisteredTool {
        definition: ToolDefinition {
            name: "unfour.ssh.list_history",
            title: "List SSH Command History",
            description:
                "Lists structured SSH command history for the current MCP workspace. Returns connection, command, executedAt, and cwd/exitCode/durationMs when recorded. Results are workspace-scoped and never include terminal buffers or full session logs. Sensitive commands are excluded or replaced with [redacted command]. Use this to inspect recent host activity, then draft a reusable SSH Task for the user to confirm. Do not call unfour.ssh.save_task unless the user explicitly asks to save the draft.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspaceId": {
                        "type": "string",
                        "description": "Optional workspace ID. Uses the active MCP workspace if omitted. History from other workspaces is never returned."
                    },
                    "connectionId": {
                        "type": "string",
                        "description": "Optional saved SSH connection ID. Unknown or out-of-workspace connections return an empty list."
                    },
                    "query": {
                        "type": "string",
                        "description": "Optional substring filter matched against the stored command text."
                    },
                    "since": {
                        "type": "string",
                        "description": "Optional inclusive RFC 3339 lower bound on executedAt."
                    },
                    "until": {
                        "type": "string",
                        "description": "Optional inclusive RFC 3339 upper bound on executedAt."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of history entries to return (default 50, max 200)."
                    }
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "history": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "connection": {
                                    "type": "object",
                                    "properties": {
                                        "id": { "type": "string" },
                                        "name": { "type": ["string", "null"] },
                                        "host": { "type": ["string", "null"] },
                                        "port": { "type": ["integer", "null"] },
                                        "username": { "type": ["string", "null"] }
                                    },
                                    "required": ["id"],
                                    "additionalProperties": false
                                },
                                "command": { "type": "string" },
                                "cwd": { "type": ["string", "null"] },
                                "exitCode": { "type": ["integer", "null"] },
                                "durationMs": { "type": ["integer", "null"] },
                                "executedAt": { "type": "string" }
                            },
                            "required": ["id", "connection", "command", "executedAt"],
                            "additionalProperties": false
                        }
                    },
                    "count": { "type": "integer", "minimum": 0 },
                    "workspaceId": { "type": "string" },
                    "source": { "type": "string", "const": "command-bus" }
                },
                "required": ["history", "count", "workspaceId", "source"],
                "additionalProperties": false
            }),
            annotations: ToolAnnotations::local_read(),
        },
        handler: list_history,
    }]
}

fn list_history(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "connectionId",
            "query",
            "since",
            "until",
            "limit",
        ],
    )?;
    let workspace_id = evaluation.workspace.workspace_id.clone();
    let connection_id = optional_trimmed(&arguments, "connectionId")?;
    let search = optional_trimmed(&arguments, "query")?;
    let since = optional_trimmed(&arguments, "since")?;
    let until = optional_trimmed(&arguments, "until")?;
    if let (Some(since), Some(until)) = (since.as_deref(), until.as_deref()) {
        if since > until {
            return Err(ToolCallError::InvalidArguments(
                "argument `since` must be less than or equal to `until`".to_string(),
            ));
        }
    }
    let limit = parse_optional_history_limit(&arguments)?;

    let connections = command_bus
        .list_ssh_connections(&workspace_id)
        .map_err(execution_error)?;
    let entries = command_bus
        .list_ssh_command_history(SshCommandHistoryQuery {
            workspace_id: workspace_id.clone(),
            connection_id,
            search,
            limit,
            include_redacted: false,
            since,
            until,
        })
        .map_err(execution_error)?;

    let history = entries
        .into_iter()
        .filter(|entry| entry.workspace_id == workspace_id && !entry.redacted)
        .map(|entry| serialize_history_entry(&entry, &connections))
        .collect::<Vec<_>>();
    let count = history.len();

    Ok(json!({
        "history": history,
        "count": count,
        "workspaceId": workspace_id,
        "source": "command-bus"
    }))
}

fn serialize_history_entry(entry: &SshCommandHistoryEntry, connections: &[SshConnection]) -> Value {
    let connection = connections
        .iter()
        .find(|connection| connection.id == entry.connection_id);
    json!({
        "id": entry.id,
        "connection": {
            "id": entry.connection_id,
            "name": connection.map(|connection| connection.name.as_str()),
            "host": connection.map(|connection| connection.host.as_str()),
            "port": connection.map(|connection| connection.port),
            "username": connection.map(|connection| connection.username.as_str())
        },
        "command": redact_command_display(&entry.command),
        "cwd": entry.cwd,
        "exitCode": entry.exit_code,
        "durationMs": entry.duration_ms,
        "executedAt": entry.executed_at
    })
}

fn optional_trimmed(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    Ok(parse_optional_string(arguments, key)?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

fn parse_optional_history_limit(
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

fn execution_error(error: crate::command_bus_adapter::CommandBusAdapterError) -> ToolCallError {
    ToolCallError::Execution {
        code: error.code,
        message: error.message,
    }
}
