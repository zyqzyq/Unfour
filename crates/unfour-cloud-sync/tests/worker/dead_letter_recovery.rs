use super::support::*;

#[tokio::test]
async fn initial_dead_letter_survives_restart_and_retries_with_a_new_operation_id() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());

    assert!(matches!(
        service.enable(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let failed = service.status(&workspace_id).await.unwrap();
    assert_eq!(failed.dead_count, 1);
    assert_eq!(failed.dead_letters.len(), 1);
    assert_eq!(failed.dead_letters[0].entity_type, "workspace");
    assert_eq!(failed.dead_letters[0].error_code, "invalid_sync_entity");
    assert!(failed.dead_letters[0].entity_name.is_some());
    assert_eq!(failed.binding.as_ref().unwrap().state, "error");
    assert_eq!(
        failed.binding.as_ref().unwrap().last_error.as_deref(),
        Some("cloud_sync_dead_letter_blocked")
    );
    let push_count = transport.pushes.lock().unwrap().len();
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::DeadLetterBlocked)
    ));
    assert_eq!(transport.pushes.lock().unwrap().len(), push_count);
    assert_ne!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .state,
        "active"
    );

    let old_operation_id = failed.dead_letters[0].operation_id.clone();
    let diagnostics = service.diagnostics(&workspace_id).await.unwrap().unwrap();
    assert_eq!(diagnostics.pending_outbox_count, 1);
    assert_eq!(diagnostics.dead_outbox_count, 1);
    assert_eq!(diagnostics.dead_letters[0].operation_id, old_operation_id);

    drop(service);
    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    let restarted_status = restarted.status(&workspace_id).await.unwrap();
    assert_eq!(
        restarted_status.dead_letters[0].operation_id,
        old_operation_id
    );
    let new_operation_id = restarted
        .retry_dead_letter_current_local(&workspace_id, &old_operation_id)
        .await
        .unwrap();
    assert_ne!(new_operation_id, old_operation_id);
    let pushes = transport.pushes.lock().unwrap();
    assert_eq!(pushes.len(), 2);
    assert_eq!(pushes[0].operations[0].operation_id, old_operation_id);
    assert_eq!(pushes[1].operations[0].operation_id, new_operation_id);
    drop(pushes);
    let repaired = restarted.status(&workspace_id).await.unwrap();
    assert_eq!(repaired.dead_count, 0);
    assert!(repaired.dead_letters.is_empty());
    assert_eq!(repaired.binding.unwrap().state, "active");
    let old_attempt: String =
        sqlx::query_scalar("SELECT status FROM cloud_sync_attempts WHERE operation_id = ?1")
            .bind(old_operation_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(old_attempt, "failed");
}

#[tokio::test]
async fn legacy_batch_dead_letters_are_revived_with_the_repaired_parent() {
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
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    let folder = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Folder".into(),
        )
        .await
        .unwrap();
    let request = bus
        .save_api_request(saved_api_request(
            &workspace_id,
            &collection.id,
            Some(folder.id.clone()),
        ))
        .await
        .unwrap();
    // Simulate the pre-fix client, which had no operation-level details and
    // therefore persisted every atomically rolled-back row as dead.
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let polluted = service.status(&workspace_id).await.unwrap();
    assert_eq!(polluted.dead_count, 3);
    let collection_dead = polluted
        .dead_letters
        .iter()
        .find(|letter| letter.entity_id == collection.id)
        .expect("dead collection root")
        .operation_id
        .clone();

    service
        .retry_dead_letter_current_local(&workspace_id, &collection_dead)
        .await
        .unwrap();
    let repaired = service.status(&workspace_id).await.unwrap();
    assert_eq!(repaired.dead_count, 0);
    assert_eq!(repaired.pending_count, 0);
    assert_eq!(repaired.binding.unwrap().state, "active");
    let pushed_ids = transport
        .pushes
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .operations
        .iter()
        .map(|operation| operation.entity_id.clone())
        .collect::<Vec<_>>();
    assert!(pushed_ids.contains(&collection.id));
    assert!(pushed_ids.contains(&folder.id));
    assert!(pushed_ids.contains(&request.id));
}

