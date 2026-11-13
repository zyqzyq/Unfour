use super::*;
use crate::RedisConnectionInput;

#[tokio::test]
async fn redis_bundle_round_trip_keeps_credentials_device_scoped() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let input = RedisConnectionInput {
        id: None,
        expected_revision: None,
        workspace_id: ws.clone(),
        name: "Cache".into(),
        host: "cache.example.test".into(),
        port: 6380,
        username: Some("reader".into()),
        db_index: 4,
        tls: true,
        allow_writes: Some(true),
    };
    let source = bus
        .save_redis_connection(input, Some("test-only-cache-secret".into()))
        .await
        .unwrap();
    let flow: FlowDefinition = serde_json::from_value(json!({
        "workspaceId":ws, "id":"", "revision":0, "name":"Verify cache", "inputs":[],
        "steps":[{"id":"check","name":"Price","kind":"action","timeoutMs":1000,
            "action":{"capability":"redis","resourceId":source.id,"arguments":{"key":"fixture:price"}}}]
    })).unwrap();
    bus.save_flow(flow).await.unwrap();
    let share = bus
        .workspace_bundle_export_with_options(ws.clone(), Default::default())
        .await
        .unwrap()
        .content;
    let value: Value = serde_json::from_str(&share).unwrap();
    assert_eq!(value["version"], 3);
    assert!(!share.contains("test-only-cache-secret"));
    assert!(!share.contains("credentialRef"));
    assert!(value["credentialRequirements"]
        .as_array()
        .unwrap()
        .contains(&json!(source.id)));
    let copy = bus
        .workspace_bundle_import(share, "Portable cache".into())
        .await
        .unwrap();
    let copied = bus
        .list_redis_connections(&copy.id)
        .await
        .unwrap()
        .remove(0);
    assert_ne!(copied.id, source.id);
    let flow_copy = bus
        .list_flows(copy.id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|flow| flow.name == "Verify cache")
        .unwrap();
    assert_eq!(
        serde_json::to_value(flow_copy.steps).unwrap()[0]["action"]["resourceId"],
        copied.id
    );
    assert!(!copied.has_password);
    assert!(!copied.allow_writes);
    assert!(value["connections"][0]["config"]
        .get("allowWrites")
        .is_none());
    assert_eq!((copied.port, copied.db_index, copied.tls), (6380, 4, true));

    let backup = || WorkspaceBundleOptions {
        password: Some("test-only-archive-password".into()),
        include_secrets: true,
        ..Default::default()
    };
    let archive = bus
        .workspace_bundle_export_with_options(ws, backup())
        .await
        .unwrap()
        .content;
    let restored = bus
        .workspace_bundle_import_with_options(archive, "Restored cache".into(), backup())
        .await
        .unwrap();
    assert!(!bus.list_redis_connections(&restored.id).await.unwrap()[0].allow_writes);
    let mut conn = bus.db.pool().acquire().await.unwrap();
    let refs = unfour_redis_engine::exchange::credential_refs_on(&mut conn, &restored.id)
        .await
        .unwrap();
    drop(conn);
    assert_eq!(refs.len(), 1);
    assert_ne!(refs[0].0, source.id);
    assert_eq!(
        bus.secret_store
            .read_secret(restored.id, refs[0].1.clone())
            .await
            .unwrap(),
        "test-only-cache-secret"
    );
}
