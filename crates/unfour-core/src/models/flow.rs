use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowDefinition {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub revision: i64,
    #[serde(deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<FlowInputDefinition>,
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
    WaitUntil {
        probe: FlowAction,
        success_when: FlowPredicate,
        failure_when: Option<FlowPredicate>,
        interval_ms: u64,
        max_attempts: Option<u32>,
        #[serde(default)]
        probe_error_policy: FlowProbeErrorPolicy,
        #[serde(default)]
        interval_strategy: FlowIntervalStrategy,
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
    In,
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
pub struct FlowRunSummary {
    pub id: String,
    pub flow_id: String,
    pub status: FlowRunStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
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
    pub status: FlowRunStatus,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub steps: Vec<FlowStepRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowStepRun {
    pub step_id: String,
    pub status: FlowStepRunStatus,
    pub duration_ms: u64,
    pub attempts: Vec<FlowAttempt>,
    pub error: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub next_check_at: Option<String>,
    #[serde(default)]
    pub output: Option<Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowInputDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub input_type: FlowInputType,
    #[serde(default)]
    pub required: bool,
    // Explicit JSON null is a real default; an absent default stays absent.
    #[serde(
        default,
        deserialize_with = "deserialize_default",
        skip_serializing_if = "Option::is_none"
    )]
    pub default: Option<Value>,
    #[serde(default)]
    pub secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
fn deserialize_default<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}
fn deserialize_inputs<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<FlowInputDefinition>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Input {
        Legacy(String),
        Definition(FlowInputDefinition),
    }
    Ok(Vec::<Input>::deserialize(d)?
        .into_iter()
        .map(|input| match input {
            Input::Definition(definition) => definition,
            Input::Legacy(name) => FlowInputDefinition {
                name,
                input_type: FlowInputType::Json,
                required: true,
                default: None,
                secret: false,
                description: None,
            },
        })
        .collect())
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FlowInputType {
    String,
    Number,
    Boolean,
    Json,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FlowProbeErrorPolicy {
    #[default]
    FailImmediately,
    RetryTransientErrors,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FlowIntervalStrategy {
    #[default]
    Fixed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FlowRunStatus {
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
    Interrupted,
    ValidationFailed,
}

impl FlowRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timedOut",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
            Self::ValidationFailed => "validationFailed",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FlowStepRunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
    Interrupted,
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flow_legacy_inputs_upgrade_without_losing_value_types() {
        let definition: FlowDefinition = serde_json::from_value(json!({
            "id":"old", "workspaceId":"ws", "name":"old", "revision":1,
            "inputs":["value"], "steps":[]
        }))
        .unwrap();
        let value = serde_json::to_value(definition).unwrap();
        assert_eq!(value["inputs"][0]["name"], "value");
        assert_eq!(value["inputs"][0]["type"], "json");
        assert_eq!(value["inputs"][0]["required"], true);
    }

    #[test]
    fn flow_wait_until_contract_roundtrips() {
        let step: FlowStep = serde_json::from_value(json!({
            "id":"wait", "name":"wait", "timeoutMs":500,
            "kind":"waitUntil", "probe":{"capability":"api","resourceId":"api","arguments":{}},
            "successWhen":{"left":true,"op":"eq","right":true},
            "failureWhen":{"left":"pending","op":"in","right":["failed","cancelled"]},
            "intervalMs":10
        }))
        .unwrap();
        let value = serde_json::to_value(step).unwrap();
        assert_eq!(value["probeErrorPolicy"], "failImmediately");
        assert_eq!(value["intervalStrategy"], "fixed");
        assert!(value["maxAttempts"].is_null());
    }

    #[test]
    fn flow_status_wire_values_remain_stable_and_reject_unknown_states() {
        for (status, wire) in [
            (FlowRunStatus::Running, "running"),
            (FlowRunStatus::Succeeded, "succeeded"),
            (FlowRunStatus::Failed, "failed"),
            (FlowRunStatus::TimedOut, "timedOut"),
            (FlowRunStatus::Cancelled, "cancelled"),
            (FlowRunStatus::Interrupted, "interrupted"),
            (FlowRunStatus::ValidationFailed, "validationFailed"),
        ] {
            assert_eq!(serde_json::to_value(status).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<FlowRunStatus>(json!(wire)).unwrap(),
                status
            );
            assert_eq!(status.as_str(), wire);
        }
        for (status, wire) in [
            (FlowStepRunStatus::Pending, "pending"),
            (FlowStepRunStatus::Running, "running"),
            (FlowStepRunStatus::Succeeded, "succeeded"),
            (FlowStepRunStatus::Failed, "failed"),
            (FlowStepRunStatus::TimedOut, "timedOut"),
            (FlowStepRunStatus::Cancelled, "cancelled"),
            (FlowStepRunStatus::Interrupted, "interrupted"),
            (FlowStepRunStatus::Skipped, "skipped"),
        ] {
            assert_eq!(serde_json::to_value(status).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<FlowStepRunStatus>(json!(wire)).unwrap(),
                status
            );
        }
        assert!(serde_json::from_value::<FlowRunStatus>(json!("typo")).is_err());
        assert!(serde_json::from_value::<FlowStepRunStatus>(json!("validationFailed")).is_err());
    }
}
