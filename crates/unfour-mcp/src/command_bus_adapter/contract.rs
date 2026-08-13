use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiCollection, ApiEnvironment, ApiRequestInput, ApiResponse, ApiSavedRequest,
    CredentialCreateInput, CredentialMetadata, DatabaseConnection, DatabaseConnectionInput,
    DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema, DatabaseTestResult, KeyValue,
    SshCommandHistoryEntry, SshCommandHistoryQuery, SshConnection, SshConnectionInput,
    SshDiagnosticInput, SshDiagnosticResult, SshTask, SshTaskCancelInput, SshTaskCleanupInput,
    SshTaskCleanupResult, SshTaskDetail, SshTaskRun, SshTaskRunInput, SshTaskSaveInput,
    SshTasksReorderInput, SystemHealth, WorkspaceVariable, WorkspaceVariableInput,
};

use super::CommandBusAdapterError;

pub trait CommandBusAdapter: Send + Sync {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError>;

    fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError>;

    fn execute_saved_api_request_in_workspace(
        &self,
        _workspace_id: Option<&str>,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.execute_saved_api_request(request_id, timeout_ms)
    }

    fn send_api_request(
        &self,
        _input: ApiRequestInput,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support ad-hoc API sends.",
        })
    }

    fn save_api_request(
        &self,
        _input: ApiRequestInput,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support API request saves.",
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
        })
    }

    fn list_workspace_variables(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<WorkspaceVariable>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support workspace variable reads.",
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
        })
    }

    fn create_credential(
        &self,
        _input: CredentialCreateInput,
    ) -> Result<CredentialMetadata, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support credential creation.",
        })
    }

    fn get_db_schema(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError>;

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
        })
    }

    /// Return command-bus / storage health for diagnostics.
    fn system_health(&self) -> Result<SystemHealth, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support system health reads.",
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
        })
    }

    fn list_ssh_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<SshConnection>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH connection listing.",
        })
    }

    fn list_ssh_command_history(
        &self,
        _query: SshCommandHistoryQuery,
    ) -> Result<Vec<SshCommandHistoryEntry>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH command history reads.",
        })
    }

    fn save_ssh_connection(
        &self,
        _input: SshConnectionInput,
    ) -> Result<SshConnection, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH connection saves.",
        })
    }

    fn run_ssh_command(
        &self,
        _input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH command execution.",
        })
    }

    fn list_ssh_tasks(&self, _workspace_id: &str) -> Result<Vec<SshTask>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task reads.",
        })
    }

    fn reorder_ssh_tasks(
        &self,
        _input: SshTasksReorderInput,
    ) -> Result<Vec<SshTask>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task reordering.",
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
        })
    }

    fn save_ssh_task(
        &self,
        _input: SshTaskSaveInput,
    ) -> Result<SshTaskDetail, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task saves.",
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
        })
    }

    fn run_ssh_task(&self, _input: SshTaskRunInput) -> Result<SshTaskRun, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task execution.",
        })
    }

    fn cancel_ssh_task_run(
        &self,
        _input: SshTaskCancelInput,
    ) -> Result<SshTaskRun, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task cancellation.",
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
        })
    }

    fn clear_ssh_task_runs(
        &self,
        _input: SshTaskCleanupInput,
    ) -> Result<SshTaskCleanupResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH task run cleanup.",
        })
    }
}
