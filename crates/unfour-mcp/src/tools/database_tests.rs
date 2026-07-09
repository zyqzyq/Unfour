use std::sync::Arc;

use serde_json::json;
use unfour_command_bus::{
    ConnectionListResult, CurrentWorkspaceResult, ReadCommand, ReadCommandResult,
    WorkspaceListResult, WorkspaceSummary,
};
use unfour_core::models::{
    ApiResponse, CredentialCreateInput, CredentialMetadata, DatabaseConnection,
    DatabaseConnectionInput, DatabaseQueryInput, DatabaseQueryResult, DatabaseQuerySafety,
    DatabaseResultColumn, DatabaseSchema, DatabaseTable, DatabaseTableColumn, DatabaseTestResult,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use crate::tools::ToolRegistry;

// --- Stub implementations ---

struct DbStubCommandBus;

impl CommandBusAdapter for DbStubCommandBus {
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
                    mcp_policy: "guarded".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                })
            }
            ReadCommand::ListWorkspaces => ReadCommandResult::Workspaces(WorkspaceListResult {
                workspaces: vec![WorkspaceSummary {
                    id: "workspace-1".to_string(),
                    name: "Workspace".to_string(),
                    is_default: true,
                    is_active: true,
                    environment_type: "dev".to_string(),
                    mcp_policy: "guarded".to_string(),
                    last_opened_at: None,
                }],
                active_workspace_id: "workspace-1".to_string(),
                count: 1,
                source: "command-bus".to_string(),
            }),
            ReadCommand::ListConnections { .. } => {
                ReadCommandResult::Connections(ConnectionListResult {
                    connections: vec![],
                    count: 0,
                    source: "command-bus".to_string(),
                })
            }
            _ => ReadCommandResult::CurrentWorkspace(CurrentWorkspaceResult {
                workspace_id: "workspace-1".to_string(),
                workspace_name: "Workspace".to_string(),
                environment_type: "dev".to_string(),
                mcp_policy: "guarded".to_string(),
                workspace_root: None,
                mode: "local".to_string(),
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
            history_id: "history-1".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![],
            body: "{}".to_string(),
            duration_ms: 0,
        })
    }

    fn list_db_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        Ok(vec![DatabaseConnection {
            id: "conn-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            name: "Dev Database".to_string(),
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            database: Some("app_dev".to_string()),
            username: Some("admin".to_string()),
            ssl_mode: None,
            sqlite_path: None,
            credential_ref: Some("secret-ref-123".to_string()),
            read_only: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            deleted_at: None,
            revision: 1,
            sync_status: "local".to_string(),
            remote_id: None,
        }])
    }

    fn create_credential(
        &self,
        input: CredentialCreateInput,
    ) -> Result<CredentialMetadata, CommandBusAdapterError> {
        Ok(CredentialMetadata {
            workspace_id: input.workspace_id,
            kind: input.kind,
            label: input.label,
            credential_ref: "unfour:workspace-1:database-password:cred-1".to_string(),
        })
    }

    fn save_db_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> Result<DatabaseConnection, CommandBusAdapterError> {
        Ok(DatabaseConnection {
            id: "created-db-1".to_string(),
            workspace_id: input.workspace_id,
            name: input.name,
            driver: input.driver,
            host: input.host,
            port: input.port,
            database: input.database,
            username: input.username,
            ssl_mode: input.ssl_mode,
            sqlite_path: input.sqlite_path,
            credential_ref: input.credential_ref,
            read_only: input.read_only,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            deleted_at: None,
            revision: 1,
            sync_status: "local".to_string(),
            remote_id: None,
        })
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        Ok(DatabaseSchema {
            connection_id: connection_id.to_string(),
            tables: vec![
                DatabaseTable {
                    catalog: None,
                    schema: Some("public".to_string()),
                    name: "users".to_string(),
                    kind: "table".to_string(),
                    columns: vec![
                        DatabaseTableColumn {
                            name: "id".to_string(),
                            data_type: "integer".to_string(),
                            nullable: false,
                            primary_key: true,
                            default_value: None,
                        },
                        DatabaseTableColumn {
                            name: "email".to_string(),
                            data_type: "varchar".to_string(),
                            nullable: false,
                            primary_key: false,
                            default_value: None,
                        },
                        DatabaseTableColumn {
                            name: "created_at".to_string(),
                            data_type: "timestamp".to_string(),
                            nullable: true,
                            primary_key: false,
                            default_value: None,
                        },
                    ],
                },
                DatabaseTable {
                    catalog: None,
                    schema: Some("public".to_string()),
                    name: "orders".to_string(),
                    kind: "table".to_string(),
                    columns: vec![DatabaseTableColumn {
                        name: "id".to_string(),
                        data_type: "integer".to_string(),
                        nullable: false,
                        primary_key: true,
                        default_value: None,
                    }],
                },
                DatabaseTable {
                    catalog: None,
                    schema: Some("analytics".to_string()),
                    name: "events".to_string(),
                    kind: "view".to_string(),
                    columns: vec![],
                },
                DatabaseTable {
                    catalog: None,
                    schema: Some("analytics".to_string()),
                    name: "summary".to_string(),
                    kind: "table".to_string(),
                    columns: vec![],
                },
                DatabaseTable {
                    catalog: None,
                    schema: Some("audit".to_string()),
                    name: "logs".to_string(),
                    kind: "table".to_string(),
                    columns: vec![],
                },
            ],
        })
    }

    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        let keyword = input
            .sql
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let is_read = matches!(keyword.as_str(), "select" | "with" | "explain" | "show");
        Ok(DatabaseQueryResult {
            columns: if is_read {
                vec![
                    DatabaseResultColumn {
                        name: "id".to_string(),
                        data_type: "integer".to_string(),
                    },
                    DatabaseResultColumn {
                        name: "email".to_string(),
                        data_type: "varchar".to_string(),
                    },
                ]
            } else {
                vec![]
            },
            rows: if is_read {
                vec![
                    vec![Some("1".to_string()), Some("user@example.com".to_string())],
                    vec![Some("2".to_string()), Some("other@example.com".to_string())],
                ]
            } else {
                vec![]
            },
            affected_rows: if is_read { 0 } else { 2 },
            duration_ms: 42,
            safety: DatabaseQuerySafety {
                classification: if is_read { "read" } else { "mutation" }.to_string(),
                requires_confirmation: !is_read,
                confirmed: is_read || input.confirm_mutation == Some(true),
                message: None,
            },
        })
    }

    fn test_db_connection(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseTestResult, CommandBusAdapterError> {
        Ok(DatabaseTestResult {
            ok: true,
            message: "Connection successful".to_string(),
            server_version: Some("PostgreSQL 16.1".to_string()),
        })
    }
}

