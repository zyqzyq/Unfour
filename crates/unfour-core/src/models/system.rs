use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    pub app_name: String,
    pub storage_ready: bool,
    pub command_bus_ready: bool,
    pub ai_reserved_capabilities: Vec<String>,
    pub sync_strategy: String,
}
