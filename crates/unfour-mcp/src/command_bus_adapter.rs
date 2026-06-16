use std::path::Path;
use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};
use unfour_command_bus::{CommandBus, ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema,
};

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

impl LocalCommandBusAdapter {
    pub fn app_data() -> Result<Arc<dyn CommandBusAdapter>, CommandBusAdapterError> {
        Self::from_command_bus_future(CommandBus::from_existing_app_data_read_only())
    }

    pub fn send_app_data() -> Result<Arc<dyn CommandBusAdapter>, CommandBusAdapterError> {
        Self::from_command_bus_future(CommandBus::from_existing_app_data())
    }

    pub fn from_app_data_dir(
        app_data_dir: impl AsRef<Path>,
    ) -> Result<Arc<dyn CommandBusAdapter>, CommandBusAdapterError> {
        Self::from_command_bus_future(CommandBus::from_existing_app_data_dir_read_only(
            app_data_dir,
        ))
    }

    pub fn ephemeral() -> Result<Arc<dyn CommandBusAdapter>, CommandBusAdapterError> {
        Self::from_command_bus_future(CommandBus::ephemeral())
    }

    fn from_command_bus_future<E>(
        command_bus: impl std::future::Future<Output = Result<CommandBus, E>>,
    ) -> Result<Arc<dyn CommandBusAdapter>, CommandBusAdapterError> {
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
            .map_err(|_| CommandBusAdapterError {
                code: "COMMAND_BUS_READ_FAILED",
                message: "The command-bus read operation failed.",
            })
    }

    fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.execute_saved_api_request(request_id, timeout_ms))
            .map_err(|_| CommandBusAdapterError {
                code: "COMMAND_BUS_API_SEND_FAILED",
                message: "The command-bus API send operation failed.",
            })
    }

    fn list_db_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.list_database_connections(workspace_id.to_string()))
            .map_err(|_| CommandBusAdapterError {
                code: "COMMAND_BUS_DB_LIST_FAILED",
                message: "The command-bus database list operation failed.",
            })
    }

    fn get_db_schema(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.database_schema(workspace_id.to_string(), connection_id.to_string()))
            .map_err(|_| CommandBusAdapterError {
                code: "COMMAND_BUS_DB_SCHEMA_FAILED",
                message: "The command-bus database schema operation failed.",
            })
    }

    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        self.runtime
            .block_on(self.bus.execute_database_query(input))
            .map_err(|_| CommandBusAdapterError {
                code: "COMMAND_BUS_DB_QUERY_FAILED",
                message: "The command-bus database query operation failed.",
            })
    }
}

impl CommandBusAdapterError {
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
    }

    #[test]
    fn app_data_adapter_reads_persisted_connection_metadata() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime");
        let app_data_dir = std::env::temp_dir().join(format!(
            "unfour-mcp-app-data-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        runtime.block_on(async {
            let db_path = app_data_dir.join("unfour-workspace.sqlite");
            let db = LocalDb::connect_path(&db_path).await.expect("create db");
            db.migrate().await.expect("migrate db");
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
            })
            .await
            .expect("save ssh connection");
        });

        let adapter =
            LocalCommandBusAdapter::from_app_data_dir(&app_data_dir).expect("open app data");
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

        let _ = std::fs::remove_dir_all(app_data_dir);
    }
}