#[tokio::test]
async fn incremental_dead_letter_cannot_return_to_synced_until_local_repair() {
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
    let created = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "BLOCKED", "first", false),
        )
        .await
        .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::DeadLetterBlocked)
    ));
    let blocked = service.status(&workspace_id).await.unwrap();
    assert_eq!(blocked.dead_count, 1);
    assert_ne!(blocked.binding.unwrap().state, "active");

    bus.workspace_variable_update(
        workspace_id.clone(),
        created.id.clone(),
        variable(Some(created.id), "BLOCKED", "repaired", false),
    )
    .await
    .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    let repaired = service.status(&workspace_id).await.unwrap();
    assert_eq!(repaired.dead_count, 0);
    assert_eq!(repaired.binding.unwrap().state, "active");
}

#[tokio::test]
async fn dead_letter_use_remote_restores_the_cloud_entity_atomically() {
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
    let created = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "REMOTE_WINS", "cloud value", false),
        )
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    let remote_payload = transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|request| &request.operations)
        .find(|operation| operation.entity_id == created.id)
        .and_then(|operation| operation.payload.clone())
        .unwrap();

    bus.workspace_variable_update(
        workspace_id.clone(),
        created.id.clone(),
        variable(
            Some(created.id.clone()),
            "REMOTE_WINS",
            "blocked local value",
            false,
        ),
    )
    .await
    .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let blocked = service.status(&workspace_id).await.unwrap();
    let dead = blocked.dead_letters[0].clone();
    let binding = blocked.binding.unwrap();
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
        at_cursor: binding.last_pulled_cursor,
        current_cursor: binding.last_pulled_cursor,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::WorkspaceVariable,
            entity_id: created.id.clone(),
            parent_entity_id: Some(workspace_id.clone()),
            server_version: 1,
            payload_schema_version: 1,
            payload: remote_payload,
        }],
        next_page_token: None,
    });

    drop(service);
    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport);
    restarted
        .use_remote_dead_letter(&workspace_id, &dead.operation_id)
        .await
        .unwrap();
    let local_value: String = sqlx::query_scalar(
        "SELECT value FROM workspace_variables WHERE workspace_id = ?1 AND id = ?2",
    )
    .bind(&workspace_id)
    .bind(&created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(local_value, "cloud value");
    let persisted: (i64, String, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM cloud_sync_outbox WHERE operation_id = ?1),
             (SELECT sync_status FROM cloud_sync_entity_state
              WHERE account_id = 'account-a' AND entity_id = ?2),
             (SELECT server_version FROM cloud_sync_entity_state
              WHERE account_id = 'account-a' AND entity_id = ?2)"#,
    )
    .bind(&dead.operation_id)
    .bind(&created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(persisted, (0, "synced".into(), 1));
    let status = restarted.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 0);
    assert_eq!(status.binding.unwrap().state, "active");
}

#[tokio::test]
async fn failed_dead_letter_recovery_keeps_the_same_dead_entry_after_restart() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert!(matches!(
        service.enable(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let blocked = service.status(&workspace_id).await.unwrap();
    let dead = blocked.dead_letters[0].clone();
    let binding = blocked.binding.unwrap();
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION + 1,
        cloud_workspace_id: binding.cloud_workspace_id,
        at_cursor: binding.last_pulled_cursor,
        current_cursor: binding.last_pulled_cursor,
        items: Vec::new(),
        next_page_token: None,
    });
    drop(service);

    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport);
    assert!(matches!(
        restarted
            .use_remote_dead_letter(&workspace_id, &dead.operation_id)
            .await,
        Err(SyncError::InvalidData)
    ));
    let still_dead = restarted.status(&workspace_id).await.unwrap();
    assert_eq!(still_dead.dead_count, 1);
    assert_eq!(still_dead.dead_letters[0].operation_id, dead.operation_id);
    let row: (String, String) = sqlx::query_as(
        "SELECT operation_id, status FROM cloud_sync_outbox WHERE operation_id = ?1",
    )
    .bind(&dead.operation_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row, (dead.operation_id, "dead".into()));
}

