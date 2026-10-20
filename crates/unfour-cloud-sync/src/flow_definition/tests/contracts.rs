use super::*;
use crate::remote_compatibility::{remote_change_disposition, RemoteEntityDisposition};

#[test]
fn all_node_kinds_and_reference_values_roundtrip_without_semantic_changes() {
    let action = json!({"capability":"api", "resourceId":RESOURCE_ID, "connectionId":null,
        "arguments":{"headers":{"$ref":"/inputs/headers"}}});
    let predicate = json!({"left":{"$ref":"/probe/status"}, "op":"eq", "right":200});
    let mut value = payload();
    value["steps"] = json!([
        {"id":"call", "name":"Call", "timeoutMs":1000, "kind":"action", "action":action},
        {"id":"branch", "name":"Branch", "timeoutMs":1000, "kind":"condition",
            "predicate":{"left":{"$ref":"/steps/call/status"}, "op":"eq", "right":200},
            "ifTrue":"poll", "ifFalse":"$end"},
        {"id":"poll", "name":"Poll", "timeoutMs":1000, "kind":"poll", "probe":action,
            "predicate":predicate, "intervalMs":10, "maxAttempts":3},
        {"id":"until", "name":"Until", "timeoutMs":1000, "kind":"waitUntil", "probe":action,
            "successWhen":predicate, "failureWhen":{"left":{"$ref":"/probe/status"},"op":"eq","right":500},
            "intervalMs":10, "maxAttempts":null, "probeErrorPolicy":"retryTransientErrors", "intervalStrategy":"fixed"},
        {"id":"wait", "name":"Wait", "timeoutMs":1000, "kind":"wait", "durationMs":10, "next":"$end"}
    ]);
    let ExternalFlowDefinitionApply::Upsert(record) =
        parse_change("ws", &change("ws", FLOW_ID, Some(value), 1)).unwrap()
    else {
        panic!()
    };
    let snapshot = FlowDefinitionSnapshot {
        id: FLOW_ID.into(),
        workspace_id: "ws".into(),
        revision: 1,
        definition: record.definition.clone(),
    };
    let wire = canonical_payload(&snapshot).unwrap();
    let ExternalFlowDefinitionApply::Upsert(replayed) =
        parse_change("ws", &change("ws", FLOW_ID, Some(wire), 1)).unwrap()
    else {
        panic!()
    };
    assert_eq!(record.definition, replayed.definition);
}

#[test]
fn strict_shape_and_compatibility() {
    let value = payload();
    parse_change("ws", &change("ws", FLOW_ID, Some(value.clone()), 1)).unwrap();
    for (pointer, extra) in [
        ("/revision", json!(99)),
        ("/runs", json!([])),
        ("/status", json!("running")),
        ("/steps/0/outputs", json!({})),
        ("/inputs/0/value", json!("secret")),
    ] {
        let mut bad = value.clone();
        let (parent, field) = pointer.rsplit_once('/').unwrap();
        bad.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(field.into(), extra);
        assert_eq!(
            parse_change("ws", &change("ws", FLOW_ID, Some(bad), 1)).unwrap_err(),
            SyncError::InvalidData
        );
    }
    let mut future = change("ws", FLOW_ID, Some(value.clone()), 1);
    future.payload_schema_version = 2;
    assert_eq!(
        parse_change("ws", &future).unwrap_err(),
        SyncError::CompatibilityWaiting
    );
    assert_eq!(
        remote_change_disposition(&future).unwrap(),
        RemoteEntityDisposition::SkipUnsupportedPayload
    );
    for (pointer, subtype) in [("/steps/0/kind", "parallel"), ("/inputs/0/type", "file")] {
        future.payload_schema_version = 1;
        future.payload = Some(value.clone());
        *future
            .payload
            .as_mut()
            .unwrap()
            .pointer_mut(pointer)
            .unwrap() = json!(subtype);
        assert_eq!(
            remote_change_disposition(&future).unwrap(),
            RemoteEntityDisposition::SkipUnsupportedPayload
        );
    }
    let mut deleted = change("ws", FLOW_ID, None, 2);
    deleted.payload_schema_version = 99;
    parse_change("ws", &deleted).unwrap();
    deleted.parent_entity_id = Some("wrong".into());
    assert!(parse_change("ws", &deleted).is_err());
    deleted.parent_entity_id = Some("ws".into());
    deleted.deleted_at = Some("invalid".into());
    assert!(parse_change("ws", &deleted).is_err());
}

