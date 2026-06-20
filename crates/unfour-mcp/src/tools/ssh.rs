use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::SshDiagnosticInput;

use crate::command_bus_adapter::CommandBusAdapter;

use super::{object_with_allowed_keys, RegisteredTool, ToolCallError, ToolDefinition, ToolHandler};

const MAX_DIAGNOSTIC_TIMEOUT_MS: u64 = 60_000;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![RegisteredTool {
        definition: ToolDefinition {
            name: "unfour.ssh.run_diagnostic",
            title: "Run SSH Diagnostic Command",
            description:
                "Runs a single read-only diagnostic command on a saved SSH connection through the Unfour command bus and returns captured stdout/stderr. Only a fixed allowlist of read-only utilities is permitted (df, free, uptime, ps, ss, ip, tail, cat, systemctl status, journalctl, ...); shells, pipes, redirection, chaining, and any write/control operation are rejected. Output is line-redacted for sensitive material. Requires an SSH-native build.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "connectionId": {
                        "type": "string",
                        "description": "The saved SSH connection ID to run the command on."
                    },
                    "command": {
                        "type": "string",
                        "description": "A single read-only diagnostic command from the allowlist (e.g. \"df -h\", \"systemctl status nginx\", \"tail -n 200 /var/log/syslog\")."
                    },
                    "workspaceId": {
                        "type": "string",
                        "description": "Optional workspace ID. Uses the active workspace if omitted."
                    },
                    "timeoutMs": {
                        "type": "integer",
                        "description": "Optional command timeout in milliseconds (default 15000, max 60000)."
                    }
                },
                "required": ["connectionId", "command"],
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "connectionId": { "type": "string" },
                    "command": { "type": "string" },
                    "stdout": { "type": "string" },
                    "stderr": { "type": "string" },
                    "exitStatus": { "type": ["integer", "null"] },
                    "truncated": { "type": "boolean" },
                    "source": { "type": "string", "const": "command-bus" }
                },
                "required": ["connectionId", "command", "stdout", "stderr", "truncated", "source"],
                "additionalProperties": false
            }),
        },
        handler: ToolHandler::Real(ssh_run_diagnostic),
    }]
}

fn ssh_run_diagnostic(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &["connectionId", "command", "workspaceId", "timeoutMs"],
    )?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.ssh.run_diagnostic")?;
    let command = parse_required_string(&arguments, "command", "unfour.ssh.run_diagnostic")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let timeout_ms = parse_optional_timeout(&arguments)?;

    let result = command_bus
        .run_ssh_diagnostic(SshDiagnosticInput {
            workspace_id,
            connection_id,
            command,
            timeout_ms,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    Ok(json!({
        "connectionId": result.connection_id,
        "command": result.command,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exitStatus": result.exit_status,
        "truncated": result.truncated,
        "source": "command-bus"
    }))
}

fn resolve_workspace_id(
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

fn parse_optional_string(
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

fn parse_required_string(
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

fn parse_optional_timeout(arguments: &Map<String, Value>) -> Result<Option<u64>, ToolCallError> {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use unfour_command_bus::{CurrentWorkspaceResult, ReadCommand, ReadCommandResult};
    use unfour_core::models::{
        ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult,
        DatabaseQuerySafety, DatabaseSchema, SshDiagnosticInput, SshDiagnosticResult,
    };

    use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
    use crate::tools::ToolRegistry;

    /// Stub that runs diagnostics, echoing the validated command back.
    struct SshStubCommandBus;

    impl CommandBusAdapter for SshStubCommandBus {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            match command {
                ReadCommand::CurrentWorkspace => Ok(ReadCommandResult::CurrentWorkspace(
                    CurrentWorkspaceResult {
                        workspace_id: "ws-active".to_string(),
                        workspace_name: "Active".to_string(),
                        workspace_root: None,
                        mode: "local".to_string(),
                        source: "command-bus".to_string(),
                    },
                )),
                _ => Err(CommandBusAdapterError {
                    code: "UNEXPECTED",
                    message: "unexpected command",
                }),
            }
        }

        fn run_ssh_diagnostic(
            &self,
            input: SshDiagnosticInput,
        ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
            Ok(SshDiagnosticResult {
                connection_id: input.connection_id,
                command: input.command,
                stdout: "Filesystem Size Used Avail\n/dev/sda1 50G 20G 30G".to_string(),
                stderr: String::new(),
                exit_status: Some(0),
                truncated: false,
            })
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            unreachable!()
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            Ok(vec![])
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            _connection_id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            Ok(DatabaseSchema {
                connection_id: String::new(),
                tables: vec![],
            })
        }

        fn execute_db_query(
            &self,
            _input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            Ok(DatabaseQueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: 0,
                duration_ms: 0,
                safety: DatabaseQuerySafety {
                    classification: "read".to_string(),
                    requires_confirmation: false,
                    confirmed: true,
                    message: None,
                },
            })
        }
    }

    /// Stub that does NOT override `run_ssh_diagnostic`, so the trait default
    /// (unsupported) is exercised — mirrors a build without `ssh-native`.
    struct UnsupportedSshCommandBus;

    impl CommandBusAdapter for UnsupportedSshCommandBus {
        fn execute_read(
            &self,
            _command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "ws-active".to_string(),
                    workspace_name: "Active".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            ))
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            unreachable!()
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            Ok(vec![])
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            _connection_id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            Ok(DatabaseSchema {
                connection_id: String::new(),
                tables: vec![],
            })
        }

        fn execute_db_query(
            &self,
            _input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            unreachable!()
        }
    }

    fn registry() -> ToolRegistry {
        ToolRegistry::with_command_bus(Arc::new(SshStubCommandBus))
    }

    #[test]
    fn ssh_tool_is_registered() {
        assert!(registry()
            .definitions()
            .iter()
            .any(|d| d.name == "unfour.ssh.run_diagnostic"));
    }

    #[test]
    fn run_diagnostic_returns_captured_output() {
        let result = registry()
            .call(
                "unfour.ssh.run_diagnostic",
                json!({ "connectionId": "conn-1", "command": "df -h" }),
            )
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        let content = &result["structuredContent"];
        assert_eq!(content["connectionId"], "conn-1");
        assert_eq!(content["command"], "df -h");
        assert_eq!(content["exitStatus"], 0);
        assert!(content["stdout"].as_str().unwrap().contains("Filesystem"));
        assert_eq!(content["source"], "command-bus");
    }

    #[test]
    fn run_diagnostic_requires_connection_and_command() {
        assert!(registry()
            .call("unfour.ssh.run_diagnostic", json!({ "command": "df -h" }))
            .is_err());
        assert!(registry()
            .call(
                "unfour.ssh.run_diagnostic",
                json!({ "connectionId": "conn-1" })
            )
            .is_err());
    }

    #[test]
    fn run_diagnostic_surfaces_unsupported_when_native_disabled() {
        let registry = ToolRegistry::with_command_bus(Arc::new(UnsupportedSshCommandBus));
        let result = registry
            .call(
                "unfour.ssh.run_diagnostic",
                json!({ "connectionId": "conn-1", "command": "uptime" }),
            )
            .expect("execution errors become MCP tool results");

        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "COMMAND_BUS_OPERATION_UNSUPPORTED"
        );
    }
}