#[tokio::test]
async fn cursor_change_during_remote_recovery_rolls_back_and_keeps_dead() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert!(matches!(
        service.enable(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let blocked = service.status(&workspace_id).await.unwrap();
    let dead = blocked.dead_letters[0].clone();
    let binding = blocked.binding.unwrap();
    let root_payload = transport.pushes.lock().unwrap()[0].operations[0]
        .payload
        .clone()
        .unwrap();
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
        at_cursor: binding.last_pulled_cursor,
        current_cursor: binding.last_pulled_cursor,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Workspace,
            entity_id: workspace_id.clone(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: 1,
            payload: root_payload,
        }],
        next_page_token: None,
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.snapshot_barrier.lock().unwrap() = Some(barrier.clone());
    drop(service);
    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport);
    let recovery = {
        let restarted = restarted.clone();
        let workspace_id = workspace_id.clone();
        let operation_id = dead.operation_id.clone();
        tokio::spawn(async move {
            restarted
                .use_remote_dead_letter(&workspace_id, &operation_id)
                .await
        })
    };
    barrier.wait().await;
    sqlx::query(
        "UPDATE cloud_sync_workspace_bindings SET last_pulled_cursor = last_pulled_cursor + 1",
    )
    .execute(db.pool())
    .await
    .unwrap();
    barrier.wait().await;
    assert!(matches!(
        recovery.await.unwrap(),
        Err(SyncError::AccountChanged)
    ));
    let still_dead = restarted.status(&workspace_id).await.unwrap();
    assert_eq!(still_dead.dead_count, 1);
    assert_eq!(still_dead.dead_letters[0].operation_id, dead.operation_id);
}

#[tokio::test]
async fn use_remote_on_absent_api_collection_refuses_while_local_children_exist() {
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
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    let folder = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Root".into(),
        )
        .await
        .unwrap();
    bus.save_api_request(saved_api_request(
        &workspace_id,
        &collection.id,
        Some(folder.id.clone()),
    ))
    .await
    .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let blocked = service.status(&workspace_id).await.unwrap();
    let collection_dead = blocked
        .dead_letters
        .iter()
        .find(|letter| letter.entity_id == collection.id)
        .expect("collection dead letter")
        .clone();
    assert_eq!(collection_dead.entity_name.as_deref(), Some("Accounts"));
    let binding = blocked.binding.unwrap();
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
        at_cursor: binding.last_pulled_cursor,
        current_cursor: binding.last_pulled_cursor,
        items: Vec::new(),
        next_page_token: None,
    });

    assert!(matches!(
        service
            .use_remote_dead_letter(&workspace_id, &collection_dead.operation_id)
            .await,
        Err(SyncError::SafeReplaceUnavailable)
    ));
    let survived: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM api_collections WHERE id = ?1 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM api_collection_folders WHERE collection_id = ?1 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM api_requests WHERE collection_id = ?1 AND deleted_at IS NULL)"#,
    )
    .bind(&collection.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(survived, (1, 1, 1));
    let still_dead: String =
        sqlx::query_scalar("SELECT status FROM cloud_sync_outbox WHERE operation_id = ?1")
            .bind(&collection_dead.operation_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(still_dead, "dead");
}

#[tokio::test]
async fn use_remote_on_absent_childless_api_collection_deletes_locally() {
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
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Solo".into())
        .await
        .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let blocked = service.status(&workspace_id).await.unwrap();
    let dead = blocked.dead_letters[0].clone();
    assert_eq!(dead.entity_type, "apiCollection");
    let binding = blocked.binding.unwrap();
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
        at_cursor: binding.last_pulled_cursor,
        current_cursor: binding.last_pulled_cursor,
        items: Vec::new(),
        next_page_token: None,
    });

    service
        .use_remote_dead_letter(&workspace_id, &dead.operation_id)
        .await
        .unwrap();
    let deleted_at: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM api_collections WHERE id = ?1")
            .bind(&collection.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(deleted_at.is_some());
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 0);
    assert_eq!(status.binding.unwrap().state, "active");
}

