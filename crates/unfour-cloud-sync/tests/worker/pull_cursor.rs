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
            entity_type: SyncEntityType::WorkspaceVariable,
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
