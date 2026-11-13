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
    mod ui_notifications;
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

#[test]
fn redis_flow_wire_retains_exact_operation_and_rejects_scan_or_unknown_fields() {
    let mut flow = payload();
    flow["steps"] = json!([{"id":"cache","name":"Read cache","kind":"action","timeoutMs":1000,
    "action":{"capability":"redis","resourceId":RESOURCE_ID,"arguments":{
        "key":"product:${/inputs/id}","equals":{"$ref":"/inputs/price"}
    }}}]);
    let change_for = |payload| change("workspace", FLOW_ID, Some(payload), 1);
    let ExternalFlowDefinitionApply::Upsert(applied) =
        parse_change("workspace", &change_for(flow.clone())).unwrap()
    else {
        panic!("upsert");
    };
    assert_eq!(
        applied.definition["steps"][0]["action"]["arguments"],
        flow["steps"][0]["action"]["arguments"]
    );
    flow["steps"][0]["action"]["arguments"]["scan"] = json!("*");
    assert!(matches!(
        parse_change("workspace", &change_for(flow)),
        Err(SyncError::InvalidData)
    ));
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
