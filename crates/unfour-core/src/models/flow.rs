use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowDefinition {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub revision: i64,
    pub inputs: Vec<String>,
    pub steps: Vec<FlowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowStep {
    pub id: String,
    pub name: String,
    pub timeout_ms: u64,
    /// Absent means the next step; "$end" completes the run.
    pub next: Option<String>,
    #[serde(flatten)]
    pub node: FlowNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FlowNode {
    Action {
        action: FlowAction,
    },
    #[serde(rename_all = "camelCase")]
    Condition {
        predicate: FlowPredicate,
        if_true: String,
        if_false: String,
    },
    #[serde(rename_all = "camelCase")]
    Poll {
        probe: FlowAction,
        predicate: FlowPredicate,
        interval_ms: u64,
        max_attempts: u32,
    },
    #[serde(rename_all = "camelCase")]
    Wait {
        duration_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowAction {
    pub capability: FlowCapability,
    pub resource_id: String,
    pub connection_id: Option<String>,
    /// JSON values, {"$ref":"/steps/id/body/value"}, or ${/inputs/name} in text.
    pub arguments: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FlowCapability {
    Api,
    Ssh,
    Database,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowPredicate {
    pub left: Value,
    pub op: FlowOperator,
    pub right: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FlowOperator {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowRunInput {
    pub workspace_id: String,
    pub flow_id: String,
    /// None explicitly uses workspace variables without an active UI environment.
    pub environment_id: Option<String>,
    pub inputs: Value,
    #[serde(default)]
    pub secret_input_names: Vec<String>,
    pub initiator: FlowInitiator,
    /// Per-run consent for remote side effects; never stored in a definition.
    pub confirm_effects: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FlowInitiator {
    Human,
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowRun {
    pub id: String,
    pub workspace_id: String,
    pub flow_id: String,
    pub definition: FlowDefinition,
    pub context: FlowRunInput,
    pub resources: Value,
    pub status: String,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub steps: Vec<FlowStepRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowStepRun {
    pub step_id: String,
    pub status: String,
    pub duration_ms: u64,
    pub attempts: Vec<FlowAttempt>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowAttempt {
    pub number: u32,
    pub input: Value,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}
