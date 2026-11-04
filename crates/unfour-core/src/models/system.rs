use serde::{Deserialize, Serialize};

/// An opaque local commit clock; never includes business content or secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDataVersion {
    pub workspace_id: String,
    pub domain: String,
    // String avoids losing integer precision in JavaScript.
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    pub app_name: String,
    pub storage_ready: bool,
    pub command_bus_ready: bool,
    pub ai_reserved_capabilities: Vec<String>,
}
