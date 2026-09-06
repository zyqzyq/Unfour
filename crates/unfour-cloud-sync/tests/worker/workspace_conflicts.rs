//! Preservation of local variable intent during remote conflicts.

use super::support::*;

#[tokio::test]
async fn pending_local_intent_blocks_remote_upsert_and_delete_without_losing_local_data() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "KEY", "local-value", false),
        )
        .await
        .unwrap();
    let cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor + 1,
        next_cursor: cursor + 1,
        changes: vec![RemoteChange {
            cursor: cursor + 1,
            operation_id: "remote-upsert".into(),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: local.id.clone(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Upsert,
            server_version: 2,
            payload_schema_version: 1,
            payload: Some(variable_payload("remote-value")),
            deleted_at: None,
        }],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    let value: String = sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = ?1")
        .bind(&local.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(value, "local-value");
    assert_eq!(
        service.conflicts(&workspace_id).await.unwrap()[0]
            .remote_payload
            .as_ref()
            .unwrap()["value"],
        "remote-value"
    );

    service
        .keep_local(&workspace_id, SyncEntityType::WorkspaceVariable, &local.id)
        .await
        .unwrap();
    bus.workspace_variable_update(
        workspace_id.clone(),
        local.id.clone(),
        variable(Some(local.id.clone()), "KEY", "new-local", false),
    )
    .await
    .unwrap();
    let cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor + 1,
        next_cursor: cursor + 1,
        changes: vec![RemoteChange {
            cursor: cursor + 1,
            operation_id: "remote-delete".into(),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: local.id.clone(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Delete,
            server_version: 4,
            payload_schema_version: 1,
            payload: None,
            deleted_at: Some("2026-07-28T01:00:00Z".into()),
        }],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    let row: (String, Option<String>) =
        sqlx::query_as("SELECT value, deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(row, ("new-local".into(), None));
}

#[tokio::test]
async fn future_schema_push_conflict_is_preserved_and_cannot_be_resolved_as_schema_one() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "LOCAL", "local-value", false),
        )
        .await
        .unwrap();
    let before: (String, String, i64) = sqlx::query_as(
        r#"SELECT operation_id, canonical_payload_json, payload_schema_version
           FROM cloud_sync_outbox WHERE entity_id = ?1"#,
    )
    .bind(&local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    *transport.push_conflict.lock().unwrap() = Some(unfour_cloud_sync::SyncConflictDetails {
        entity_type: SyncEntityType::WorkspaceVariable,
        entity_id: local.id.clone(),
        parent_entity_id: Some(workspace_id.clone()),
        server_version: 8,
        operation: SyncOperation::Upsert,
        payload_schema_version: 7,
        payload: Some(variable_payload("future-remote")),
    });

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    let stored: (i64, Option<String>, String) = sqlx::query_as(
        r#"SELECT conflict_payload_schema_version, conflict_operation_id, sync_status
           FROM cloud_sync_entity_state WHERE entity_id = ?1"#,
    )
    .bind(&local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(stored, (7, None, "conflict".into()));
    assert!(matches!(
        service
            .keep_local(&workspace_id, SyncEntityType::WorkspaceVariable, &local.id)
            .await,
        Err(SyncError::CompatibilityWaiting)
    ));
    assert!(matches!(
        service
            .use_remote(&workspace_id, SyncEntityType::WorkspaceVariable, &local.id)
            .await,
        Err(SyncError::CompatibilityWaiting)
    ));
    let after: (String, String, i64, String) = sqlx::query_as(
        r#"SELECT operation_id, canonical_payload_json, payload_schema_version, status
           FROM cloud_sync_outbox WHERE entity_id = ?1"#,
    )
    .bind(&local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!((after.0, after.1, after.2), before);
    assert_eq!(after.3, "pending");
}
