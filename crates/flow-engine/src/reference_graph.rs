//! Reference availability follows execution edges, not storage order.
use crate::expression::invalid;
use serde_json::Value;
use std::collections::HashSet;
use unfour_core::{models::*, AppResult};

#[cfg(test)]
#[path = "reference_graph_tests.rs"]
mod tests;

pub(crate) fn guaranteed_upstream(steps: &[FlowStep]) -> Vec<HashSet<String>> {
    let mut incoming: Vec<Vec<HashSet<String>>> = vec![vec![]; steps.len()];
    if !steps.is_empty() {
        incoming[0].push(HashSet::new());
    }
    let mut result = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let mut safe = incoming[index].first().cloned().unwrap_or_default();
        for path in &incoming[index] {
            safe.retain(|id| path.contains(id));
        }
        result.push(safe.clone());
        if incoming[index].is_empty() {
            continue;
        }
        safe.insert(step.id.clone());
        let targets: Vec<&str> = match &step.node {
            FlowNode::Condition {
                if_true, if_false, ..
            } => vec![if_true, if_false],
            _ => step
                .next
                .as_deref()
                .or_else(|| steps.get(index + 1).map(|s| s.id.as_str()))
                .into_iter()
                .collect(),
        };
        for target in targets {
            if let Some(next) = steps
                .iter()
                .position(|s| s.id == target)
                .filter(|next| *next > index)
            {
                incoming[next].push(safe.clone());
            }
        }
    }
    result
}

fn check_pointer(pointer: &str, safe: &HashSet<String>, probe: bool) -> AppResult<()> {
    let mut parts = pointer.strip_prefix('/').unwrap_or("").split('/');
    let decode = |part: &str| part.replace("~1", "/").replace("~0", "~");
    let root = decode(parts.next().unwrap_or(""));
    if (root == "steps" && !parts.next().is_some_and(|id| safe.contains(&decode(id))))
        || (root == "probe" && !probe)
    {
        return Err(invalid("FLOW_UNSAFE_REFERENCE"));
    }
    Ok(())
}

fn check(value: &Value, safe: &HashSet<String>, probe: bool) -> AppResult<()> {
    match value {
        Value::Object(map) => {
            if let Some(pointer) = map.get("$ref").and_then(Value::as_str) {
                check_pointer(pointer, safe, probe)?;
            } else {
                for child in map.values() {
                    check(child, safe, probe)?;
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                check(child, safe, probe)?;
            }
        }
        Value::String(text) => {
            let mut rest = text.as_str();
            while let Some(start) = rest.find("${") {
                rest = &rest[start + 2..];
                let Some(end) = rest.find('}') else { break };
                check_pointer(&rest[..end], safe, probe)?;
                rest = &rest[end + 1..];
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn validate_references(steps: &[FlowStep]) -> AppResult<()> {
    let upstream = guaranteed_upstream(steps);
    for (step, safe) in steps.iter().zip(&upstream) {
        let predicate =
            |value: &FlowPredicate, probe| check(&serde_json::to_value(value)?, safe, probe);
        match &step.node {
            FlowNode::Action { action } => check(&action.arguments, safe, false)?,
            FlowNode::Condition {
                predicate: value, ..
            } => predicate(value, false)?,
            FlowNode::Poll {
                probe,
                predicate: value,
                ..
            } => {
                check(&probe.arguments, safe, false)?;
                predicate(value, true)?;
            }
            FlowNode::WaitUntil {
                probe,
                success_when,
                failure_when,
                ..
            } => {
                check(&probe.arguments, safe, false)?;
                predicate(success_when, true)?;
                if let Some(value) = failure_when {
                    predicate(value, true)?;
                }
            }
            FlowNode::Wait { .. } => {}
        }
    }
    Ok(())
}
