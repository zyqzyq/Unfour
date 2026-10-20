//! Dormant Protocol 5 Flow adapter. Production Registry deliberately excludes it.
//! No runtime feature flag: test-only wiring exercises the existing sync engine.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use unfour_core::{domain::*, models::FlowDefinition};

use crate::{RemoteChange, SyncError, SyncOperation};

pub const WIRE_NAME: &str = "flow";
pub const SCHEMA_VERSION: i64 = 1;
pub const MAX_PAYLOAD_BYTES: usize = 65_536;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Payload {
    name: String,
    inputs: Value,
    steps: Value,
}

/// Returns the exact definition, or rejects it. Never redacts executable data.
pub fn canonical_payload(snapshot: &FlowDefinitionSnapshot) -> Result<Value, SyncError> {
    let definition: FlowDefinition =
        serde_json::from_value(snapshot.definition.clone()).map_err(|_| SyncError::InvalidData)?;
    if definition.id != snapshot.id
        || definition.workspace_id != snapshot.workspace_id
        || definition.revision != snapshot.revision
        || snapshot.revision < 1
    {
        return Err(SyncError::InvalidData);
    }
    let normalized = serde_json::to_value(&definition).map_err(|_| SyncError::InvalidData)?;
    ensure_known_fields(&snapshot.definition, &normalized)?;
    let payload = json!({"name": definition.name, "inputs": normalized["inputs"], "steps": normalized["steps"]});
    validate_payload(&payload, &snapshot.workspace_id, &snapshot.id)?;
    Ok(payload)
}

pub fn parse_change(
    workspace: &str,
    change: &RemoteChange,
) -> Result<ExternalFlowDefinitionApply, SyncError> {
    if change.entity_type != WIRE_NAME
        || change.entity_id.is_empty()
        || change.parent_entity_id.as_deref() != Some(workspace)
        || change.server_version < 1
        || change.payload_schema_version < 1
    {
        return Err(SyncError::InvalidData);
    }
    uuid::Uuid::parse_str(&change.entity_id).map_err(|_| SyncError::InvalidData)?;
    match change.operation {
        SyncOperation::Delete => {
            if change.payload.is_some() {
                return Err(SyncError::InvalidData);
            }
            let deleted_at = change.deleted_at.clone().ok_or(SyncError::InvalidData)?;
            chrono::DateTime::parse_from_rfc3339(&deleted_at)
                .map_err(|_| SyncError::InvalidData)?;
            Ok(ExternalFlowDefinitionApply::Delete(ExternalDelete {
                entity: DomainEntityKey::new(
                    DomainEntityType::FlowDefinition,
                    workspace,
                    &change.entity_id,
                ),
                deleted_at,
            }))
        }
        SyncOperation::Upsert => {
            if change.payload_schema_version != SCHEMA_VERSION {
                return Err(SyncError::CompatibilityWaiting);
            }
            if change.deleted_at.is_some() {
                return Err(SyncError::InvalidData);
            }
            let payload = change.payload.as_ref().ok_or(SyncError::InvalidData)?;
            let definition = validate_payload(payload, workspace, &change.entity_id)?;
            Ok(ExternalFlowDefinitionApply::Upsert(
                ExternalFlowDefinitionUpsert {
                    id: change.entity_id.clone(),
                    workspace_id: workspace.into(),
                    definition: serde_json::to_value(definition)
                        .map_err(|_| SyncError::InvalidData)?,
                },
            ))
        }
    }
}

