use super::*;

mod contracts;
mod payload_safety;
#[allow(dead_code, unused_imports)]
#[path = "../../tests/worker/support/mod.rs"]
mod support;

const FLOW_ID: &str = "00000000-0000-4000-8000-000000000001";
const RESOURCE_ID: &str = "00000000-0000-4000-8000-000000000002";

fn payload() -> Value {
    json!({"name":"Deploy", "inputs":[{"name":"credential", "type":"string", "secret":true, "required":true}],
        "steps":[{"id":"wait", "name":"Wait", "kind":"wait", "durationMs":0, "timeoutMs":1000}]})
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
        deleted_at: payload.is_none().then(|| "2026-10-20T00:00:00Z".into()),
        payload,
    }
}
