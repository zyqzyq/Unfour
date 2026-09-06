//! Incremental cursor continuity and acknowledgement of own operations.

use super::support::*;

#[tokio::test]
async fn push_global_cursor_does_not_skip_interleaved_remote_changes() {
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
    let operation_id: String =
        sqlx::query_scalar("SELECT operation_id FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    sqlx::query(
        "UPDATE cloud_sync_workspace_bindings SET last_pulled_cursor = 10 WHERE local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.cursor.store(11, Ordering::SeqCst);
    transport
        .changes
        .lock()
        .unwrap()
        .push_back(transport.terminal_page(10, "cloud-created"));
    let push_barrier = Arc::new(Barrier::new(2));
    *transport.push_barrier.lock().unwrap() = Some(push_barrier.clone());
    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };

    push_barrier.wait().await;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: 12,
        next_cursor: 12,
        changes: vec![
            remote_variable_change(
                &workspace_id,
                11,
                "remote-interleaved",
                "remote-variable",
                "REMOTE",
                "remote-value",
            ),
            remote_variable_change(
                &workspace_id,
                12,
                &operation_id,
                &local.id,
                "LOCAL",
                "local-value",
            ),
        ],
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());
    push_barrier.wait().await;
    barrier.wait().await;
    assert_eq!(transport.cursor.load(Ordering::SeqCst), 12);
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        10,
        "a Push response at the workspace's global cursor must not acknowledge unseen changes",
    );
    barrier.wait().await;
    worker.await.unwrap().unwrap();

    let values: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, value FROM workspace_variables WHERE id IN (?1, 'remote-variable') ORDER BY id",
    )
    .bind(&local.id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        values,
        vec![
            (local.id, "local-value".into()),
            ("remote-variable".into(), "remote-value".into()),
        ]
    );
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        12,
    );
}

#[tokio::test]
async fn own_pushed_change_is_consumed_once_before_pull_cursor_advances() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.cursor.store(0, Ordering::SeqCst);
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local = bus
        .workspace_variable_create(workspace_id.clone(), variable(None, "OWN", "one", false))
        .await
        .unwrap();
    let operation_id: String =
        sqlx::query_scalar("SELECT operation_id FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    transport
        .changes
        .lock()
        .unwrap()
        .push_back(transport.terminal_page(0, "cloud-created"));
    let push_barrier = Arc::new(Barrier::new(2));
    *transport.push_barrier.lock().unwrap() = Some(push_barrier.clone());
    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };

    push_barrier.wait().await;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: 1,
        next_cursor: 1,
        changes: vec![remote_variable_change(
            &workspace_id,
            1,
            &operation_id,
            &local.id,
            "OWN",
            "one",
        )],
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());
    push_barrier.wait().await;
    barrier.wait().await;
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        0,
    );
    barrier.wait().await;
    worker.await.unwrap().unwrap();

    let business_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workspace_variables WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let outbox_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(business_rows, 1);
    assert_eq!(outbox_rows, 0);
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        1,
    );
}

#[tokio::test]
async fn multi_page_changes_continue_until_next_cursor_equals_current_cursor() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let base = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    let page = |cursor: i64, id: &str, value: &str| {
        let mut payload = variable_payload(value);
        payload["key"] = serde_json::json!(id);
        RemoteChange {
            cursor,
            operation_id: format!("remote-{cursor}"),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: id.into(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: 1,
            payload: Some(payload),
            deleted_at: None,
        }
    };
    transport.changes.lock().unwrap().extend([
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-created".into(),
            current_cursor: base + 2,
            next_cursor: base + 1,
            changes: vec![page(base + 1, "remote-one", "one")],
        },
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-created".into(),
            current_cursor: base + 2,
            next_cursor: base + 2,
            changes: vec![page(base + 2, "remote-two", "two")],
        },
    ]);
    service.sync_workspace(&workspace_id).await.unwrap();
    let values: Vec<String> = sqlx::query_scalar(
        "SELECT value FROM workspace_variables WHERE id IN ('remote-one', 'remote-two') ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(values, vec!["one", "two"]);
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        base + 2
    );
}

