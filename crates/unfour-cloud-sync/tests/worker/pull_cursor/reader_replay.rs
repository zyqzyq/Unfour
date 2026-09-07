//! Reader-revision replay of retained remote envelopes.

use super::*;

#[tokio::test]
async fn restarted_new_reader_reclassifies_and_applies_a_retained_complete_envelope() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (old_runtime, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    old_runtime.enable(&workspace_id).await.unwrap();
    let binding = old_runtime
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let payload = variable_payload("retained-value");

    // This row models a complete envelope retained by an older reader. The
    // current runtime must re-evaluate compatibility instead of trusting the
    // persisted deferred classification or rebuilding from local defaults.
    sqlx::query(
        r#"INSERT INTO cloud_sync_remote_entity (
             account_id, cloud_workspace_id, entity_type, entity_id,
             parent_entity_id, server_version, payload_schema_version,
             operation, canonical_payload_json, deleted_at, operation_id,
             compatibility_state, created_at, updated_at
           ) VALUES (?1, ?2, 'workspaceVariable', 'reader-upgrade-variable',
                     ?3, 7, 1, 'upsert', ?4, NULL, 'retained-operation',
                     'deferred_compatibility', '2026-08-13T00:00:00Z',
                     '2026-08-13T00:00:00Z')"#,
    )
    .bind(&binding.account_id)
    .bind(&binding.cloud_workspace_id)
    .bind(&workspace_id)
    .bind(serde_json::to_string(&payload).unwrap())
    .execute(db.pool())
    .await
    .unwrap();
    drop(old_runtime);

    let (upgraded_runtime, _, _) = SyncRuntime::build(db.clone(), transport);
    upgraded_runtime
        .sync_workspace(&workspace_id)
        .await
        .unwrap();

    let value: String = sqlx::query_scalar(
        "SELECT value FROM workspace_variables WHERE id = 'reader-upgrade-variable'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(value, "retained-value");
    let remote: (String, String, i64) = sqlx::query_as(
        r#"SELECT compatibility_state, canonical_payload_json, payload_schema_version
           FROM cloud_sync_remote_entity WHERE entity_id = 'reader-upgrade-variable'"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remote.0, "supported");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&remote.1).unwrap(),
        payload
    );
    assert_eq!(remote.2, 1);
    let state: (i64, String, Option<String>) = sqlx::query_as(
        r#"SELECT server_version, sync_status, last_operation_id
           FROM cloud_sync_entity_state WHERE entity_id = 'reader-upgrade-variable'"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        state,
        (7, "synced".into(), Some("retained-operation".into()))
    );
}

