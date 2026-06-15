use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};
use unfour_command_bus::{CommandBus, ReadCommand, ReadCommandResult};

pub trait CommandBusAdapter: Send + Sync {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError>;
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
    pub fn ephemeral() -> Result<Arc<dyn CommandBusAdapter>, CommandBusAdapterError> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| CommandBusAdapterError::initialization_failed())?;
        let bus = runtime
            .block_on(CommandBus::ephemeral())
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
    }
}
