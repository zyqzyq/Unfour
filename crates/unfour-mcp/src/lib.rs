mod command_bus_adapter;
mod protocol;
mod response;
mod sanitize;
mod server;
mod shutdown;
mod tools;

pub const MCP_STORAGE_MODE_ENV: &str = "UNFOUR_MCP_STORAGE_MODE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Default,
    Ephemeral,
}

impl StorageMode {
    /// Recognize only the explicit registry/CI mode. Any other value keeps the
    /// normal persistent-storage behavior.
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value {
            Some("ephemeral") => Self::Ephemeral,
            _ => Self::Default,
        }
    }

    pub fn from_env() -> Self {
        Self::from_env_value(std::env::var(MCP_STORAGE_MODE_ENV).ok().as_deref())
    }
}

pub use command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError, LocalCommandBusAdapter};
pub use server::{
    run_stdio, run_stdio_with_adapter, run_stdio_with_adapter_and_idle_timeout, McpServer,
    SUPPORTED_PROTOCOL_VERSION,
};
pub use shutdown::Shutdown;
pub use tools::{ToolDefinition, ToolRegistry};
