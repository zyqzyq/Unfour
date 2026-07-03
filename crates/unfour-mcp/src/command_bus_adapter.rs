use std::path::Path;
use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};
use unfour_command_bus::{CommandBus, ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema,
    DatabaseTestResult, SshDiagnosticInput, SshDiagnosticResult, SystemHealth,
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

    fn list_db_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError>;

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
                CommandBusAdapterError::from_app_error("The command-bus SSH diagnostic failed.", &e)
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

    fn initialization_failed() -> Self {
        Self {
            code: "COMMAND_BUS_INITIALIZATION_FAILED",
            message: "The command-bus adapter could not be initialized.",
        }
    }
}

#[cfg(test)]
mod tests {
    use unfour_command_bus::{ConnectionType, ReadCommand, ReadCommandResult};

    use super::LocalCommandBusAdapter;
    use unfour_command_bus::CommandBus;
    use unfour_core::models::SshConnectionInput;
    use unfour_local_storage::LocalDb;

    #[test]
    fn ephemeral_adapter_executes_real_command_bus_reads() {
        let adapter = LocalCommandBusAdapter::ephemeral().expect("create adapter");

        let workspace = adapter
            .execute_read(ReadCommand::CurrentWorkspace)
            .expect("read current workspace");
        let ReadCommandResult::CurrentWorkspace(workspace) = workspace else {
            panic!("expected current workspace result");
        };
        assert!(!workspace.workspace_id.is_empty());
        assert_eq!(workspace.source, "command-bus");

        let connections = adapter
            .execute_read(ReadCommand::ListConnections {
                connection_type: ConnectionType::All,
            })
            .expect("list connections");
        let ReadCommandResult::Connections(connections) = connections else {
            panic!("expected connection list result");
        };
        assert_eq!(connections.count, 0);
        assert_eq!(connections.source, "command-bus");

        // Database connections should also be listable through the adapter.
        let db_connections = adapter
            .list_db_connections(&workspace.workspace_id)
            .expect("list db connections");
        assert_eq!(db_connections.len(), 0);

        // System health should be readable through the real adapter.
        let health = adapter.system_health().expect("system health");
        assert!(health.command_bus_ready);
        assert!(health.storage_ready);
    }

    #[test]
    fn storage_dir_adapter_reads_persisted_connection_metadata() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime");
        let storage_dir = std::env::temp_dir().join(format!(
            "unfour-mcp-storage-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        runtime.block_on(async {
            let db_path = storage_dir.join(unfour_command_bus::DEFAULT_DATABASE_FILE);
            let db = LocalDb::connect_path(&db_path).await.expect("create db");
            db.migrate().await.expect("run migrations");
            let bus = CommandBus::from_db(db).await.expect("create bus");
            let state = bus.list_workspaces().await.expect("list workspaces");
            bus.save_ssh_connection(SshConnectionInput {
                id: None,
                workspace_id: state.active_workspace_id,
                name: "Manual SSH".to_string(),
                host: "ssh.example.test".to_string(),
                port: Some(22),
                username: "developer".to_string(),
                auth_kind: "password".to_string(),
                key_path: None,
                credential_ref: Some("ssh-secret".to_string()),
                secret: None,
            })
            .await
            .expect("save ssh connection");
        });

        let adapter =
            LocalCommandBusAdapter::from_storage_dir_read_only(&storage_dir).expect("open storage");
        let result = adapter
            .execute_read(ReadCommand::ListConnections {
                connection_type: ConnectionType::Ssh,
            })
            .expect("list ssh connections");
        let ReadCommandResult::Connections(result) = result else {
            panic!("expected connections");
        };

        assert_eq!(result.count, 1);
        assert_eq!(result.connections[0].name, "Manual SSH");
        assert_eq!(
            result.connections[0].safe_summary.host.as_deref(),
            Some("ssh.example.test")
        );
        let json = serde_json::to_string(&result).expect("serialize result");
        assert!(!json.contains("developer"));
        assert!(!json.contains("ssh-secret"));

        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn default_storage_database_path_matches_command_bus_default() {
        assert_eq!(
            LocalCommandBusAdapter::default_database_path().expect("adapter database path"),
            unfour_command_bus::default_database_path().expect("command bus database path")
        );
    }
}
