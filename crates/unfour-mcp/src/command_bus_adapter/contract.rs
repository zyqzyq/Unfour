use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiCollection, ApiEnvironment, ApiRequestInput, ApiResponse, ApiSavedRequest,
    CredentialCreateInput, CredentialMetadata, DatabaseConnection, DatabaseConnectionInput,
    DatabaseExportTableInput, DatabaseExportTableResult, DatabaseQueryInput, DatabaseQueryResult,
    DatabaseSchema, DatabaseTableList, DatabaseTableStructure, DatabaseTableStructureInput,
    DatabaseTestResult, KeyValue, SshCommandHistoryEntry, SshCommandHistoryQuery, SshConnection,
    SshConnectionInput, SshDiagnosticInput, SshDiagnosticResult, SshTask, SshTaskCancelInput,
    SshTaskCleanupInput, SshTaskCleanupResult, SshTaskDetail, SshTaskRun, SshTaskRunInput,
    SshTaskSaveInput, SshTasksReorderInput, SystemHealth, WorkspaceEnvironment,
    WorkspaceEnvironmentVariable, WorkspaceVariable, WorkspaceVariableInput,
};
use unfour_core::models::{
    FlowDefinition, FlowRun, FlowRunInput, FlowRunPage, FlowRunSummary, FlowSummary,
};
use unfour_core::models::{SshFilePatchInput, SshFileWriteInput};

use super::CommandBusAdapterError;
use unfour_core::models::{ApiCollectionFolder, SavedSql, SavedSqlInput, Workspace};