struct DbFailingCommandBus;

impl CommandBusAdapter for DbFailingCommandBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        match command {
            ReadCommand::CurrentWorkspace => Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "workspace-1".to_string(),
                    workspace_name: "Workspace".to_string(),
                    environment_type: "dev".to_string(),
                    mcp_policy: "auto".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            )),
            ReadCommand::ListWorkspaces => Ok(ReadCommandResult::Workspaces(WorkspaceListResult {
                workspaces: vec![WorkspaceSummary {
                    id: "workspace-1".to_string(),
                    name: "Workspace".to_string(),
                    is_default: true,
                    is_active: true,
                    environment_type: "dev".to_string(),
                    mcp_policy: "auto".to_string(),
                    last_opened_at: None,
                }],
                active_workspace_id: "workspace-1".to_string(),
                count: 1,
                source: "command-bus".to_string(),
            })),
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

fn registry() -> ToolRegistry {
    ToolRegistry::with_command_bus(Arc::new(DbStubCommandBus))
}

struct ProdDbStubCommandBus;

impl CommandBusAdapter for ProdDbStubCommandBus {
    fn execute_read(
        &self,
        _command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        Ok(ReadCommandResult::CurrentWorkspace(
            CurrentWorkspaceResult {
                workspace_id: "workspace-prod".to_string(),
                workspace_name: "Production".to_string(),
                environment_type: "prod".to_string(),
                mcp_policy: "auto".to_string(),
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
        unreachable!()
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        unreachable!()
    }

    fn execute_db_query(
        &self,
        _input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        panic!("prod write should be blocked before command-bus execution")
    }
}

#[path = "database_tests/connections.rs"]
mod connections;
#[path = "database_tests/execute.rs"]
mod execute;
#[path = "database_tests/query_readonly.rs"]
mod query_readonly;
#[path = "database_tests/schema.rs"]
mod schema;
#[path = "database_tests/test_connection.rs"]
mod test_connection;
