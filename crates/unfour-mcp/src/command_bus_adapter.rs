use std::path::Path;
use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};
use unfour_command_bus::{CommandBus, ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiCollection, ApiRequestInput, ApiResponse, ApiSavedRequest, CredentialCreateInput,
    CredentialMetadata, DatabaseConnection, DatabaseConnectionInput, DatabaseQueryInput,
    DatabaseQueryResult, DatabaseSchema, DatabaseTestResult, SshConnection, SshDiagnosticInput,
    SshDiagnosticResult, SystemHealth,
};
use unfour_core::AppError;

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

    fn run_ssh_command(
        &self,
        _input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
            message: "This command-bus adapter does not support SSH command execution.",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBusAdapterError {
    pub code: &'static str,
    pub message: &'static str,
}

pub struct LocalCommandBusAdapter {
    runtime: Runtime,
    bus: CommandBus,
}

type AdapterResult = Result<Arc<dyn CommandBusAdapter>, CommandBusAdapterError>;

impl LocalCommandBusAdapter {
    pub fn default_storage_read_only() -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_default_storage_read_only())
    }

    pub fn default_storage() -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_default_storage())
    }

    pub fn default_database_path() -> Result<std::path::PathBuf, CommandBusAdapterError> {
        unfour_command_bus::default_database_path()
            .map_err(|_| CommandBusAdapterError::initialization_failed())
    }

    pub fn from_storage_dir_read_only(storage_dir: impl AsRef<Path>) -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_storage_dir_read_only(storage_dir))
    }

    pub fn ephemeral() -> AdapterResult {
        Self::from_command_bus_future(CommandBus::ephemeral())
    }

    fn from_command_bus_future<E>(
        command_bus: impl std::future::Future<Output = Result<CommandBus, E>>,
    ) -> AdapterResult {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| CommandBusAdapterError::initialization_failed())?;
        let bus = runtime
            .block_on(command_bus)
            .map_err(|_| CommandBusAdapterError::initialization_failed())?;

        Ok(Arc::new(Self { runtime, bus }))
    }
}

impl CommandBusAdapter for LocalCommandBusAdapter {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.execute_read(command))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error("The command-bus read operation failed.", &e)
            })
    }

    fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.execute_saved_api_request(request_id, timeout_ms))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API send operation failed.",
                    &e,
                )
            })
    }

    fn send_api_request(
        &self,
        input: ApiRequestInput,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.send_api_request(input))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API send operation failed.",
                    &e,
                )
            })
    }

    fn save_api_request(
        &self,
        input: ApiRequestInput,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.save_api_request(input))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API request save operation failed.",
                    &e,
                )
            })
    }

    fn update_api_request(
        &self,
        workspace_id: &str,
        request_id: &str,
        input: ApiRequestInput,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.update_api_request(
                workspace_id.to_string(),
                request_id.to_string(),
                input,
            ))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API request update operation failed.",
                    &e,
                )
            })
    }

    fn delete_api_request(
        &self,
        workspace_id: &str,
        request_id: &str,
    ) -> Result<Vec<ApiSavedRequest>, CommandBusAdapterError> {
        self.runtime
            .block_on(
                self.bus
                    .delete_api_request(workspace_id.to_string(), request_id.to_string()),
            )
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API request delete operation failed.",
                    &e,
                )
            })
    }

    fn create_api_collection(
        &self,
        workspace_id: &str,
        name: &str,
    ) -> Result<ApiCollection, CommandBusAdapterError> {
        self.runtime
            .block_on(
                self.bus
                    .api_collection_create(workspace_id.to_string(), name.to_string()),
            )
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API collection create operation failed.",
                    &e,
                )
            })
    }

    fn update_api_collection(
        &self,
        workspace_id: &str,
        collection_id: &str,
        name: &str,
    ) -> Result<ApiCollection, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.api_collection_rename(
                workspace_id.to_string(),
                collection_id.to_string(),
                name.to_string(),
            ))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API collection update operation failed.",
                    &e,
                )
            })
    }

    fn delete_api_collection(
        &self,
        workspace_id: &str,
        collection_id: &str,
    ) -> Result<Vec<ApiCollection>, CommandBusAdapterError> {
        self.runtime
            .block_on(
                self.bus
                    .api_collection_delete(workspace_id.to_string(), collection_id.to_string()),
            )
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API collection delete operation failed.",
                    &e,
                )
            })
    }

    fn list_db_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.list_database_connections(workspace_id.to_string()))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus database list operation failed.",
                    &e,
                )
            })
    }

    fn save_db_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> Result<DatabaseConnection, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.save_database_connection(input))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus database connection save failed.",
                    &e,
                )
            })
    }

    fn create_credential(
        &self,
        input: CredentialCreateInput,
    ) -> Result<CredentialMetadata, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.create_credential(input))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus credential create operation failed.",
                    &e,
                )
            })
    }

    fn get_db_schema(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.database_schema(
                workspace_id.to_string(),
                connection_id.to_string(),
                None,
            ))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus database schema operation failed.",
                    &e,
                )
            })
    }

    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.execute_database_query(input))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus database query operation failed.",
                    &e,
                )
            })
    }

    fn test_db_connection(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseTestResult, CommandBusAdapterError> {
        self.runtime
            .block_on(
                self.bus
                    .test_database_connection(workspace_id.to_string(), connection_id.to_string()),
            )
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus database connection test failed.",
                    &e,
                )
            })
    }

    fn system_health(&self) -> Result<SystemHealth, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.system_health())
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus system health read failed.",
                    &e,
                )
            })
    }

    fn run_ssh_diagnostic(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.run_ssh_diagnostic(input))
            .map_err(|e| {
                CommandBusAdapterError::from_ssh_app_error(
                    "The command-bus SSH diagnostic failed.",
                    &e,
                )
            })
    }

    fn list_ssh_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<SshConnection>, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.list_ssh_connections(workspace_id.to_string()))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus SSH connection list operation failed.",
                    &e,
                )
            })
    }

    fn run_ssh_command(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.run_ssh_command(input))
            .map_err(|e| {
                CommandBusAdapterError::from_ssh_app_error(
                    "The command-bus SSH command failed.",
                    &e,
                )
            })
    }
}

impl CommandBusAdapterError {
    /// Build an adapter error that surfaces the underlying `AppError`'s stable
    /// classification code (e.g. `NOT_FOUND`, `DATABASE_ERROR`,
    /// `UNSUPPORTED_OPERATION`) alongside a safe, operation-specific message.
    /// The `AppError` `Display` text is intentionally not propagated because it
    /// may embed hosts, DSNs, or other sensitive detail.
    fn from_app_error(message: &'static str, error: &AppError) -> Self {
        Self {
            code: error.code(),
            message,
        }
    }

    fn from_ssh_app_error(message: &'static str, error: &AppError) -> Self {
        let message = match error {
            AppError::Validation(reason) if reason.contains("control characters") => {
                "SSH command validation failed: control characters/newlines are not allowed."
            }
            AppError::Validation(reason) if reason.contains("4096") => {
                "SSH command validation failed: command exceeds 4096 characters."
            }
            AppError::Validation(_) => "SSH command validation failed before execution.",
            _ => message,
        };
        Self {
            code: error.code(),
            message,
        }
    }

    fn initialization_failed() -> Self {
        Self {
            code: "COMMAND_BUS_INITIALIZATION_FAILED",
            message: "The command-bus adapter could not be initialized.",
        }
    }
}

#[cfg(test)]
#[path = "command_bus_adapter_tests/mod.rs"]
mod command_bus_adapter_tests;
