use super::*;
use std::sync::Arc;
use unfour_command_bus::{
    ApiCollectionListResult, ApiCollectionSummary, ApiEnvironmentListResult,
    ApiHistoryDetailResult, ApiHistoryListResult, ApiRequestDetailResult, ApiRequestListResult,
    ApiRequestSummary, CurrentWorkspaceResult, ReadCommand, ReadCommandResult, WorkspaceListResult,
    WorkspaceSummary,
};
use unfour_core::models::{
    ApiEnvironment, ApiHistoryDetail, ApiHistoryItem, ApiResponse, ApiSavedRequest,
    DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult, DatabaseQuerySafety,
    DatabaseSchema, KeyValue,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use crate::tools::ToolRegistry;

// --- Test stubs ---

struct ApiStubCommandBus;

impl CommandBusAdapter for ApiStubCommandBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        Ok(match command {
                ReadCommand::CurrentWorkspace => {
                    ReadCommandResult::CurrentWorkspace(CurrentWorkspaceResult {
                        workspace_id: "ws-1".to_string(),
                        workspace_name: "API Workspace".to_string(),
                        environment_type: "dev".to_string(),
                        mcp_policy: "auto".to_string(),
                        workspace_root: None,
                        mode: "local".to_string(),
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ListWorkspaces => ReadCommandResult::Workspaces(WorkspaceListResult {
                    workspaces: vec![WorkspaceSummary {
                        id: "ws-1".to_string(),
                        name: "API Workspace".to_string(),
                        is_default: true,
                        is_active: true,
                        environment_type: "dev".to_string(),
                        mcp_policy: "auto".to_string(),
                        last_opened_at: None,
                    }],
                    active_workspace_id: "ws-1".to_string(),
                    count: 1,
                    source: "command-bus".to_string(),
                }),
                ReadCommand::ApiListCollections { .. } => {
                    ReadCommandResult::ApiCollections(ApiCollectionListResult {
                        collections: vec![
                            ApiCollectionSummary {
                                id: "users".to_string(),
                                name: "Users".to_string(),
                                request_count: 3,
                                workspace_id: "ws-1".to_string(),
                            },
                        ],
                        count: 1,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListRequests { .. } => {
                    ReadCommandResult::ApiRequests(ApiRequestListResult {
                        requests: vec![ApiRequestSummary {
                            id: "req-1".to_string(),
                            name: "Get Users".to_string(),
                            method: "GET".to_string(),
                            url_preview: "https://api.example.com/users?token=secret123&page=1".to_string(),
                            collection_id: "users".to_string(),
                            workspace_id: "ws-1".to_string(),
                            has_body: false,
                            header_count: 2,
                        }],
                        count: 1,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiGetRequest { request_id } => {
                    ReadCommandResult::ApiRequest(ApiRequestDetailResult {
                        request: ApiSavedRequest {
                            id: request_id,
                            workspace_id: "ws-1".to_string(),
                            name: "Create User".to_string(),
                            collection_id: "users".to_string(),
                            parent_folder_id: Some("folder-users".to_string()),
                            sort_order: 0,
                            auth_json: r#"{"type":"none"}"#.to_string(),
                            method: "POST".to_string(),
                            url: "https://api.example.com/users?api_key=secret".to_string(),
                            headers_json: r#"[{"key":"Authorization","value":"Bearer secret-token","enabled":true},{"key":"Content-Type","value":"application/json","enabled":true}]"#.to_string(),
                            query_json: r#"[{"key":"page","value":"1","enabled":true},{"key":"token","value":"secret","enabled":true}]"#.to_string(),
                            body: Some(r#"{"name":"test","password":"secret123"}"#.to_string()),
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
                        history: vec![ApiHistoryItem {
                            id: "hist-1".to_string(),
                            workspace_id: "ws-1".to_string(),
                            name: Some("Get Users".to_string()),
                            method: "GET".to_string(),
                            url: "https://api.example.com/users?token=secret123&page=2".to_string(),
                            status: Some(500),
                            duration_ms: Some(87),
                            created_at: "2026-06-20T00:00:00Z".to_string(),
                            updated_at: "2026-06-20T00:00:00Z".to_string(),
                        }],
                        count: 1,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiGetHistory { history_id, .. } => {
                    ReadCommandResult::ApiHistoryDetailResult(ApiHistoryDetailResult {
                        detail: ApiHistoryDetail {
                            id: history_id,
                            workspace_id: "ws-1".to_string(),
                            name: Some("Create User".to_string()),
                            method: "POST".to_string(),
                            url: "https://api.example.com/users?api_key=secret".to_string(),
                            request_headers_json: r#"[{"key":"Authorization","value":"Bearer secret-token","enabled":true}]"#.to_string(),
                            request_query_json: r#"[{"key":"token","value":"secret","enabled":true}]"#.to_string(),
                            request_body: Some(r#"{"name":"test","password":"secret123"}"#.to_string()),
                            status: Some(401),
                            duration_ms: Some(120),
                            response_headers_json: r#"[{"key":"Set-Cookie","value":"session=secret-session-id","enabled":true}]"#.to_string(),
                            response_body_preview: Some(r#"{"error":"unauthorized","token":"secret-jwt"}"#.to_string()),
                            created_at: "2026-06-20T00:00:00Z".to_string(),
                            updated_at: "2026-06-20T00:00:00Z".to_string(),
                        },
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListEnvironments { .. } => {
                    ReadCommandResult::ApiEnvironments(ApiEnvironmentListResult {
                        environments: vec![ApiEnvironment {
                            id: "env-1".to_string(),
                            workspace_id: "ws-1".to_string(),
                            name: "Staging".to_string(),
                            variables: vec![
                                KeyValue {
                                    key: "baseUrl".to_string(),
                                    value: "https://api.staging.example.com".to_string(),
                                    enabled: true,
                                },
                                KeyValue {
                                    key: "token".to_string(),
                                    value: "Bearer secret-token".to_string(),
                                    enabled: true,
                                },
                            ],
                            is_active: true,
                            created_at: String::new(),
                            updated_at: String::new(),
                        }],
                        count: 1,
                        source: "command-bus".to_string(),
                    })
                }
                _ => ReadCommandResult::ApiCollections(ApiCollectionListResult {
                    collections: vec![],
                    count: 0,
                    source: "command-bus".to_string(),
                }),
            })
    }

    fn execute_saved_api_request(
        &self,
        _request_id: &str,
        _timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        Ok(ApiResponse {
            history_id: "hist-1".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![
                KeyValue {
                    key: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                },
                KeyValue {
                    key: "Set-Cookie".to_string(),
                    value: "session=secret-session-id".to_string(),
                    enabled: true,
                },
            ],
            body: r#"{"ok":true,"token":"secret-jwt"}"#.to_string(),
            duration_ms: 123,
        })
    }

    fn send_api_request(
        &self,
        input: ApiRequestInput,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        assert_eq!(input.method, "POST");
        assert_eq!(input.workspace_id, "ws-1");
        Ok(ApiResponse {
            history_id: "hist-post".to_string(),
            status: 201,
            status_text: "Created".to_string(),
            headers: vec![KeyValue {
                key: "Set-Cookie".to_string(),
                value: "session=secret-session-id".to_string(),
                enabled: true,
            }],
            body: r#"{"id":1,"token":"secret-jwt"}"#.to_string(),
            duration_ms: 77,
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

struct FailingApiCommandBus;

impl CommandBusAdapter for FailingApiCommandBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        match command {
            ReadCommand::CurrentWorkspace => Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "ws-1".to_string(),
                    workspace_name: "API Workspace".to_string(),
                    environment_type: "dev".to_string(),
                    mcp_policy: "auto".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            )),
            ReadCommand::ListWorkspaces => Ok(ReadCommandResult::Workspaces(WorkspaceListResult {
                workspaces: vec![WorkspaceSummary {
                    id: "ws-1".to_string(),
                    name: "API Workspace".to_string(),
                    is_default: true,
                    is_active: true,
                    environment_type: "dev".to_string(),
                    mcp_policy: "auto".to_string(),
                    last_opened_at: None,
                }],
                active_workspace_id: "ws-1".to_string(),
                count: 1,
                source: "command-bus".to_string(),
            })),
            ReadCommand::ApiGetRequest { request_id } => {
                Ok(ReadCommandResult::ApiRequest(ApiRequestDetailResult {
                    request: ApiSavedRequest {
                        id: request_id,
                        workspace_id: "ws-1".to_string(),
                        name: "Create User".to_string(),
                        collection_id: "users".to_string(),
                        parent_folder_id: None,
                        sort_order: 0,
                        auth_json: r#"{"type":"none"}"#.to_string(),
                        method: "POST".to_string(),
                        url: "https://api.example.com/users".to_string(),
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
                }))
            }
            _ => Err(CommandBusAdapterError {
                code: "COMMAND_BUS_READ_FAILED",
                message: "The command-bus read operation failed.",
            }),
        }
    }

    fn execute_saved_api_request(
        &self,
        _request_id: &str,
        _timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_API_SEND_FAILED",
            message: "The command-bus API send operation failed.",
        })
    }

    fn list_db_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_DB_LIST_FAILED",
            message: "The command-bus database list operation failed.",
        })
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_DB_SCHEMA_FAILED",
            message: "The command-bus database schema operation failed.",
        })
    }

    fn execute_db_query(
        &self,
        _input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_DB_QUERY_FAILED",
            message: "The command-bus database query operation failed.",
        })
    }
}

fn api_registry() -> ToolRegistry {
    ToolRegistry::with_command_bus(Arc::new(ApiStubCommandBus))
}

#[path = "api_tests/environment.rs"]
mod environment;
#[path = "api_tests/history.rs"]
mod history;
#[path = "api_tests/requests.rs"]
mod requests;
#[path = "api_tests/schema.rs"]
mod schema;
#[path = "api_tests/send_request.rs"]
mod send_request;
#[path = "api_tests/utilities.rs"]
mod utilities;
