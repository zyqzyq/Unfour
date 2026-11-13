use super::*;

#[tokio::test]
async fn sql_route_review_is_invalidated_by_each_host_key_change() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let jump = bus.save_ssh_connection(serde_json::from_value(json!({
        "workspaceId":ws,"name":"jump","host":"jump.invalid","username":"reader","authKind":"none"
    })).unwrap()).await.unwrap();
    let mut input: unfour_core::models::DatabaseConnectionInput = serde_json::from_value(json!({
        "workspaceId":ws,"name":"warehouse","driver":"mysql","host":"db.invalid","username":"reader",
        "sshConnectionId":jump.id,"expectedSshRevision":jump.revision
    })).unwrap();
    let saved = bus
        .save_database_connection_with_secret(input.clone(), Some("fixture-password".into()))
        .await
        .unwrap();
    let keys = unfour_ssh_engine::HostKeyStore::new(bus.db.pool().clone());
    keys.record_fingerprint(&ws, "jump.invalid", 22, "first")
        .await
        .unwrap();
    keys.delete_fingerprint(&ws, "jump.invalid", 22)
        .await
        .unwrap();
    let first = bus
        .list_database_connections(ws.clone())
        .await
        .unwrap()
        .remove(0);
    assert_eq!(first.ssh_revision, Some(-1));
    keys.record_fingerprint(&ws, "jump.invalid", 22, "second")
        .await
        .unwrap();
    keys.record_fingerprint(&ws, "jump.invalid", 22, "third")
        .await
        .unwrap();
    let second = bus
        .list_database_connections(ws.clone())
        .await
        .unwrap()
        .remove(0);
    assert_eq!(second.ssh_revision, Some(-2));
    assert_eq!(
        bus.test_database_connection(ws.clone(), saved.id.clone())
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );

    let snapshot = bus.workspace_bundle_export(ws.clone()).await.unwrap();
    let copy = bus
        .workspace_bundle_import(snapshot, "Restored".into())
        .await
        .unwrap();
    assert_eq!(
        bus.list_database_connections(copy.id)
            .await
            .unwrap()
            .remove(0)
            .ssh_revision,
        Some(-1)
    );
    input.id = Some(saved.id);
    input.credential_ref = saved.credential_ref;
    assert!(matches!(
        bus.save_database_connection_checked(input.clone(), None, Some(first.clone()))
            .await,
        Err(unfour_core::AppError::RowConflict(_))
    ));
    let reviewed = bus
        .save_database_connection_checked(input.clone(), None, Some(second))
        .await
        .unwrap();
    assert_eq!(reviewed.ssh_revision, Some(jump.revision));
    keys.delete_fingerprint(&ws, "jump.invalid", 22)
        .await
        .unwrap();
    assert_eq!(
        bus.list_database_connections(ws.clone()).await.unwrap()[0].ssh_revision,
        Some(-3)
    );
    assert!(matches!(
        bus.save_database_connection_checked(input, None, Some(first))
            .await,
        Err(unfour_core::AppError::RowConflict(_))
    ));
}

#[tokio::test]
async fn sql_route_preserves_old_clients_and_remaps_jump_on_workspace_restore() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let other = bus.create_workspace("Other".into()).await.unwrap();
    let jump = bus.save_ssh_connection(serde_json::from_value(json!({
        "workspaceId":ws,"name":"Jump","host":"jump.invalid","username":"app","authKind":"none"
    })).unwrap()).await.unwrap();
    let target = json!({"workspaceId":ws,"name":"Warehouse","driver":"postgres","host":"pg.invalid",
        "database":"app","username":"reader","sshConnectionId":jump.id,"expectedSshRevision":jump.revision});
    let mut wrong_workspace = target.clone();
    wrong_workspace["workspaceId"] = json!(other.id);
    let err = bus
        .save_database_connection(serde_json::from_value(wrong_workspace).unwrap())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "SSH_TUNNEL_ROUTE_MISSING");
    let saved = bus
        .save_database_connection(serde_json::from_value(target.clone()).unwrap())
        .await
        .unwrap();
    let mut edited = target;
    edited["id"] = json!(saved.id);
    edited.as_object_mut().unwrap().remove("sshConnectionId");
    edited
        .as_object_mut()
        .unwrap()
        .remove("expectedSshRevision");
    let preserved = bus
        .save_database_connection(serde_json::from_value(edited.clone()).unwrap())
        .await
        .unwrap();
    assert_eq!(preserved.ssh_connection_id.as_ref(), Some(&jump.id));
    let mut stale = edited.clone();
    stale["sshConnectionId"] = json!(jump.id);
    stale["expectedSshRevision"] = json!(jump.revision + 1);
    assert_eq!(
        bus.save_database_connection(serde_json::from_value(stale).unwrap())
            .await
            .unwrap_err()
            .code(),
        "SSH_TUNNEL_ROUTE_CHANGED"
    );
    let snapshot = bus.workspace_bundle_export(ws.clone()).await.unwrap();
    let parsed: Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(parsed["version"], 4);
    assert_eq!(parsed["databaseRoutes"][0]["sshConnectionId"], jump.id);
    let copy = bus
        .workspace_bundle_import(snapshot, "Restored".into())
        .await
        .unwrap();
    let copied = bus
        .list_database_connections(copy.id.clone())
        .await
        .unwrap()
        .remove(0);
    let copied_jump = bus.list_ssh_connections(copy.id).await.unwrap().remove(0);
    assert_ne!(copied_jump.id, jump.id);
    assert_eq!(copied.ssh_connection_id, Some(copied_jump.id));
    assert_eq!(copied.ssh_revision, Some(copied_jump.revision));
    edited["sshConnectionId"] = Value::Null;
    let direct = bus
        .save_database_connection(serde_json::from_value(edited).unwrap())
        .await
        .unwrap();
    assert!(direct.ssh_connection_id.is_none());
    assert!(direct.ssh_revision.is_none());
}
