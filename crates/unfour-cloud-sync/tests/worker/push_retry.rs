//! Uncertain sends, idempotency and permanent push outcomes.

use super::support::*;

#[tokio::test]
async fn api_request_uses_existing_uncertain_retry_and_dead_letter_paths() {
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
        .api_collection_create(workspace_id.clone(), "Retry".into())
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();

    let request = bus
        .save_api_request(saved_api_request(&workspace_id, &collection.id, None))
        .await
        .unwrap();
    transport.fail_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Transport)
    ));
    let uncertain = service.status(&workspace_id).await.unwrap();
    assert_eq!(uncertain.uncertain_count, 1);
    assert_eq!(uncertain.dead_count, 0);
    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = NULL WHERE entity_id = ?1")
        .bind(&request.id)
        .execute(db.pool())
        .await
        .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let dead = service.status(&workspace_id).await.unwrap();
    assert_eq!(dead.uncertain_count, 0);
    assert_eq!(dead.dead_count, 1);
    assert_eq!(dead.dead_letters[0].entity_type, "apiRequest");
    assert_eq!(dead.dead_letters[0].entity_id, request.id);
    assert_eq!(dead.dead_letters[0].error_code, "invalid_sync_entity");
}

#[tokio::test]
async fn workspace_deleted_preserves_the_entire_batch_without_dead_letters() {
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
    bus.workspace_variable_create(workspace_id.clone(), variable(None, "KEEP_A", "a", false))
        .await
        .unwrap();
    bus.workspace_variable_create(workspace_id.clone(), variable(None, "KEEP_B", "b", false))
        .await
        .unwrap();
    transport
        .workspace_deleted_pushes
        .store(1, Ordering::SeqCst);

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::WorkspaceDeleted)
    ));
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.pending_count, 2);
    assert_eq!(status.dead_count, 0);
    let binding = status.binding.unwrap();
    assert_eq!(binding.state, "error");
    assert_eq!(
        binding.last_error.as_deref(),
        Some("cloud_sync_workspace_deleted")
    );
    let diagnostics = service.diagnostics(&workspace_id).await.unwrap().unwrap();
    assert_eq!(
        diagnostics.last_server_request_id.as_deref(),
        Some("server-request-deleted")
    );
    assert_eq!(
        diagnostics.last_server_error_code.as_deref(),
        Some("sync_workspace_deleted")
    );
    assert_eq!(diagnostics.last_http_status, Some(409));
    assert_eq!(diagnostics.last_sync_phase.as_deref(), Some("push"));
}

#[tokio::test]
async fn lost_response_replays_same_operation_as_no_op_and_recovers() {
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
    transport.cursor.store(0, Ordering::SeqCst);
    let local = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "RETRY", "value", false),
        )
        .await
        .unwrap();
    transport.fail_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Transport)
    ));
    let first_id = transport.pushes.lock().unwrap()[0].operations[0]
        .operation_id
        .clone();
    let uncertain = service.status(&workspace_id).await.unwrap();
    assert_eq!(uncertain.uncertain_count, 1);
    assert_eq!(uncertain.binding.unwrap().last_pulled_cursor, 0);
    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = NULL")
        .execute(db.pool())
        .await
        .unwrap();
    transport.no_op_pushes.store(1, Ordering::SeqCst);
    transport
        .changes
        .lock()
        .unwrap()
        .push_back(transport.terminal_page(0, "cloud-created"));
    let push_barrier = Arc::new(Barrier::new(2));
    *transport.push_barrier.lock().unwrap() = Some(push_barrier.clone());
    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    let worker = {
        let restarted = restarted.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { restarted.sync_workspace(&workspace_id).await })
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
            &first_id,
            &local.id,
            "RETRY",
            "value",
        )],
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());
    push_barrier.wait().await;
    barrier.wait().await;
    assert_eq!(
        restarted
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
    let pushes = transport.pushes.lock().unwrap();
    assert_eq!(pushes[1].operations[0].operation_id, first_id);
    drop(pushes);
    assert_eq!(
        restarted.status(&workspace_id).await.unwrap().pending_count,
        0
    );
    let status = restarted.status(&workspace_id).await.unwrap();
    assert_eq!(status.uncertain_count, 0);
    assert_eq!(status.binding.unwrap().last_pulled_cursor, 1);
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
}
