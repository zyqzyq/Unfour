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
                    environment_type: "dev".to_string(),
                    mcp_policy: "auto".to_string(),
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
                        collection_id: "collection-1".to_string(),
                        parent_folder_id: None,
                        sort_order: 0,
                        auth_json: r#"{"type":"none"}"#.to_string(),
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
        34
    );
    // `run_stdio` opens the real app-data store, so assert only on stable,
    // data-independent fields rather than a specific workspace id.
    assert_eq!(responses[2]["result"]["isError"], false);
    assert_eq!(
        responses[2]["result"]["structuredContent"]["source"],
        "command-bus"
    );
}