#[tokio::test]
async fn pull_retains_future_entities_advances_cursor_and_blocks_unsafe_apply() {
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
    let base = binding.last_pulled_cursor;

    let mut first_payload = variable_payload("one");
    first_payload["key"] = serde_json::json!("FIRST");
    let future_api_payload = serde_json::json!({
        "collectionId": "future-collection",
        "parentFolderId": null,
        "name": "Future body mode",
        "sortOrder": 0,
        "authJson": "{}",
        "method": "POST",
        "url": "https://example.test/future",
        "headers": [],
        "query": [],
        "body": "future",
        "bodyKind": "multipart",
        "settingsJson": "{\"timeoutMs\":null}",
        "preRequestScript": null,
        "postResponseScript": null,
        "scriptSchemaVersion": 1,
        "createdAt": "2026-08-13T00:00:00Z",
        "updatedAt": "2026-08-13T00:00:00Z"
    });
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: base + 5,
        next_cursor: base + 5,
        changes: vec![
            RemoteChange {
                cursor: base + 1,
                operation_id: "known-before".into(),
                entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
                entity_id: "known-before".into(),
                parent_entity_id: Some(workspace_id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(first_payload),
                deleted_at: None,
            },
            RemoteChange {
                cursor: base + 2,
                operation_id: "future-entity".into(),
                entity_type: "flowNode".into(),
                entity_id: "future-node".into(),
                parent_entity_id: Some(workspace_id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(serde_json::json!({"future": true})),
                deleted_at: None,
            },
            RemoteChange {
                cursor: base + 3,
                operation_id: "future-subtype".into(),
                entity_type: SyncEntityType::ApiRequest.as_str().into(),
                entity_id: "future-api-request".into(),
                parent_entity_id: Some("future-collection".into()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(future_api_payload),
                deleted_at: None,
            },
            RemoteChange {
                cursor: base + 4,
                operation_id: "child-of-future-parent".into(),
                entity_type: SyncEntityType::WorkspaceEnvironmentVariable.as_str().into(),
                entity_id: "blocked-known-child".into(),
                parent_entity_id: Some("future-node".into()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(variable_payload("must-not-apply")),
                deleted_at: None,
            },
            remote_variable_change(
                &workspace_id,
                base + 5,
                "known-after",
                "known-after",
                "AFTER",
                "two",
            ),
        ],
    });

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::CompatibilityWaiting)
    ));

    let values: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, value FROM workspace_variables WHERE id IN ('known-before', 'known-after') ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(values, Vec::<(String, String)>::new());
    let skipped_business_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM api_requests WHERE id = 'future-api-request'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(skipped_business_rows, 0);
    let skipped_state_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_entity_state WHERE entity_id IN ('future-node', 'future-api-request', 'blocked-known-child')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(skipped_state_rows, 0);

    let retained: Vec<(String, String, i64, String)> = sqlx::query_as(
        r#"SELECT entity_type, entity_id, server_version, compatibility_state
           FROM cloud_sync_remote_entity
           WHERE cloud_workspace_id = ?1 ORDER BY entity_id"#,
    )
    .bind(&binding.cloud_workspace_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(retained.len(), 5);
    assert!(retained.iter().any(|row| {
        row.0 == "flowNode" && row.1 == "future-node" && row.3 == "deferred_compatibility"
    }));
    let status = service.status(&workspace_id).await.unwrap();
    let binding = status.binding.unwrap();
    assert_eq!(binding.last_pulled_cursor, base + 5);
    assert_eq!(binding.state, "error");
    assert_eq!(
        binding.last_error.as_deref(),
        Some("cloud_sync_compatibility_waiting")
    );
    assert_eq!(status.dead_count, 0);
    let diagnostics: Vec<(String, String, String)> = sqlx::query_as(
        r#"SELECT error_code, entity_type, entity_id
           FROM cloud_sync_diagnostics
           WHERE error_code = 'remote_entity_compatibility_deferred'
           ORDER BY id"#,
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        diagnostics,
        vec![
            (
                "remote_entity_compatibility_deferred".into(),
                "flowNode".into(),
                "future-node".into(),
            ),
            (
                "remote_entity_compatibility_deferred".into(),
                "apiRequest".into(),
                "future-api-request".into(),
            ),
        ]
    );

    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert!(matches!(
        restarted.sync_workspace(&workspace_id).await,
        Err(SyncError::CompatibilityWaiting)
    ));
    let retained_after_restart: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_remote_entity WHERE cloud_workspace_id = ?1",
    )
    .bind(&binding.cloud_workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(retained_after_restart, 5);
    assert_eq!(
        restarted
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        base + 5,
    );
}

