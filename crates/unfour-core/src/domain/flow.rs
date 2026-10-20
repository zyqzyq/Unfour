//! Definition-only boundary; runtime snapshots and history never cross it.
use super::*;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowDefinitionSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub revision: i64,
    pub definition: serde_json::Value,
}

impl fmt::Debug for FlowDefinitionSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowDefinitionSnapshot")
            .field("id", &self.id)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalFlowDefinitionUpsert {
    pub id: String,
    pub workspace_id: String,
    /// Full local definition shape; revision is assigned by the owning engine.
    pub definition: serde_json::Value,
}

impl fmt::Debug for ExternalFlowDefinitionUpsert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalFlowDefinitionUpsert")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "record", rename_all = "camelCase")]
pub enum ExternalFlowDefinitionApply {
    Upsert(ExternalFlowDefinitionUpsert),
    Delete(ExternalDelete),
}
