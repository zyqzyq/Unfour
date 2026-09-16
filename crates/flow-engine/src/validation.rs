use crate::expression::invalid;
use std::collections::HashSet;
use unfour_core::{models::*, AppResult};

pub fn validate(definition: &FlowDefinition) -> AppResult<()> {
    if definition.workspace_id.is_empty()
        || definition.name.trim().is_empty()
        || definition.name.len() > 200
        || definition.steps.is_empty()
        || definition.steps.len() > 100
        || serde_json::to_vec(definition)?.len() > 1_048_576
    {
        return Err(invalid("FLOW_INVALID_DEFINITION"));
    }
    let mut ids = HashSet::new();
    for step in &definition.steps {
        if step.id.is_empty()
            || step.id.len() > 80
            || !step
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            || !ids.insert(&step.id)
            || !(1..=3_600_000).contains(&step.timeout_ms)
        {
            return Err(invalid("FLOW_INVALID_STEP"));
        }
    }
    for (index, step) in definition.steps.iter().enumerate() {
        let forward = |target: &str| -> AppResult<()> {
            if target == "$end"
                || definition
                    .steps
                    .iter()
                    .skip(index + 1)
                    .any(|s| s.id == target)
            {
                Ok(())
            } else {
                Err(invalid("FLOW_BRANCH_MUST_POINT_FORWARD"))
            }
        };
        if let Some(next) = &step.next {
            forward(next)?;
        }
        match &step.node {
            FlowNode::Condition {
                if_true, if_false, ..
            } => {
                forward(if_true)?;
                forward(if_false)?;
            }
            FlowNode::Poll {
                interval_ms,
                max_attempts,
                ..
            } if !(10..=60_000).contains(interval_ms) || !(1..=1000).contains(max_attempts) => {
                return Err(invalid("FLOW_INVALID_POLL_BOUNDS"))
            }
            FlowNode::Wait { duration_ms } if *duration_ms >= step.timeout_ms => {
                return Err(invalid("FLOW_WAIT_EXCEEDS_TIMEOUT"))
            }
            _ => {}
        }
    }
    Ok(())
}
