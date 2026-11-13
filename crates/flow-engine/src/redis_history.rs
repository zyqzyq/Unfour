//! Redis Flow checks store their conclusion. The live executor retains the inputs.
use serde_json::{json, Value};
use std::collections::HashSet;
use unfour_core::models::{FlowCapability, FlowNode, FlowRun};

fn visit_pointers(value: &Value, mut visit: impl FnMut(&str)) {
    if let Some(pointer) = value.get("$ref").and_then(Value::as_str) {
        visit(pointer);
    } else if let Some(text) = value.as_str() {
        for part in text.split("${").skip(1) {
            if let Some((pointer, _)) = part.split_once('}') {
                visit(pointer);
            }
        }
    }
}

pub(crate) fn protect_history(run: &FlowRun, persisted: &mut Value) -> HashSet<String> {
    let mut private_inputs = HashSet::new();
    let mut all_inputs = false;
    for (index, step) in run.definition.steps.iter().enumerate() {
        let (action, kind) = match &step.node {
            FlowNode::Action { action } => (action, "action"),
            FlowNode::Poll { probe, .. } | FlowNode::WaitUntil { probe, .. } => (probe, "probe"),
            _ => continue,
        };
        if action.capability != FlowCapability::Redis {
            continue;
        }
        persisted["definition"]["steps"][index][kind]["arguments"] = json!({});
        if let Some(args) = action.arguments.as_object() {
            for value in args.values() {
                visit_pointers(value, |pointer| {
                    if pointer == "/inputs" {
                        all_inputs = true;
                    } else if let Some(path) = pointer.strip_prefix("/inputs/") {
                        // Redact the top-level input even for a nested JSON Pointer.
                        let name = path
                            .split('/')
                            .next()
                            .unwrap_or("")
                            .replace("~1", "/")
                            .replace("~0", "~");
                        private_inputs.insert(name);
                    }
                });
            }
        }
    }
    if all_inputs {
        private_inputs.extend(run.definition.inputs.iter().map(|input| input.name.clone()));
        if let Some(inputs) = run.context.inputs.as_object() {
            private_inputs.extend(inputs.keys().cloned());
        }
    }
    if let Some(inputs) = persisted["context"]["inputs"].as_object_mut() {
        for (name, value) in inputs {
            if private_inputs.contains(name) {
                *value = json!("<redacted>");
            }
        }
    }
    for (index, input) in run.definition.inputs.iter().enumerate() {
        if input.default.is_some() && private_inputs.contains(&input.name) {
            persisted["definition"]["inputs"][index]["default"] = json!("<redacted>");
        }
    }
    private_inputs
}

/// A cache operand can equal "true", "-1", or a Redis type name. Preserve
/// engine-produced booleans, numbers and known type labels after masking user
/// text. Module-defined type names and any unrecognized fields stay scrubbed.
pub(crate) fn preserve_check_metadata(run: &FlowRun, persisted: &mut Value) {
    for (index, (step, record)) in run.definition.steps.iter().zip(&run.steps).enumerate() {
        let action = match &step.node {
            FlowNode::Action { action } => action,
            FlowNode::Poll { probe, .. } | FlowNode::WaitUntil { probe, .. } => probe,
            _ => continue,
        };
        if action.capability != FlowCapability::Redis || step.id != record.step_id {
            continue;
        }
        let saved = &mut persisted["steps"][index];
        for (i, attempt) in record.attempts.iter().enumerate() {
            if attempt.error.is_none() {
                if let Some(output) = &attempt.output {
                    preserve_conclusion(output, &mut saved["attempts"][i]["output"]);
                }
            }
        }
        if let Some(output) = &record.output {
            let target = &mut saved["output"];
            if matches!(step.node, FlowNode::WaitUntil { .. }) && output.get("result").is_some() {
                preserve_conclusion(&output["result"], &mut target["result"]);
                for name in ["attempts", "elapsedMs"] {
                    if output[name].is_u64() {
                        target[name] = output[name].clone();
                    }
                }
            } else {
                preserve_conclusion(output, target);
            }
        }
    }
}

fn preserve_conclusion(output: &Value, target: &mut Value) {
    for name in ["exists", "matched"] {
        if output[name].is_boolean() {
            target[name] = output[name].clone();
        }
    }
    if output["pttlMs"].as_i64().is_some_and(|ttl| ttl >= -2) {
        target["pttlMs"] = output["pttlMs"].clone();
    }
    if matches!(
        output["keyType"].as_str(),
        Some("string" | "hash" | "list" | "set" | "zset" | "stream" | "none")
    ) {
        target["keyType"] = output["keyType"].clone();
    }
}
