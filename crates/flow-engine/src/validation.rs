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
    for input in &definition.inputs {
        if input.name.trim().is_empty() || input.name.len() > 200 || !ids.insert(&input.name) {
            return Err(invalid("FLOW_INVALID_INPUT_DEFINITION"));
        }
        if let Some(default) = &input.default {
            if input.secret || crate::expression::sensitive(&input.name) {
                return Err(invalid("FLOW_SECRET_DEFAULT_NOT_ALLOWED"));
            }
            if !matches_type(input.input_type, default) {
                return Err(invalid("FLOW_INPUT_TYPE_MISMATCH"));
            }
        }
    }
    ids.clear();
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
            FlowNode::WaitUntil {
                interval_ms,
                max_attempts,
                probe,
                ..
            } => {
                if !(10..=60_000).contains(interval_ms)
                    || max_attempts.is_some_and(|n| !(1..=1000).contains(&n))
                {
                    return Err(invalid("FLOW_INVALID_WAIT_UNTIL_BOUNDS"));
                }
                if probe.capability == FlowCapability::Ssh {
                    return Err(invalid("FLOW_SSH_PROBE_UNSUPPORTED"));
                }
            }
            FlowNode::Wait { duration_ms } if *duration_ms >= step.timeout_ms => {
                return Err(invalid("FLOW_WAIT_EXCEEDS_TIMEOUT"))
            }
            _ => {}
        }
    }
    Ok(())
}

fn matches_type(kind: FlowInputType, value: &serde_json::Value) -> bool {
    match kind {
        FlowInputType::String => value.is_string(),
        FlowInputType::Number => value.is_number(),
        FlowInputType::Boolean => value.is_boolean(),
        FlowInputType::Json => true,
    }
}

pub(crate) fn resolve_inputs(
    definition: &FlowDefinition,
    inputs: &mut serde_json::Value,
) -> AppResult<()> {
    let inputs = inputs
        .as_object_mut()
        .ok_or_else(|| invalid("FLOW_INPUTS_OBJECT_REQUIRED"))?;
    for field in &definition.inputs {
        if !inputs.contains_key(&field.name) {
            if let Some(default) = &field.default {
                inputs.insert(field.name.clone(), default.clone());
            } else if field.required {
                return Err(invalid("FLOW_REQUIRED_INPUT_MISSING"));
            }
        }
        if let Some(value) = inputs.get(&field.name) {
            if !matches_type(field.input_type, value) {
                return Err(invalid("FLOW_INPUT_TYPE_MISMATCH"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn definition(inputs: Value) -> FlowDefinition {
        serde_json::from_value(json!({"id":"", "workspaceId":"ws", "name":"test", "revision":0,"inputs":inputs,"steps":[{"id":"wait","name":"wait","kind":"wait","durationMs":0,"timeoutMs":100}]})).unwrap()
    }
    #[test]
    fn flow_inputs_validate_schema_defaults_and_runtime_types() {
        let flow = definition(json!([
            {"name":"service","type":"string","required":true},
            {"name":"version","type":"number","required":true},
            {"name":"dryRun","type":"boolean","default":false},
            {"name":"config","type":"json","default":null},
            {"name":"optional","type":"string","secret":true}
        ]));
        validate(&flow).unwrap();
        let mut input = json!({"service":"app","version":2});
        resolve_inputs(&flow, &mut input).unwrap();
        assert_eq!(
            input,
            json!({"service":"app","version":2,"dryRun":false,"config":null})
        );
        for mut value in [
            json!({}),
            json!({"service":false,"version":2}),
            json!({"service":"app","version":"2"}),
            json!([]),
        ] {
            assert!(resolve_inputs(&flow, &mut value).is_err());
        }
        for inputs in [
            json!([{"name":"a","type":"string"},{"name":"a","type":"string"}]),
            json!([{"name":" ","type":"string"}]),
            json!([{"name":"a","type":"number","default":"bad"}]),
            json!([{"name":"a","type":"string","secret":true,"default":"never-store"}]),
            json!([{"name":"authorization","type":"string","default":"never-store"}]),
            json!([{"name":"Passphrase","type":"string","default":"never-store"}]),
        ] {
            assert!(validate(&definition(inputs)).is_err());
        }
        // A legacy input historically accepted any JSON type.
        resolve_inputs(&definition(json!(["value"])), &mut json!({"value":false})).unwrap();
    }
}
