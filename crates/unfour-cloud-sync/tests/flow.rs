//! Exercises the shipping Registry/Worker without unit-build registration gates.
use serde_json::{json, Value};
use unfour_cloud_sync::{flow_definition::*, RemoteChange, SyncError, SyncOperation};
use unfour_core::{domain::*, models::FlowDefinition};

#[allow(dead_code, unused_imports)]
#[path = "worker/support/mod.rs"]
mod support;
mod flow {
    pub mod lifecycle;
    mod regressions;
    mod snapshots;
    mod upgrade;
    use super::*;
}

const FLOW_ID: &str = "00000000-0000-4000-8000-000000000001";
const RESOURCE_ID: &str = "00000000-0000-4000-8000-000000000002";

fn payload() -> Value {
    json!({"name":"Deploy", "inputs":[{"name":"credential", "type":"string", "secret":true, "required":true}],
        "steps":[{"id":"wait", "name":"Wait", "kind":"wait", "durationMs":0, "timeoutMs":1000}]})
}

fn definition(workspace: &str) -> FlowDefinition {
    let mut value = payload();
    value["id"] = json!("");
    value["workspaceId"] = json!(workspace);
    value["revision"] = json!(0);
    serde_json::from_value(value).unwrap()
}

fn change(workspace: &str, id: &str, payload: Option<Value>, version: i64) -> RemoteChange {
    RemoteChange {
        cursor: version,
        operation_id: format!("remote-{version}"),
        entity_type: WIRE_NAME.into(),
        entity_id: id.into(),
        parent_entity_id: Some(workspace.into()),
        operation: if payload.is_some() {
            SyncOperation::Upsert
        } else {
            SyncOperation::Delete
        },
        server_version: version,
        payload_schema_version: 1,
        deleted_at: payload.is_none().then(|| "2026-10-21T00:00:00Z".into()),
        payload,
    }
}
