mod contract;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::runtime::{Builder, Runtime};
use unfour_command_bus::{CommandBus, ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiCollection, ApiRequestInput, ApiResponse, ApiSavedRequest, CredentialCreateInput,
    CredentialMetadata, DatabaseConnection, DatabaseConnectionInput, DatabaseQueryInput,
    DatabaseQueryResult, DatabaseSchema, DatabaseTestResult, SshConnection, SshConnectionInput,
    SshDiagnosticInput, SshDiagnosticResult, SystemHealth,
};
use unfour_core::AppError;

pub use contract::CommandBusAdapter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBusAdapterError {
    pub code: &'static str,
    pub message: &'static str,
}

pub struct LocalCommandBusAdapter {
    // Wrapped in an `Option` behind a `Mutex` so `shutdown` can take
    // ownership of the runtime for a *bounded* shutdown. The default
    // `Runtime::Drop` blocks until every spawned task finishes, which is what
    // lets a lingering SSH supervisor / pool keep-alive hang the process after
    // the stdio loop ends. Taking it out lets us call `shutdown_timeout`.
    runtime: Mutex<Option<Runtime>>,
    bus: CommandBus,
}

type AdapterResult = Result<Arc<LocalCommandBusAdapter>, CommandBusAdapterError>;

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

        Ok(Arc::new(Self {
            runtime: Mutex::new(Some(runtime)),
            bus,
        }))
    }
}

impl LocalCommandBusAdapter {
    /// Run a future to completion on the internal multi-thread runtime. Every
    /// adapter method goes through this so the `Runtime` stays behind an
    /// interior-mutable `Option`, which lets [`LocalCommandBusAdapter::shutdown`]
    /// take ownership of it for a bounded shutdown.
    fn run<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime
            .lock()
            .expect("command-bus runtime lock poisoned")
            .as_ref()
            .expect("command-bus runtime already shut down")
            .block_on(future)
    }

    /// Cancel all background tokio tasks (SSH supervisors, DB/API pool
    /// keep-alives, fire-and-forget flush tasks) and stop the runtime, bounded
    /// so a stuck task can never block process exit. Idempotent: a second call
    /// is a no-op.
    ///
    /// This is the unified shutdown signal for the stdio process: it releases
    /// SSH sessions, database connections, and the API runtime before the
    /// adapter is dropped.
    pub fn shutdown(&self) {
        if let Some(runtime) = self
            .runtime
            .lock()
            .expect("command-bus runtime lock poisoned")
            .take()
        {
            runtime.shutdown_timeout(Duration::from_secs(2));
        }
    }
}

impl Drop for LocalCommandBusAdapter {
    fn drop(&mut self) {
        // Bounded backstop: if `shutdown` was never called (e.g. a panic
        // before the normal exit path), still guarantee the runtime cannot hang
        // the process on a lingering background task.
        if let Ok(guard) = self.runtime.get_mut() {
            if let Some(runtime) = guard.take() {
                runtime.shutdown_timeout(Duration::from_secs(2));
            }
        }
    }
}

impl CommandBusAdapter for LocalCommandBusAdapter {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        self.run(self.bus.execute_read(command)).map_err(|e| {
            CommandBusAdapterError::from_app_error("The command-bus read operation failed.", &e)
        })
    }

    fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.run(self.bus.execute_saved_api_request(request_id, timeout_ms))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API send operation failed.",
                    &e,
                )
            })
    }

    fn execute_saved_api_request_in_workspace(
        &self,
        workspace_id: Option<&str>,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.run(self.bus.execute_saved_api_request_in_workspace(
            workspace_id.map(str::to_string),
            request_id,
            timeout_ms,
        ))
        .map_err(|e| {
            CommandBusAdapterError::from_app_error("The command-bus API send operation failed.", &e)
        })
    }

    fn send_api_request(
        &self,
        input: ApiRequestInput,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.run(self.bus.send_api_request(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error("The command-bus API send operation failed.", &e)
        })
    }

    fn save_api_request(
        &self,
        input: ApiRequestInput,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.run(self.bus.save_api_request(input)).map_err(|e| {
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
        self.run(self.bus.update_api_request(
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
        self.run(
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
        self.run(
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
        self.run(self.bus.api_collection_rename(
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
        self.run(
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
        self.run(self.bus.list_database_connections(workspace_id.to_string()))
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
        self.run(self.bus.save_database_connection(input))
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
        self.run(self.bus.create_credential(input)).map_err(|e| {
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
        self.run(self.bus.database_schema(
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
        self.run(self.bus.execute_database_query(input))
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
        self.run(
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
        self.run(self.bus.system_health()).map_err(|e| {
            CommandBusAdapterError::from_app_error("The command-bus system health read failed.", &e)
        })
    }

    fn run_ssh_diagnostic(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.run(self.bus.run_ssh_diagnostic(input)).map_err(|e| {
            CommandBusAdapterError::from_ssh_app_error("The command-bus SSH diagnostic failed.", &e)
        })
    }

    fn list_ssh_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<SshConnection>, CommandBusAdapterError> {
        self.run(self.bus.list_ssh_connections(workspace_id.to_string()))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus SSH connection list operation failed.",
                    &e,
                )
            })
    }

    fn save_ssh_connection(
        &self,
        input: SshConnectionInput,
    ) -> Result<SshConnection, CommandBusAdapterError> {
        self.run(self.bus.save_ssh_connection(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH connection save operation failed.",
                &e,
            )
        })
    }

    fn run_ssh_command(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.run(self.bus.run_ssh_command(input)).map_err(|e| {
            CommandBusAdapterError::from_ssh_app_error("The command-bus SSH command failed.", &e)
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
