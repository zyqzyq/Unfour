use std::io::{self, BufRead, Write};
use std::sync::Arc;

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
        Self {
            tools: ToolRegistry::with_command_bus(command_bus),
        }
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
            "instructions": "Unfour exposes read-only backend diagnostics over the command bus, scoped to the active workspace. Recommended troubleshooting flow: (1) unfour.system.health to confirm the store is ready; (2) unfour.activity.list to see what changed recently before a failure started; (3) for API issues, unfour.api.list_history then unfour.api.get_history to find the first failing request and inspect masked auth, and unfour.api.send_request to replay a saved request; (4) for database issues, unfour.db.list_connections, unfour.db.list_tables, unfour.db.describe_table, and unfour.db.query_readonly (SELECT/WITH/SHOW/EXPLAIN only); (5) for host/service issues, unfour.ssh.run_diagnostic with read-only commands (df, free, journalctl, grep, docker logs, kubectl get/logs, ...). Every tool is read-only or replay-only and never mutates your data; secrets are masked and never returned in usable form. Check each tool's annotations: openWorldHint marks tools that reach an external database or SSH host.",
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

        self.tools
            .call(name, arguments)
            .map_err(|error| match error {
                ToolCallError::UnknownTool(name) => (-32602, format!("Unknown tool: {name}")),
                ToolCallError::InvalidArguments(message) => (-32602, message),
                ToolCallError::Execution { code, message } => {
                    (-32000, format!("{code}: {message}"))
                }
            })
    }
}

pub fn run_stdio<R, W>(reader: R, mut writer: W) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let command_bus = LocalCommandBusAdapter::send_app_data()
        .map_err(|error| io::Error::other(format!("{}: {}", error.code, error.message)))?;
    let server = McpServer::new(command_bus);
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
mod tests {
    use std::io::Cursor;
    use std::sync::Arc;

    use serde_json::{json, Value};
    use unfour_command_bus::{
        ApiCollectionListResult, ApiEnvironmentListResult, ApiHistoryDetailResult,
        ApiHistoryListResult, ApiRequestDetailResult, ApiRequestListResult, ConnectionListResult,
        CurrentWorkspaceResult, ReadCommand, ReadCommandResult, WorkspaceListResult,
    };
    use unfour_core::models::{
        ApiHistoryDetail, ApiResponse, ApiSavedRequest, DatabaseConnection, DatabaseQueryInput,
        DatabaseQueryResult, DatabaseQuerySafety, DatabaseSchema,
    };

    use super::{run_stdio_with_server, McpServer, SUPPORTED_PROTOCOL_VERSION};
    use crate::command_bus_adapter::{
        CommandBusAdapter, CommandBusAdapterError, LocalCommandBusAdapter,
    };

    struct StubCommandBus;