pub trait CommandBusAdapter: Send + Sync {
    fn check_redis_key(
        &self,
        _input: super::RedisCheck<'_>,
    ) -> Result<unfour_command_bus::RedisCheckResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Redis checks are unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn api_folder_delete_snapshot(
        &self,
        _workspace_id: String,
        _folder_id: String,
    ) -> Result<unfour_core::models::ApiFolderDeleteSnapshot, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Folder deletion preview is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn create_workspace_inactive(
        &self,
        name: String,
        source_id: String,
        environment_type: String,
        mcp_policy: String,
    ) -> Result<Workspace, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn rename_workspace(
        &self,
        workspace_id: String,
        name: String,
        expected_name: String,
    ) -> Result<Workspace, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn list_api_folders(
        &self,
        workspace_id: String,
        collection_id: Option<String>,
    ) -> Result<Vec<ApiCollectionFolder>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn create_api_folder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        name: String,
    ) -> Result<ApiCollectionFolder, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn rename_api_folder(
        &self,
        workspace_id: String,
        folder_id: String,
        name: String,
        expected_updated_at: String,
    ) -> Result<ApiCollectionFolder, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn move_api_folder(
        &self,
        workspace_id: String,
        folder_id: String,
        parent_folder_id: Option<String>,
    ) -> Result<ApiCollectionFolder, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn delete_api_folder(
        &self,
        workspace_id: String,
        folder_id: String,
        expected: unfour_core::models::ApiFolderDeleteSnapshot,
    ) -> Result<Vec<ApiCollectionFolder>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn reorder_api_folders(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        folder_ids: Vec<String>,
    ) -> Result<Vec<ApiCollectionFolder>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn move_api_request(
        &self,
        workspace_id: String,
        request_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn duplicate_api_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn list_saved_sql(
        &self,
        workspace_id: String,
    ) -> Result<Vec<SavedSql>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn save_saved_sql(
        &self,
        input: SavedSqlInput,
        expected_revision: Option<i64>,
    ) -> Result<SavedSql, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn delete_saved_sql(
        &self,
        workspace_id: String,
        id: String,
        expected_revision: i64,
    ) -> Result<Vec<SavedSql>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    #[allow(unused_variables)]
    fn list_database_catalogs(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> Result<Vec<String>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Metadata operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn list_flows(&self, _workspace_id: &str) -> Result<Vec<FlowSummary>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn get_flow(
        &self,
        _workspace_id: &str,
        _flow_id: &str,
    ) -> Result<FlowDefinition, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn save_flow(&self, _input: FlowDefinition) -> Result<FlowDefinition, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn delete_flow(
        &self,
        _workspace_id: &str,
        _flow_id: &str,
        _expected_revision: i64,
    ) -> Result<(), CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn run_flow(
        &self,
        _input: FlowRunInput,
        _expected_revision: i64,
    ) -> Result<FlowRun, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn cancel_flow_run(
        &self,
        _workspace_id: &str,
        _run_id: &str,
    ) -> Result<FlowRun, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn list_flow_runs(
        &self,
        _workspace_id: &str,
        _flow_id: &str,
    ) -> Result<Vec<FlowRunSummary>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn list_flow_runs_page(
        &self,
        _workspace_id: &str,
        _flow_id: &str,
        _limit: u32,
        _cursor: Option<&str>,
    ) -> Result<FlowRunPage, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn get_flow_run(
        &self,
        _workspace_id: &str,
        _run_id: &str,
    ) -> Result<FlowRun, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Flow operation is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn list_db_history(
        &self,
        _workspace_id: &str,
        _limit: i64,
    ) -> Result<Vec<unfour_core::models::DbQueryHistoryEntry>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Database history is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn delete_db_connection(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<(), CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Database connection deletion is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn delete_ssh_connection(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<(), CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "SSH connection deletion is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn test_ssh_connection(
        &self,
        _input: SshConnectionInput,
    ) -> Result<unfour_core::models::SshTestResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "SSH connection testing is unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn get_ssh_host_key(
        &self,
        _input: unfour_core::models::SshHostKeyInput,
    ) -> Result<Option<unfour_core::models::SshHostFingerprintInfo>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "SSH host fingerprints are unavailable.",
            details: serde_json::json!({}),
        })
    }
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError>;

    fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError>;

    /// Replay exactly the saved definition that the MCP policy evaluated.
    fn execute_saved_api_request_snapshot(
        &self,
        _snapshot: &ApiSavedRequest,
        _timeout_ms: Option<u64>,
        _environment_id: Option<&str>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support saved API request snapshots.",
            details: serde_json::json!({}),
        })
    }

    fn execute_saved_api_request_in_workspace(
        &self,
        _workspace_id: Option<&str>,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.execute_saved_api_request(request_id, timeout_ms)
    }

    fn execute_saved_api_request_with_scripts_in_workspace(
        &self,
        workspace_id: Option<&str>,
        request_id: &str,
        timeout_ms: Option<u64>,
        environment_id: Option<&str>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        if environment_id.is_some() {
            return Err(CommandBusAdapterError {
                code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
                message: "This command-bus adapter does not support per-call API environments.",
                details: serde_json::json!({}),
            });
        }
        self.execute_saved_api_request_in_workspace(workspace_id, request_id, timeout_ms)
    }

    fn send_api_request(
        &self,
        _input: ApiRequestInput,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support ad-hoc API sends.",
            details: serde_json::json!({}),
        })
    }

    fn send_api_request_in_environment(
        &self,
        input: ApiRequestInput,
        environment_id: Option<&str>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        if environment_id.is_some() {
            return Err(CommandBusAdapterError {
                code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
                message: "This command-bus adapter does not support per-call API environments.",
                details: serde_json::json!({}),
            });
        }
        self.send_api_request(input)
    }

    fn save_api_request(
        &self,
        _input: ApiRequestInput,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API request saves.",
            details: serde_json::json!({}),
        })
    }

    fn update_api_request(
        &self,
        _workspace_id: &str,
        _request_id: &str,
        _input: ApiRequestInput,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API request updates.",
            details: serde_json::json!({}),
        })
    }

    fn update_api_request_if_current(
        &self,
        _workspace_id: &str,
        _request_id: &str,
        _input: ApiRequestInput,
        _expected_revision: i64,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message:
                "This command-bus adapter does not support revision-checked API request updates.",
            details: serde_json::json!({}),
        })
    }

    fn delete_api_request(
        &self,
        _workspace_id: &str,
        _request_id: &str,
    ) -> Result<Vec<ApiSavedRequest>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API request deletion.",
            details: serde_json::json!({}),
        })
    }

    fn create_api_collection(
        &self,
        _workspace_id: &str,
        _name: &str,
    ) -> Result<ApiCollection, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API collection creation.",
            details: serde_json::json!({}),
        })
    }

    fn update_api_collection(
        &self,
        _workspace_id: &str,
        _collection_id: &str,
        _name: &str,
    ) -> Result<ApiCollection, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API collection updates.",
            details: serde_json::json!({}),
        })
    }

    fn delete_api_collection(
        &self,
        _workspace_id: &str,
        _collection_id: &str,
    ) -> Result<Vec<ApiCollection>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API collection deletion.",
            details: serde_json::json!({}),
        })
    }

    fn create_api_environment(
        &self,
        _workspace_id: &str,
        _name: &str,
    ) -> Result<ApiEnvironment, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API environment creation.",
            details: serde_json::json!({}),
        })
    }

    fn update_api_environment(
        &self,
        _workspace_id: &str,
        _environment_id: &str,
        _name: &str,
        _variables: Vec<KeyValue>,
    ) -> Result<ApiEnvironment, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API environment updates.",
            details: serde_json::json!({}),
        })
    }

    fn delete_api_environment(
        &self,
        _workspace_id: &str,
        _environment_id: &str,
    ) -> Result<Vec<ApiEnvironment>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API environment deletion.",
            details: serde_json::json!({}),
        })
    }

    fn list_workspace_environments(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<WorkspaceEnvironment>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support workspace environment reads.",
            details: serde_json::json!({}),
        })
    }

    fn create_api_environment_variable(
        &self,
        _workspace_id: &str,
        _environment_id: &str,
        _input: WorkspaceVariableInput,
    ) -> Result<WorkspaceEnvironmentVariable, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API environment variable creation.",
            details: serde_json::json!({}),
        })
    }

    fn update_api_environment_variable(
        &self,
        _workspace_id: &str,
        _environment_id: &str,
        _variable_id: &str,
        _input: WorkspaceVariableInput,
    ) -> Result<WorkspaceEnvironmentVariable, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API environment variable updates.",
            details: serde_json::json!({}),
        })
    }

    fn delete_api_environment_variable(
        &self,
        _workspace_id: &str,
        _environment_id: &str,
        _variable_id: &str,
    ) -> Result<Vec<WorkspaceEnvironmentVariable>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API environment variable deletion.",
            details: serde_json::json!({}),
        })
    }

    fn list_workspace_variables(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<WorkspaceVariable>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support workspace variable reads.",
            details: serde_json::json!({}),
        })
    }

    fn replace_workspace_variables(
        &self,
        _workspace_id: &str,
        _variables: Vec<WorkspaceVariableInput>,
    ) -> Result<Vec<WorkspaceVariable>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support workspace variable replacement.",
            details: serde_json::json!({}),
        })
    }

    fn create_workspace_variable(
        &self,
        _workspace_id: &str,
        _input: WorkspaceVariableInput,
    ) -> Result<WorkspaceVariable, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support workspace variable creation.",
            details: serde_json::json!({}),
        })
    }

    fn update_workspace_variable(
        &self,
        _workspace_id: &str,
        _variable_id: &str,
        _input: WorkspaceVariableInput,
    ) -> Result<WorkspaceVariable, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support workspace variable updates.",
            details: serde_json::json!({}),
        })
    }

    fn delete_workspace_variable(
        &self,
        _workspace_id: &str,
        _variable_id: &str,
    ) -> Result<Vec<WorkspaceVariable>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support workspace variable deletion.",
            details: serde_json::json!({}),
        })
    }

    fn list_db_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError>;

    fn save_db_connection(
        &self,
        _input: DatabaseConnectionInput,
    ) -> Result<DatabaseConnection, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support database connection saves.",
            details: serde_json::json!({}),
        })
    }

    fn create_credential(
        &self,
        _input: CredentialCreateInput,
    ) -> Result<CredentialMetadata, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support credential creation.",
            details: serde_json::json!({}),
        })
    }

    fn get_db_schema(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError>;

    /// List tables in an optional catalog.
    ///
    /// `catalog == None` falls back to [`Self::get_db_schema`]. An explicit
    /// catalog returns an unsupported error unless the adapter overrides this
    /// method. `LocalCommandBusAdapter` passes `catalog` through to the
    /// database engine.
    fn get_db_schema_for_catalog(
        &self,
        workspace_id: &str,
        connection_id: &str,
        catalog: Option<&str>,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        if catalog.is_some() {
            return Err(CommandBusAdapterError {
                code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
                message:
                    "This command-bus adapter does not support catalog-aware database schema reads.",
                details: serde_json::json!({}),
            });
        }
        self.get_db_schema(workspace_id, connection_id)
    }

    fn list_db_tables(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
        _catalog: Option<&str>,
        _schema: Option<&str>,
        _limit: u32,
    ) -> Result<DatabaseTableList, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Table listing is unavailable.",
            details: serde_json::json!({}),
        })
    }

    fn get_db_table_structure(
        &self,
        _input: DatabaseTableStructureInput,
    ) -> Result<DatabaseTableStructure, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Table structure is unavailable.",
            details: serde_json::json!({}),
        })
    }

    fn export_db_table(
        &self,
        _input: DatabaseExportTableInput,
    ) -> Result<DatabaseExportTableResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "Table export is unavailable.",
            details: serde_json::json!({}),
        })
    }

    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError>;

    /// Test connectivity to a saved database connection. Diagnostic action with
    /// a side effect (opens a connection), so it is not a `ReadCommand`. Adapters
    /// that cannot test connections may use the default unsupported response.
    fn test_db_connection(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseTestResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support connection testing.",
            details: serde_json::json!({}),
        })
    }

    /// Return command-bus / storage health for diagnostics.
    fn system_health(&self) -> Result<SystemHealth, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support system health reads.",
            details: serde_json::json!({}),
        })
    }

    /// Run a read-only, allowlist-validated SSH diagnostic command. Diagnostic
    /// action with a side effect (opens a connection and executes a command), so
    /// it is not a `ReadCommand`. Adapters that cannot run diagnostics may use
    /// the default unsupported response.
    fn run_ssh_diagnostic(
        &self,
        _input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH diagnostics.",
            details: serde_json::json!({}),
        })
    }

    fn list_ssh_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<SshConnection>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH connection listing.",
            details: serde_json::json!({}),
        })
    }

    fn list_ssh_command_history(
        &self,
        _query: SshCommandHistoryQuery,
    ) -> Result<Vec<SshCommandHistoryEntry>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH command history reads.",
            details: serde_json::json!({}),
        })
    }

    fn save_ssh_connection(
        &self,
        _input: SshConnectionInput,
    ) -> Result<SshConnection, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH connection saves.",
            details: serde_json::json!({}),
        })
    }

    fn write_ssh_file(
        &self,
        _input: SshFileWriteInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "SSH file operation is unavailable.",
            details: serde_json::json!({}),
        })
    }

    fn patch_ssh_file(
        &self,
        _input: SshFilePatchInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "SSH file operation is unavailable.",
            details: serde_json::json!({}),
        })
    }

    fn run_ssh_command(
        &self,
        _input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH command execution.",
            details: serde_json::json!({}),
        })
    }

    fn list_ssh_tasks(&self, _workspace_id: &str) -> Result<Vec<SshTask>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task reads.",
            details: serde_json::json!({}),
        })
    }

    fn reorder_ssh_tasks(
        &self,
        _input: SshTasksReorderInput,
    ) -> Result<Vec<SshTask>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task reordering.",
            details: serde_json::json!({}),
        })
    }

    fn get_ssh_task(
        &self,
        _workspace_id: &str,
        _task_id: &str,
    ) -> Result<SshTaskDetail, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task detail reads.",
            details: serde_json::json!({}),
        })
    }

    fn save_ssh_task(
        &self,
        _input: SshTaskSaveInput,
    ) -> Result<SshTaskDetail, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task saves.",
            details: serde_json::json!({}),
        })
    }

    fn duplicate_ssh_task(
        &self,
        _workspace_id: &str,
        _task_id: &str,
    ) -> Result<SshTaskDetail, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task duplication.",
            details: serde_json::json!({}),
        })
    }

    fn delete_ssh_task(
        &self,
        _workspace_id: &str,
        _task_id: &str,
    ) -> Result<(), CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task deletion.",
            details: serde_json::json!({}),
        })
    }

    fn run_ssh_task(&self, _input: SshTaskRunInput) -> Result<SshTaskRun, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task execution.",
            details: serde_json::json!({}),
        })
    }

    fn cancel_ssh_task_run(
        &self,
        _input: SshTaskCancelInput,
    ) -> Result<SshTaskRun, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task cancellation.",
            details: serde_json::json!({}),
        })
    }

    fn list_ssh_task_runs(
        &self,
        _workspace_id: &str,
        _task_id: &str,
    ) -> Result<Vec<SshTaskRun>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task run reads.",
            details: serde_json::json!({}),
        })
    }

    fn read_ssh_task_run_log(
        &self,
        _workspace_id: &str,
        _run_id: &str,
    ) -> Result<String, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task log reads.",
            details: serde_json::json!({}),
        })
    }

    fn clear_ssh_task_runs(
        &self,
        _input: SshTaskCleanupInput,
    ) -> Result<SshTaskCleanupResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task run cleanup.",
            details: serde_json::json!({}),
        })
    }
}