#[tokio::test]
async fn additive_optional_payload_field_is_retained_and_applied_without_waiting() {
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
    let mut payload = variable_payload("applied");
    payload["key"] = serde_json::json!("ADDITIVE");
    payload["futureOptionalField"] = serde_json::json!({"preserved": true});
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: binding.last_pulled_cursor + 1,
        next_cursor: binding.last_pulled_cursor + 1,
        changes: vec![RemoteChange {
            cursor: binding.last_pulled_cursor + 1,
            operation_id: "additive-field".into(),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: "variable-additive".into(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: SyncEntityType::WorkspaceVariable.payload_schema_version(),
            payload: Some(payload),
            deleted_at: None,
        }],
    });

    service.sync_workspace(&workspace_id).await.unwrap();

    let value: String =
        sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = 'variable-additive'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(value, "applied");
    let retained: (String, String) = sqlx::query_as(
        r#"SELECT canonical_payload_json, compatibility_state
           FROM cloud_sync_remote_entity WHERE entity_id = 'variable-additive'"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(retained.0.contains("futureOptionalField"));
    assert_eq!(retained.1, "supported");
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(
        status.binding.as_ref().unwrap().last_pulled_cursor,
        binding.last_pulled_cursor + 1
    );
    assert_ne!(status.binding.as_ref().unwrap().state, "error");
    assert_eq!(status.dead_count, 0);
}

#[tokio::test]
async fn future_schema_tombstone_for_known_entity_applies_without_waiting() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let variable = seed
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "DELETE_ME", "local", false),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: binding.last_pulled_cursor + 1,
        next_cursor: binding.last_pulled_cursor + 1,
        changes: vec![RemoteChange {
            cursor: binding.last_pulled_cursor + 1,
            operation_id: "future-schema-delete".into(),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: variable.id.clone(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Delete,
            server_version: 2,
            payload_schema_version: SyncEntityType::WorkspaceVariable.payload_schema_version() + 1,
            payload: None,
            deleted_at: Some("2026-09-06T01:00:00Z".into()),
        }],
    });

    service.sync_workspace(&workspace_id).await.unwrap();

    let deleted_at: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(&variable.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(deleted_at.is_some());
    let compatibility_state: String = sqlx::query_scalar(
        "SELECT compatibility_state FROM cloud_sync_remote_entity WHERE entity_id = ?1",
    )
    .bind(&variable.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(compatibility_state, "supported");
    let status = service.status(&workspace_id).await.unwrap();
    assert_ne!(status.binding.as_ref().unwrap().state, "error");
    assert_eq!(status.dead_count, 0);
}

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
async fn delete_then_restore_converges_in_page_across_pages_and_after_old_redelivery() {
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
    let base = binding.last_pulled_cursor;
    let entity_id = "delete-restore-variable";
    let upsert = |cursor: i64, server_version: i64, value: &str| RemoteChange {
        cursor,
        operation_id: format!("restore-{cursor}"),
        entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
        entity_id: entity_id.into(),
        parent_entity_id: Some(workspace_id.clone()),
        operation: SyncOperation::Upsert,
        server_version,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(variable_payload(value)),
        deleted_at: None,
    };
    let delete = |cursor: i64, server_version: i64| RemoteChange {
        cursor,
        operation_id: format!("delete-{cursor}"),
        entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
        entity_id: entity_id.into(),
        parent_entity_id: Some(workspace_id.clone()),
        operation: SyncOperation::Delete,
        server_version,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: None,
        deleted_at: Some(format!("2026-09-06T00:00:{server_version:02}Z")),
    };

    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: base + 3,
        next_cursor: base + 3,
        changes: vec![
            upsert(base + 1, 1, "created"),
            delete(base + 2, 2),
            upsert(base + 3, 3, "same-page-restored"),
        ],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let same_page: (String, Option<String>) =
        sqlx::query_as("SELECT value, deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(entity_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(same_page, ("same-page-restored".into(), None));

    transport.changes.lock().unwrap().extend([
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: binding.cloud_workspace_id.clone(),
            current_cursor: base + 5,
            next_cursor: base + 4,
            changes: vec![delete(base + 4, 4)],
        },
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: binding.cloud_workspace_id.clone(),
            current_cursor: base + 5,
            next_cursor: base + 5,
            changes: vec![upsert(base + 5, 5, "cross-page-restored")],
        },
    ]);
    service.sync_workspace(&workspace_id).await.unwrap();
    let cross_page: (String, Option<String>) =
        sqlx::query_as("SELECT value, deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(entity_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(cross_page, ("cross-page-restored".into(), None));

    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: base + 6,
        next_cursor: base + 6,
        changes: vec![delete(base + 6, 4)],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let redelivered: (String, Option<String>) =
        sqlx::query_as("SELECT value, deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(entity_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(redelivered, ("cross-page-restored".into(), None));
    let retained: (i64, String) = sqlx::query_as(
        "SELECT server_version, operation FROM cloud_sync_remote_entity WHERE entity_id = ?1",
    )
    .bind(entity_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(retained, (5, "upsert".into()));
}

#[tokio::test]
async fn gapped_changes_page_does_not_advance_the_pull_cursor() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let base = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base + 2,
        next_cursor: base + 2,
        changes: vec![remote_variable_change(
            &workspace_id,
            base + 2,
            "remote-gap",
            "remote-gap-variable",
            "GAP",
            "must-not-apply",
        )],
    });

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::InvalidData)
    ));
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.last_pulled_cursor, base);
    let applied: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_variables WHERE id = 'remote-gap-variable'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(applied, 0);
}