fn validate_payload(
    payload: &Value,
    workspace: &str,
    id: &str,
) -> Result<FlowDefinition, SyncError> {
    if unsupported_subtype(payload)? {
        return Err(SyncError::CompatibilityWaiting);
    }
    if serde_json::to_vec(payload)
        .map_err(|_| SyncError::InvalidData)?
        .len()
        > MAX_PAYLOAD_BYTES
    {
        return Err(SyncError::FlowPayloadTooLarge);
    }
    uuid::Uuid::parse_str(id).map_err(|_| SyncError::InvalidData)?;
    let wire: Payload =
        serde_json::from_value(payload.clone()).map_err(|_| SyncError::InvalidData)?;
    let raw = json!({"id": id, "workspaceId": workspace, "revision": 1,
        "name": wire.name, "inputs": wire.inputs, "steps": wire.steps});
    let definition: FlowDefinition =
        serde_json::from_value(raw.clone()).map_err(|_| SyncError::InvalidData)?;
    let normalized = serde_json::to_value(&definition).map_err(|_| SyncError::InvalidData)?;
    ensure_known_fields(&raw, &normalized)?;
    let canonical = json!({"name": normalized["name"], "inputs": normalized["inputs"], "steps": normalized["steps"]});
    if serde_json::to_vec(&canonical)
        .map_err(|_| SyncError::InvalidData)?
        .len()
        > MAX_PAYLOAD_BYTES
    {
        return Err(SyncError::FlowPayloadTooLarge);
    }
    reject_local_material(payload)?;
    for input in &definition.inputs {
        if let Some(default) = &input.default {
            if input.secret || unfour_core::redaction::is_sensitive_flow_name(&input.name) {
                return Err(SyncError::FlowUnsafeDefinition);
            }
            safe_expression(default)?;
        }
    }
    for step in normalized["steps"]
        .as_array()
        .ok_or(SyncError::InvalidData)?
    {
        for field in ["action", "probe"] {
            if let Some(action) = step.get(field) {
                uuid::Uuid::parse_str(
                    action["resourceId"]
                        .as_str()
                        .ok_or(SyncError::InvalidData)?,
                )
                .map_err(|_| SyncError::InvalidData)?;
                if let Some(connection) = action["connectionId"].as_str() {
                    uuid::Uuid::parse_str(connection).map_err(|_| SyncError::InvalidData)?;
                }
                safe_expression(&action["arguments"])?;
            }
        }
        for field in ["predicate", "successWhen", "failureWhen"] {
            if let Some(predicate) = step.get(field).filter(|value| !value.is_null()) {
                safe_expression(&predicate["left"])?;
                safe_expression(&predicate["right"])?;
            }
        }
    }
    unfour_flow_engine::validate_definition(&definition).map_err(|_| SyncError::InvalidData)?;
    Ok(definition)
}

pub(crate) fn unsupported_subtype(payload: &Value) -> Result<bool, SyncError> {
    let steps = payload
        .get("steps")
        .and_then(Value::as_array)
        .ok_or(SyncError::InvalidData)?;
    for step in steps {
        let kind = step
            .get("kind")
            .and_then(Value::as_str)
            .ok_or(SyncError::InvalidData)?;
        if !["action", "condition", "poll", "waitUntil", "wait"].contains(&kind) {
            return Ok(true);
        }
        for (object, field, supported) in [
            (
                step,
                "probeErrorPolicy",
                &["failImmediately", "retryTransientErrors"][..],
            ),
            (step, "intervalStrategy", &["fixed"][..]),
            (
                &step["action"],
                "capability",
                &["api", "ssh", "database"][..],
            ),
            (
                &step["probe"],
                "capability",
                &["api", "ssh", "database"][..],
            ),
            (
                &step["predicate"],
                "op",
                &["eq", "ne", "gt", "ge", "lt", "le", "in"][..],
            ),
            (
                &step["successWhen"],
                "op",
                &["eq", "ne", "gt", "ge", "lt", "le", "in"][..],
            ),
            (
                &step["failureWhen"],
                "op",
                &["eq", "ne", "gt", "ge", "lt", "le", "in"][..],
            ),
        ] {
            if let Some(value) = object.get(field) {
                let value = value.as_str().ok_or(SyncError::InvalidData)?;
                if !supported.contains(&value) {
                    return Ok(true);
                }
            }
        }
    }
    let inputs = payload
        .get("inputs")
        .and_then(Value::as_array)
        .ok_or(SyncError::InvalidData)?;
    for input in inputs {
        let kind = input
            .get("type")
            .and_then(Value::as_str)
            .ok_or(SyncError::InvalidData)?;
        if !["string", "number", "boolean", "json"].contains(&kind) {
            return Ok(true);
        }
    }
    Ok(false)
}