#[tokio::test]
async fn oversized_api_operation_parks_as_repairable_dead_letter_and_batch_continues() {
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
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Big".into())
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();

    let mut oversized = saved_api_request(&workspace_id, &collection.id, None);
    oversized.body = Some(format!(r#"{{"blob":"{}"}}"#, "x".repeat(600 * 1024)));
    let request = bus.save_api_request(oversized).await.unwrap();
    let variable = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "STILL_FLOWS", "value", false),
        )
        .await
        .unwrap();
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::DeadLetterBlocked)
    ));

    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 1);
    assert_eq!(status.pending_count, 0);
    assert_eq!(status.dead_letters[0].entity_type, "apiRequest");
    assert_eq!(status.dead_letters[0].entity_id, request.id);
    assert_eq!(status.dead_letters[0].error_code, "payload_too_large");
    assert_eq!(
        status.dead_letters[0].entity_name.as_deref(),
        Some("List accounts")
    );
    let variable_pushed = transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|push| &push.operations)
        .any(|operation| operation.entity_id == variable.id);
    assert!(
        variable_pushed,
        "other pending entries must still push in the same round"
    );
}

#[tokio::test]
async fn protocol_dead_letters_revive_once_per_process_after_upgrade() {
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
    let created = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "GATED", "value", false),
        )
        .await
        .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    // Simulate an old build whose pushes were rejected by the server's
    // protocol gate before this upgrade.
    sqlx::query(
        "UPDATE cloud_sync_outbox SET last_error = 'protocol_version_unsupported' WHERE status = 'dead'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    drop(service);

    let (upgraded, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    upgraded.sync_workspace(&workspace_id).await.unwrap();
    let status = upgraded.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 0);
    assert_eq!(status.binding.unwrap().state, "active");
    let pushed = transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|push| &push.operations)
        .filter(|operation| operation.entity_id == created.id)
        .count();
    assert_eq!(pushed, 2, "the revived entry must be pushed again");

    // The same process revives only once: a fresh protocol dead letter stays
    // dead until the next restart.
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    bus.workspace_variable_update(
        workspace_id.clone(),
        created.id.clone(),
        variable(Some(created.id.clone()), "GATED", "again", false),
    )
    .await
    .unwrap();
    assert!(matches!(
        upgraded.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    sqlx::query(
        "UPDATE cloud_sync_outbox SET last_error = 'protocol_version_unsupported' WHERE status = 'dead'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(matches!(
        upgraded.sync_workspace(&workspace_id).await,
        Err(SyncError::DeadLetterBlocked)
    ));
    assert_eq!(upgraded.status(&workspace_id).await.unwrap().dead_count, 1);
}

#[tokio::test]
async fn unauthorized_push_becomes_dead_and_is_not_automatically_retried() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus = CommandBus::from_db_with_extensions(db, CommandBusExtensions::new(vec![hook]))
        .await
        .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "AUTH_BLOCKED", "value", false),
    )
    .await
    .unwrap();
    transport.unauthorized_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Unauthorized)
    ));
    let blocked = service.status(&workspace_id).await.unwrap();
    assert_eq!(blocked.dead_count, 1);
    assert_eq!(blocked.dead_letters[0].error_code, "unauthorized");
    let pushes = transport.pushes.lock().unwrap().len();
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::DeadLetterBlocked)
    ));
    assert_eq!(transport.pushes.lock().unwrap().len(), pushes);
}

#[tokio::test]
async fn incremental_dead_letter_retry_uses_current_local_payload_and_server_base() {
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
    let created = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "RETRY_LOCAL", "cloud baseline", false),
        )
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    bus.workspace_variable_update(
        workspace_id.clone(),
        created.id.clone(),
        variable(
            Some(created.id.clone()),
            "RETRY_LOCAL",
            "current local value",
            false,
        ),
    )
    .await
    .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let old_operation_id = service.status(&workspace_id).await.unwrap().dead_letters[0]
        .operation_id
        .clone();
    drop(service);

    let (restarted, _, _) = SyncRuntime::build(db, transport.clone());
    let new_operation_id = restarted
        .retry_dead_letter_current_local(&workspace_id, &old_operation_id)
        .await
        .unwrap();
    let operation = transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|request| &request.operations)
        .find(|operation| operation.operation_id == new_operation_id)
        .cloned()
        .unwrap();
    assert_ne!(operation.operation_id, old_operation_id);
    assert_eq!(operation.base_version, 1);
    assert_eq!(operation.payload.unwrap()["value"], "current local value");
    assert_eq!(restarted.status(&workspace_id).await.unwrap().dead_count, 0);
}