#[tokio::test]
async fn remote_api_request_changes_apply_each_settings_json_without_resetting_it() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let collection = seed
        .api_collection_create(workspace_id.clone(), "Remote API".into())
        .await
        .unwrap();
    let folder = seed
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Requests".into(),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let base = binding.last_pulled_cursor;

    let remote_request = |cursor: i64, operation_id: &str, settings_json: &str| RemoteChange {
        cursor,
        operation_id: operation_id.into(),
        entity_type: SyncEntityType::ApiRequest.as_str().into(),
        entity_id: "remote-request".into(),
        parent_entity_id: Some(folder.id.clone()),
        operation: SyncOperation::Upsert,
        server_version: cursor,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(serde_json::json!({
            "collectionId": collection.id.clone(),
            "parentFolderId": folder.id.clone(),
            "name": "Remote request",
            "sortOrder": 0,
            "authJson": "{}",
            "method": "GET",
            "url": "https://example.test/remote",
            "headers": [],
            "query": [],
            "body": null,
            "bodyKind": "none",
            "settingsJson": settings_json,
            "preRequestScript": null,
            "postResponseScript": null,
            "scriptSchemaVersion": 1,
            "createdAt": "2026-08-13T00:00:00Z",
            "updatedAt": "2026-08-13T00:00:00Z"
        })),
        deleted_at: None,
    };

    let accepted = [
        (r#"{"timeoutMs":null}"#, serde_json::json!(null)),
        (r#"{"timeoutMs":0}"#, serde_json::json!(0)),
        (r#"{"timeoutMs":1}"#, serde_json::json!(1)),
        (r#"{"timeoutMs":30000}"#, serde_json::json!(30000)),
        (
            r#"{"timeoutMs":9007199254740991}"#,
            serde_json::json!(9007199254740991_u64),
        ),
    ];
    for (index, (settings_json, expected_timeout)) in accepted.iter().enumerate() {
        let cursor = base + index as i64 + 1;
        transport.changes.lock().unwrap().push_back(ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: binding.cloud_workspace_id.clone(),
            current_cursor: cursor,
            next_cursor: cursor,
            changes: vec![remote_request(
                cursor,
                &format!("remote-request-{index}"),
                settings_json,
            )],
        });
        service.sync_workspace(&workspace_id).await.unwrap();
        let stored_settings: String = sqlx::query_scalar(
            "SELECT settings_json FROM api_requests WHERE id = 'remote-request'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let stored_settings: serde_json::Value = serde_json::from_str(&stored_settings).unwrap();
        assert_eq!(stored_settings["timeoutMs"], *expected_timeout);
    }

    let invalid_cursor = base + accepted.len() as i64 + 1;
    for (index, settings_json) in [r#"{"timeoutMs":9007199254740992}"#, r#"{"timeoutMs":}"#]
        .iter()
        .enumerate()
    {
        transport.changes.lock().unwrap().push_back(ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: binding.cloud_workspace_id.clone(),
            current_cursor: invalid_cursor,
            next_cursor: invalid_cursor,
            changes: vec![remote_request(
                invalid_cursor,
                &format!("remote-request-invalid-{index}"),
                settings_json,
            )],
        });
        assert!(matches!(
            service.sync_workspace(&workspace_id).await,
            Err(SyncError::Core)
        ));
        let binding_after = service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap();
        assert_eq!(binding_after.last_pulled_cursor, invalid_cursor - 1);
        let stored_settings: String = sqlx::query_scalar(
            "SELECT settings_json FROM api_requests WHERE id = 'remote-request'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(stored_settings, r#"{"timeoutMs":9007199254740991}"#);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id = 'remote-request'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        0
    );
}