#[test]
fn secrets_and_local_material_are_rejected_without_redaction() {
    for arguments in [
        json!({"authorization":"Bearer secret"}),
        json!({"cookie":123}),
        json!({"proxy-authorization":false}),
        json!({"x-api-key":"secret"}),
        json!({"x-auth-token":"secret"}),
        json!({"data":"{\"token\":\"secret\"}"}),
        json!({"data":"@unfour-secret:handle"}),
        json!({"keychain":"${/inputs/key}"}),
        json!({"privateKeyPath":"${/inputs/path}"}),
        json!({"data":format!("-----BEGIN {}-----", "PRIVATE KEY")}),
        json!({"data":"file:///home/me/key"}),
    ] {
        let mut value = payload();
        value["steps"] = json!([{"id":"api","name":"Call","timeoutMs":1000,"kind":"action",
            "action":{"capability":"api","resourceId":RESOURCE_ID,"arguments":arguments}}]);
        let original = value.clone();
        assert_eq!(
            parse_change("ws", &change("ws", FLOW_ID, Some(value.clone()), 1)).unwrap_err(),
            SyncError::FlowUnsafeDefinition
        );
        assert_eq!(value, original);
    }
    for default in [json!(null), json!("secret"), json!(123)] {
        let mut value = payload();
        value["inputs"][0]["default"] = default;
        assert!(parse_change("ws", &change("ws", FLOW_ID, Some(value), 1)).is_err());
    }
    let mut value = payload();
    value["steps"] = json!([{"id":"api","name":"Call","timeoutMs":1000,"kind":"action",
        "action":{"capability":"api","resourceId":RESOURCE_ID,"arguments":{"authorization":"Bearer ${/inputs/credential}"}}}]);
    let parsed = parse_change("ws", &change("ws", FLOW_ID, Some(value.clone()), 1)).unwrap();
    let ExternalFlowDefinitionApply::Upsert(parsed) = parsed else {
        panic!()
    };
    assert_eq!(
        parsed.definition["steps"][0]["action"]["arguments"],
        value["steps"][0]["action"]["arguments"]
    );
    assert!(!format!("{parsed:?}").contains("credential"));
}

#[test]
fn size_is_utf8_bytes_and_never_truncated() {
    let mut value = payload();
    value["steps"][0]["next"] = Value::Null;
    value["inputs"][0]["description"] = json!("");
    let overhead = serde_json::to_vec(&value).unwrap().len();
    value["inputs"][0]["description"] = json!("x".repeat(MAX_PAYLOAD_BYTES - overhead));
    assert_eq!(serde_json::to_vec(&value).unwrap().len(), MAX_PAYLOAD_BYTES);
    parse_change("ws", &change("ws", FLOW_ID, Some(value.clone()), 1)).unwrap();
    value["inputs"][0]["description"] = json!("界".repeat(MAX_PAYLOAD_BYTES / 3));
    assert_eq!(
        parse_change("ws", &change("ws", FLOW_ID, Some(value), 1)).unwrap_err(),
        SyncError::FlowPayloadTooLarge
    );
}

#[test]
fn legacy_local_inputs_normalize_but_wire_is_strict() {
    let mut old = payload();
    old["id"] = json!(FLOW_ID);
    old["workspaceId"] = json!("ws");
    old["revision"] = json!(4);
    old["inputs"] = json!(["value"]);
    let definition: FlowDefinition = serde_json::from_value(old).unwrap();
    let snapshot = FlowDefinitionSnapshot {
        id: FLOW_ID.into(),
        workspace_id: "ws".into(),
        revision: 4,
        definition: serde_json::to_value(definition).unwrap(),
    };
    let wire = canonical_payload(&snapshot).unwrap();
    assert_eq!(wire["inputs"][0]["type"], "json");
    assert_eq!(wire["inputs"][0]["required"], true);
    for field in ["id", "revision", "workspaceId", "updatedAt"] {
        assert!(wire.get(field).is_none());
    }
    let mut bad = wire;
    bad["inputs"] = json!(["value"]);
    assert_eq!(
        parse_change("ws", &change("ws", FLOW_ID, Some(bad), 1)).unwrap_err(),
        SyncError::InvalidData
    );
}