// serde's flattened FlowStep intentionally tolerates fields for local callers.
// The sync contract must not silently discard executable/critical fields.
fn ensure_known_fields(raw: &Value, normalized: &Value) -> Result<(), SyncError> {
    match (raw, normalized) {
        (Value::Object(raw), Value::Object(normalized)) => {
            for (key, value) in raw {
                ensure_known_fields(value, normalized.get(key).ok_or(SyncError::InvalidData)?)?;
            }
        }
        (Value::Array(raw), Value::Array(normalized)) => {
            if raw.len() != normalized.len() {
                return Err(SyncError::InvalidData);
            }
            for (a, b) in raw.iter().zip(normalized) {
                ensure_known_fields(a, b)?;
            }
        }
        _ if raw != normalized => return Err(SyncError::InvalidData),
        _ => {}
    }
    Ok(())
}

/// Opaque literal strings in executable slots cannot be proven secret-free.
/// v1 conservatively requires runtime references there (including SQL/shell
/// text, string defaults and string predicates). Numeric/bool/null literals
/// remain valid. Refusal is explicit; local definitions remain unrestricted.
fn safe_expression(value: &Value) -> Result<(), SyncError> {
    match value {
        Value::String(text) if is_reference(text) => Ok(()),
        Value::String(_) => Err(SyncError::FlowUnsafeDefinition),
        Value::Object(map) if map.len() == 1 && map.contains_key("$ref") => {
            let reference = map["$ref"]
                .as_str()
                .ok_or(SyncError::FlowUnsafeDefinition)?;
            if valid_pointer(reference) {
                Ok(())
            } else {
                Err(SyncError::FlowUnsafeDefinition)
            }
        }
        Value::Object(map) => {
            for (key, value) in map {
                if unfour_core::redaction::is_sensitive_flow_name(key)
                    && !value.as_str().is_some_and(is_reference)
                    && !(value
                        .as_object()
                        .is_some_and(|v| v.len() == 1 && v.contains_key("$ref")))
                {
                    return Err(SyncError::FlowUnsafeDefinition);
                }
                safe_expression(value)?;
            }
            Ok(())
        }
        Value::Array(values) => values.iter().try_for_each(safe_expression),
        _ => Ok(()),
    }
}

fn valid_pointer(value: &str) -> bool {
    ["/inputs", "/steps", "/probe"].iter().any(|prefix| {
        value == *prefix
            || value
                .strip_prefix(prefix)
                .is_some_and(|tail| tail.starts_with('/'))
    }) && !value.contains(['{', '}', '\n', '\r'])
}
fn is_reference(value: &str) -> bool {
    value
        .strip_prefix("Bearer ")
        .unwrap_or(value)
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
        .is_some_and(valid_pointer)
}

fn reject_local_material(value: &Value) -> Result<(), SyncError> {
    match value {
        Value::Object(map) => map.iter().try_for_each(|(key, value)| {
            let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
            if [
                "credentialref",
                "secretref",
                "keychain",
                "privatekeypath",
                "localpath",
                "runtimevariables",
                "temporaryvariables",
            ]
            .iter()
            .any(|marker| normalized.contains(marker))
            {
                return Err(SyncError::FlowUnsafeDefinition);
            }
            reject_local_material(value)
        }),
        Value::Array(values) => values.iter().try_for_each(reject_local_material),
        Value::String(value)
            if value.contains("@unfour-secret:")
                || value.contains("-----BEGIN ")
                || value.starts_with("file:")
                || value.contains(":\\")
                || value.starts_with("/Users/")
                || value.starts_with("/home/")
                || value.starts_with("\\\\") =>
        {
            Err(SyncError::FlowUnsafeDefinition)
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