#[tokio::test]
async fn retained_snapshot_replay_preserves_local_intent_without_inventing_operation_id() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (old_runtime, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    old_runtime.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(
                Some("snapshot-replay-conflict".into()),
                "LOCAL",
                "local-intent",
                false,
            ),
        )
        .await
        .unwrap();
    let entity_id = local.id;
    let binding = old_runtime
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let mut remote_payload = variable_payload("remote-value");
    remote_payload["key"] = serde_json::json!("REMOTE");
    sqlx::query(
        r#"INSERT INTO cloud_sync_remote_entity (
             account_id, cloud_workspace_id, entity_type, entity_id,
             parent_entity_id, server_version, payload_schema_version,
             operation, canonical_payload_json, deleted_at, operation_id,
             compatibility_state, created_at, updated_at
           ) VALUES (?1, ?2, 'workspaceVariable', ?3, ?4, 9, 1, 'upsert',
                     ?5, NULL, NULL, 'deferred_compatibility',
                     '2026-08-13T00:00:00Z', '2026-08-13T00:00:00Z')"#,
    )
    .bind(&binding.account_id)
    .bind(&binding.cloud_workspace_id)
    .bind(&entity_id)
    .bind(&workspace_id)
    .bind(serde_json::to_string(&remote_payload).unwrap())
    .execute(db.pool())
    .await
    .unwrap();
    drop(old_runtime);

    let (upgraded_runtime, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    let replay_result = upgraded_runtime.sync_workspace(&workspace_id).await;
    assert!(
        matches!(replay_result, Err(SyncError::Conflict)),
        "unexpected replay result: {replay_result:?}"
    );
    let local_value: String =
        sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = ?1")
            .bind(&entity_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(local_value, "local-intent");
    let conflict: (String, i64, Option<String>) = sqlx::query_as(
        r#"SELECT sync_status, conflict_payload_schema_version, conflict_operation_id
           FROM cloud_sync_entity_state WHERE entity_id = ?1"#,
    )
    .bind(&entity_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(conflict, ("conflict".into(), 1, None));
    let outbox: (String, i64) = sqlx::query_as(
        "SELECT status, payload_schema_version FROM cloud_sync_outbox WHERE entity_id = ?1",
    )
    .bind(&entity_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(outbox, ("pending".into(), 1));
    assert!(transport.pushes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn upgraded_reader_replays_retained_envelope_to_restore_additive_field() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    service.enable(&workspace_id).await.unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let mut payload = variable_payload("A");
    payload["key"] = serde_json::json!("A");
    payload["description"] = serde_json::json!("REMOTE_VALUE");
    payload["futureOptionalField"] = serde_json::json!("REMOTE_VALUE");
    sqlx::query(
        r#"INSERT INTO cloud_sync_remote_entity (
             account_id, cloud_workspace_id, entity_type, entity_id,
             parent_entity_id, server_version, payload_schema_version,
             operation, canonical_payload_json, deleted_at, operation_id,
             compatibility_state, created_at, updated_at
           ) VALUES (?1, ?2, 'workspaceVariable', 'reader-additive-variable',
                     ?3, 7, 1, 'upsert', ?4, NULL, 'retained-v7',
                     'supported', '2026-09-06T00:00:00Z', '2026-09-06T00:00:00Z')"#,
    )
    .bind(&binding.account_id)
    .bind(&binding.cloud_workspace_id)
    .bind(&workspace_id)
    .bind(serde_json::to_string(&payload).unwrap())
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO workspace_variables (
             id, workspace_id, key, value, is_secret, is_enabled, description,
             sort_order, created_at, updated_at, revision
           ) VALUES ('reader-additive-variable', ?1, 'A', 'A', 0, 1, NULL,
                     0, '2026-07-28T00:00:00Z', '2026-07-28T00:00:00Z', 1)"#,
    )
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO cloud_sync_entity_state (
             account_id, cloud_workspace_id, entity_type, entity_id,
             server_version, last_operation_id, sync_status,
             applied_reader_revision, updated_at
           ) VALUES (?1, ?2, 'workspaceVariable', 'reader-additive-variable',
                     7, 'retained-v7', 'synced', 0, '2026-09-06T00:00:00Z')"#,
    )
    .bind(&binding.account_id)
    .bind(&binding.cloud_workspace_id)
    .execute(db.pool())
    .await
    .unwrap();

    service.sync_workspace(&workspace_id).await.unwrap();

    let restored: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT key, description, revision FROM workspace_variables WHERE id = 'reader-additive-variable'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(restored.0, "A");
    assert_eq!(restored.1.as_deref(), Some("REMOTE_VALUE"));
    let remote: String = sqlx::query_scalar(
        "SELECT canonical_payload_json FROM cloud_sync_remote_entity WHERE entity_id = 'reader-additive-variable'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(remote.contains("futureOptionalField"));
    let state: (i64, String, i64) = sqlx::query_as(
        r#"SELECT server_version, sync_status, applied_reader_revision
           FROM cloud_sync_entity_state WHERE entity_id = 'reader-additive-variable'"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        state,
        (
            7,
            "synced".into(),
            SyncEntityType::WorkspaceVariable.reader_revision()
        )
    );
}

#[tokio::test]
async fn reader_revision_upgrade_does_not_overwrite_pending_local_intent() {
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
            variable(
                Some("pending-reader-upgrade".into()),
                "LOCAL",
                "local-intent",
                false,
            ),
        )
        .await
        .unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let mut remote_payload = variable_payload("REMOTE_VALUE");
    remote_payload["key"] = serde_json::json!("REMOTE");
    sqlx::query(
        r#"INSERT INTO cloud_sync_remote_entity (
             account_id, cloud_workspace_id, entity_type, entity_id,
             parent_entity_id, server_version, payload_schema_version,
             operation, canonical_payload_json, deleted_at, operation_id,
             compatibility_state, created_at, updated_at
           ) VALUES (?1, ?2, 'workspaceVariable', ?3, ?4, 7, 1, 'upsert',
                     ?5, NULL, 'remote-v7', 'supported',
                     '2026-09-06T00:00:00Z', '2026-09-06T00:00:00Z')"#,
    )
    .bind(&binding.account_id)
    .bind(&binding.cloud_workspace_id)
    .bind(&local.id)
    .bind(&workspace_id)
    .bind(serde_json::to_string(&remote_payload).unwrap())
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO cloud_sync_entity_state (
             account_id, cloud_workspace_id, entity_type, entity_id,
             server_version, last_operation_id, sync_status,
             applied_reader_revision, updated_at
           ) VALUES (?1, ?2, 'workspaceVariable', ?3, 7, 'remote-v7', 'synced',
                     0, '2026-09-06T00:00:00Z')
           ON CONFLICT(account_id, cloud_workspace_id, entity_type, entity_id) DO UPDATE SET
             server_version = 7, sync_status = 'synced', applied_reader_revision = 0"#,
    )
    .bind(&binding.account_id)
    .bind(&binding.cloud_workspace_id)
    .bind(&local.id)
    .execute(db.pool())
    .await
    .unwrap();

    let replay_result = service.sync_workspace(&workspace_id).await;
    assert!(
        matches!(replay_result, Err(SyncError::Conflict)),
        "unexpected replay result: {replay_result:?}"
    );
    let local_value: String =
        sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(local_value, "local-intent");
    let status: String =
        sqlx::query_scalar("SELECT sync_status FROM cloud_sync_entity_state WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(status, "conflict");
    let outbox: String =
        sqlx::query_scalar("SELECT status FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(outbox, "pending");
}

#[tokio::test]
async fn current_reader_materialized_entity_is_not_reapplied_on_repeat_sync() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let mut payload = variable_payload("stable");
    payload["key"] = serde_json::json!("STABLE");
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: binding.last_pulled_cursor + 1,
        next_cursor: binding.last_pulled_cursor + 1,
        changes: vec![RemoteChange {
            cursor: binding.last_pulled_cursor + 1,
            operation_id: "stable-apply".into(),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: "stable-variable".into(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Upsert,
            server_version: 3,
            payload_schema_version: SyncEntityType::WorkspaceVariable.payload_schema_version(),
            payload: Some(payload),
            deleted_at: None,
        }],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let first: (i64, i64) = sqlx::query_as(
        r#"SELECT variable.revision, state.applied_reader_revision
           FROM workspace_variables variable
           JOIN cloud_sync_entity_state state ON state.entity_id = variable.id
           WHERE variable.id = 'stable-variable'"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(first.1, SyncEntityType::WorkspaceVariable.reader_revision());
    service.sync_workspace(&workspace_id).await.unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    let second: (i64, i64, String) = sqlx::query_as(
        r#"SELECT variable.revision, state.applied_reader_revision, state.sync_status
           FROM workspace_variables variable
           JOIN cloud_sync_entity_state state ON state.entity_id = variable.id
           WHERE variable.id = 'stable-variable'"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(second, (first.0, first.1, "synced".into()));
}
