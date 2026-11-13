use super::*;
use crate::RedisConnectionInput;

#[tokio::test]
async fn redis_bundle_round_trip_keeps_credentials_device_scoped() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let jump = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: ws.clone(),
            name: "Jump".into(),
            host: "jump.internal".into(),
            port: Some(22),
            username: "tester".into(),
            auth_kind: "none".into(),
            key_path: None,
            credential_ref: None,
            secret: None,
        })
        .await
        .unwrap();
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
        ssh_connection_id: Some(Some(jump.id.clone())),
        expected_ssh_revision: Some(jump.revision),
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
    let route = value["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == source.id)
        .unwrap();
    assert_eq!(route["config"]["kind"], "redisSsh");
    assert_eq!(route["config"]["sshConnectionId"], jump.id);
    let mut broken = value.clone();
    broken["connections"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c["id"] != jump.id);
    assert!(
        matches!(bus.workspace_bundle_import(broken.to_string(), "Broken".into()).await,
        Err(AppError::Validation(code)) if code == "WORKSPACE_BUNDLE_MISSING_REFERENCE")
    );
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
    assert!(copied.ssh_connection_id.is_some());
    assert_ne!(copied.ssh_connection_id.as_deref(), Some(jump.id.as_str()));
    assert_eq!(copied.ssh_connection_name.as_deref(), Some("Jump"));
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
    assert!(route["config"].get("allowWrites").is_none());
    assert_eq!((copied.port, copied.db_index, copied.tls), (6380, 4, true));

    let backup = || WorkspaceBundleOptions {
        password: Some("test-only-archive-password".into()),
        include_secrets: true,
        ..Default::default()
    };
    let archive = bus
        .workspace_bundle_export_with_options(ws.clone(), backup())
        .await
        .unwrap()
        .content;
    let restored = bus
        .workspace_bundle_import_with_options(archive, "Restored cache".into(), backup())
        .await
        .unwrap();
    let restored_cache = bus
        .list_redis_connections(&restored.id)
        .await
        .unwrap()
        .remove(0);
    assert!(!restored_cache.allow_writes);
    assert!(!restored_cache.route_needs_review);
    assert_eq!(restored_cache.ssh_connection_name.as_deref(), Some("Jump"));
    assert_ne!(restored_cache.ssh_connection_id, copied.ssh_connection_id);
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

    // The old secret is still stored for deliberate replacement. A backup must
    // not resume using it against a newly retargeted jump after restoration.
    sqlx::query("UPDATE connections SET host='new-jump.internal',revision=revision+1 WHERE id=? AND workspace_id=?")
        .bind(&jump.id).bind(&ws).execute(bus.db.pool()).await.unwrap();
    let archive = bus
        .workspace_bundle_export_with_options(ws, backup())
        .await
        .unwrap()
        .content;
    let preview = bus
        .workspace_bundle_preview_with_options(&archive, backup())
        .await
        .unwrap();
    assert!(preview
        .reconfigure
        .iter()
        .any(|issue| issue.entity_id == source.id && issue.code == "connection"));
    let locked = bus
        .workspace_bundle_import_with_options(archive, "Review restored route".into(), backup())
        .await
        .unwrap();
    let record = bus
        .list_redis_connections(&locked.id)
        .await
        .unwrap()
        .remove(0);
    assert!(record.route_needs_review && record.has_password && !record.allow_writes);
    assert_eq!(
        bus.inspect_redis_key(&locked.id, &record.id, "YQ==", None, Some(record.revision))
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    let missing_jump = record.ssh_connection_id.unwrap();
    sqlx::query("UPDATE connections SET deleted_at=updated_at WHERE workspace_id=? AND id=?")
        .bind(&locked.id)
        .bind(missing_jump)
        .execute(bus.db.pool())
        .await
        .unwrap();
    assert!(
        matches!(bus.workspace_bundle_export_with_options(locked.id, backup()).await,
        Err(AppError::Validation(code)) if code == "WORKSPACE_BUNDLE_MISSING_REFERENCE")
    );
}
