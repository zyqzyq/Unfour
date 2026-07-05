use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};

use crate::command_bus_adapter::{CommandBusAdapter, LocalCommandBusAdapter};
use crate::protocol;
use crate::tools::{ToolCallError, ToolRegistry};

pub const SUPPORTED_PROTOCOL_VERSION: &str = "2025-06-18";

pub struct McpServer {
    tools: ToolRegistry,
}

impl McpServer {
    pub fn new(command_bus: Arc<dyn CommandBusAdapter>) -> Self {
        let tools = ToolRegistry::with_command_bus(command_bus);
        let definitions = tools.definitions();
        unfour_diag::log_operation_event(
            "mcp_tools_registered",
            "mcp",
            "tools_registry",
            "ok",
            None,
            None,
            json!({
                "tool_count": definitions.len(),
                "tool_names": definitions.iter().map(|tool| tool.name).collect::<Vec<_>>(),
            }),
        );
        Self { tools }
    }

    pub fn handle_line(&self, line: &str) -> Option<String> {
        let response = match serde_json::from_str::<Value>(line) {
            Ok(message) => self.handle_message(&message),
            Err(error) => Some(protocol::error(
                Value::Null,
                -32700,
                format!("Parse error: {error}"),
            )),
        };

        response.map(|value| {
            serde_json::to_string(&value).expect("serializing a JSON-RPC response cannot fail")
        })
    }

    pub fn handle_message(&self, message: &Value) -> Option<Value> {
        let Some(object) = message.as_object() else {
            return Some(protocol::error(Value::Null, -32600, "Invalid Request"));
        };

        let id = object.get("id").cloned();
        let response_id = id.clone().unwrap_or(Value::Null);

        if object.get("jsonrpc").and_then(Value::as_str) != Some(protocol::JSON_RPC_VERSION) {
            return id.map(|_| protocol::error(response_id, -32600, "Invalid Request"));
        }

        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return id.map(|_| protocol::error(response_id, -32600, "Invalid Request"));
        };

        let result = match method {
            "initialize" => self.initialize(object.get("params")),
            "notifications/initialized" => return None,
            "tools/list" => Ok(json!({ "tools": self.tools.definitions() })),
            "tools/call" => self.call_tool(object.get("params")),
            _ => Err((-32601, format!("Method not found: {method}"))),
        };

        id.map(|id| match result {
            Ok(result) => protocol::success(id, result),
            Err((code, message)) => protocol::error(id, code, message),
        })
    }

    fn initialize(&self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        let requested_version = params
            .and_then(|value| value.get("protocolVersion"))
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "Missing protocolVersion".to_string()))?;

        let protocol_version = if requested_version == SUPPORTED_PROTOCOL_VERSION {
            requested_version
        } else {
            SUPPORTED_PROTOCOL_VERSION
        };

        Ok(json!({
            "protocolVersion": protocol_version,
            "capabilities": {
                "tools": {
                    "listChanged": false,
                }
            },
            "serverInfo": {
                "name": "unfour-mcp",
                "title": "Unfour MCP",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": "Unfour exposes workspace-scoped API, database, SSH, activity, and health tools over the command bus. Default policy is environment-aware: dev workspaces allow ordinary read/write repair actions, test workspaces guard higher-risk actions, and prod workspaces are read-only except safe diagnostics. Recommended flow: (1) unfour.system.health; (2) unfour.activity.list; (3) inspect API history or saved requests, then use unfour.api.send_request/create_request/update_request/delete_request when policy allows; (4) inspect database connections and schemas, use unfour.db.query_readonly or unfour.db.explain first, then unfour.db.execute for confirmed fixes; (5) for hosts, start with unfour.ssh.run_diagnostic/list_dir/read_file, then use unfour.ssh.exec/write_file/patch_file only when appropriate. High-risk tools return CONFIRMATION_REQUIRED with a content-bound confirmation_text; re-run with confirm=true and that exact value to execute. Secrets are masked and never returned in usable form. Check tool annotations and structuredContent.risk_level before acting.",
        }))
    }

    fn call_tool(&self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        let params = params
            .and_then(Value::as_object)
            .ok_or_else(|| (-32602, "tools/call params must be an object".to_string()))?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "tools/call requires a tool name".to_string()))?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let request_id = unfour_diag::new_request_id();
        let started = Instant::now();
        unfour_diag::log_operation_event(
            "tool_call_started",
            "mcp",
            "tools_call",
            "started",
            None,
            None,
            json!({ "request_id": request_id.as_str(), "tool_name": name }),
        );
        let result = self.tools.call(name, arguments);
        match result {
            Ok(value) => {
                unfour_diag::log_operation_event(
                    "tool_call_completed",
                    "mcp",
                    "tools_call",
                    "ok",
                    Some(started.elapsed().as_millis()),
                    None,
                    json!({ "request_id": request_id.as_str(), "tool_name": name }),
                );
                Ok(value)
            }
            Err(error) => {
                let error_kind = match &error {
                    ToolCallError::UnknownTool(_) => "UNKNOWN_TOOL",
                    ToolCallError::InvalidArguments(_) => "INVALID_ARGUMENTS",
                    ToolCallError::ConfirmationRequired(_) => "CONFIRMATION_REQUIRED",
                    ToolCallError::PolicyBlocked(_) => "POLICY_BLOCKED",
                    ToolCallError::Execution { code, .. } => *code,
                };
                unfour_diag::log_operation_event(
                    "tool_call_failed",
                    "mcp",
                    "tools_call",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(error_kind),
                    json!({ "request_id": request_id.as_str(), "tool_name": name }),
                );
                Err(match error {
                    ToolCallError::UnknownTool(name) => (-32602, format!("Unknown tool: {name}")),
                    ToolCallError::InvalidArguments(message) => (-32602, message),
                    ToolCallError::ConfirmationRequired(confirmation) => (
                        -32000,
                        format!("CONFIRMATION_REQUIRED: {}", confirmation.reason),
                    ),
                    ToolCallError::PolicyBlocked(denial) => {
                        (-32000, format!("{}: {}", denial.error.code, denial.reason))
                    }
                    ToolCallError::Execution { code, message } => {
                        (-32000, format!("{code}: {message}"))
                    }
                })
            }
        }
    }
}

pub fn run_stdio<R, W>(reader: R, mut writer: W) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let command_bus = LocalCommandBusAdapter::default_storage()
        .map_err(|error| io::Error::other(format!("{}: {}", error.code, error.message)))?;
    let server = McpServer::new(command_bus);
    unfour_diag::log_operation_event(
        "mcp_server_started",
        "mcp",
        "run_stdio",
        "ok",
        None,
        None,
        json!({ "transport": "stdio" }),
    );
    run_stdio_with_server(&server, reader, &mut writer)
}

/// Drive the stdio read/write loop with an already-built server. Splitting this
/// out keeps the transport logic testable without opening the real OS app-data
/// store (which does not exist in CI).
fn run_stdio_with_server<R, W>(server: &McpServer, reader: R, writer: &mut W) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Some(response) = server.handle_line(&line) {
            writeln!(writer, "{response}")?;
            writer.flush()?;
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "server_tests/mod.rs"]
mod server_tests;
