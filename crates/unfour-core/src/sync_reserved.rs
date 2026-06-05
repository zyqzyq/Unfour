use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncStatus {
    Local,
    Pending,
    Synced,
    Conflict,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPolicy {
    pub strategy: String,
    pub conflict_resolution: String,
    pub scope: String,
}

pub fn default_policy() -> SyncPolicy {
    SyncPolicy {
        strategy: "local-first-reserved".to_string(),
        conflict_resolution: "keep-both-versions".to_string(),
        scope: "workspace".to_string(),
    }
}