    impl CommandBusAdapter for StubCommandBus {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            Ok(match command {
                ReadCommand::CurrentWorkspace => {
                    ReadCommandResult::CurrentWorkspace(CurrentWorkspaceResult {
                        workspace_id: "workspace-1".to_string(),
                        workspace_name: "Workspace".to_string(),
                        workspace_root: None,
                        mode: "local".to_string(),
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ListWorkspaces => ReadCommandResult::Workspaces(WorkspaceListResult {
                    workspaces: vec![],
                    active_workspace_id: "workspace-1".to_string(),
                    count: 0,
                    source: "command-bus".to_string(),
                }),
                ReadCommand::ListConnections { .. } => {
                    ReadCommandResult::Connections(ConnectionListResult {
                        connections: vec![],
                        count: 0,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListCollections { .. } => {
                    ReadCommandResult::ApiCollections(ApiCollectionListResult {
                        collections: vec![],
                        count: 0,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListRequests { .. } => {
                    ReadCommandResult::ApiRequests(ApiRequestListResult {
                        requests: vec![],
                        count: 0,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiGetRequest { request_id } => {
                    ReadCommandResult::ApiRequest(ApiRequestDetailResult {
                        request: ApiSavedRequest {
                            id: request_id,
                            workspace_id: "workspace-1".to_string(),
                            name: "Test".to_string(),
                            folder_path: None,
                            collection_id: None,
                            method: "GET".to_string(),
                            url: "https://example.com".to_string(),
                            headers_json: "[]".to_string(),
                            query_json: "[]".to_string(),
                            body: None,
                            body_kind: "json".to_string(),
                            created_at: String::new(),
                            updated_at: String::new(),
                            deleted_at: None,
                            revision: 1,
                            sync_status: "local".to_string(),
                            remote_id: None,
                        },
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListHistory { .. } => {
                    ReadCommandResult::ApiHistory(ApiHistoryListResult {
                        history: vec![],
                        count: 0,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiGetHistory { history_id, .. } => {
                    ReadCommandResult::ApiHistoryDetailResult(ApiHistoryDetailResult {
                        detail: ApiHistoryDetail {
                            id: history_id,
                            workspace_id: "workspace-1".to_string(),
                            name: None,
                            method: "GET".to_string(),
                            url: "https://example.com".to_string(),
                            request_headers_json: "[]".to_string(),
                            request_query_json: "[]".to_string(),
                            request_body: None,
                            status: Some(200),
                            duration_ms: Some(10),
                            response_headers_json: "[]".to_string(),
                            response_body_preview: None,
                            created_at: String::new(),
                            updated_at: String::new(),
                            deleted_at: None,
                            revision: 1,
                            sync_status: "local".to_string(),
                            remote_id: None,
                        },
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListEnvironments { .. } => {
                    ReadCommandResult::ApiEnvironments(ApiEnvironmentListResult {
                        environments: vec![],
                        count: 0,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ListActivity { .. } => {
                    ReadCommandResult::Activity(unfour_command_bus::ActivityListResult {
                        activity: vec![],
                        count: 0,
                        source: "command-bus".to_string(),
                    })
                }
            })
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            Ok(ApiResponse {
                history_id: "h-1".to_string(),
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![],
                body: "{}".to_string(),
                duration_ms: 10,
            })
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

    fn server() -> McpServer {
        McpServer::new(Arc::new(StubCommandBus))
    }

    #[test]
    fn initialize_declares_tools_capability() {
        let response = server()
            .handle_message(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": SUPPORTED_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "test-client",
                        "version": "0.1.0"
                    }
                }
            }))
            .expect("initialize should return a response");

        assert_eq!(
            response["result"]["protocolVersion"],
            SUPPORTED_PROTOCOL_VERSION
        );
        assert_eq!(
            response["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
    }

    #[test]
    fn stdio_round_trip_lists_and_calls_tools() {
        let input = [
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": SUPPORTED_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "test-client",
                        "version": "0.1.0"
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "unfour.workspace.current",
                    "arguments": {}
                }
            }),
        ]
        .into_iter()
        .map(|message| serde_json::to_string(&message).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        let mut output = Vec::new();

        // Use an ephemeral in-memory command bus rather than the real OS
        // app-data store, which does not exist in CI.
        let command_bus =
            LocalCommandBusAdapter::ephemeral().expect("ephemeral command bus should initialize");
        let server = McpServer::new(command_bus);
        run_stdio_with_server(&server, Cursor::new(input), &mut output)
            .expect("stdio server should complete");

        let responses = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(responses.len(), 3);
        assert_eq!(
            responses[1]["result"]["tools"].as_array().unwrap().len(),
            18
        );
        // `run_stdio` opens the real app-data store, so assert only on stable,
        // data-independent fields rather than a specific workspace id.
        assert_eq!(responses[2]["result"]["isError"], false);
        assert_eq!(
            responses[2]["result"]["structuredContent"]["source"],
            "command-bus"
        );
    }
}
