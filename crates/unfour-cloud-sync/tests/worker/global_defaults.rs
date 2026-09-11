//! Production account initialization and independent global/workspace preferences.
use super::support::*;

#[tokio::test]
async fn new_account_is_enabled_but_only_explicitly_bound_workspace_uploads() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local = bus.list_workspaces().await.unwrap().active_workspace_id;
    let opted_in = bus.create_workspace("Opted in".into()).await.unwrap();
    service.activate_account_context().await.unwrap();
    assert!(service.global_sync_enabled().await.unwrap());
    let persisted: bool = sqlx::query_scalar(
        "SELECT sync_enabled FROM cloud_sync_account_settings WHERE account_id = 'account-a'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(persisted);
    bus.workspace_variable_create(local.clone(), variable(None, "LOCAL", "only", false))
        .await
        .unwrap();
    service.sync_all().await.unwrap();
    assert!(service.status(&local).await.unwrap().binding.is_none());
    assert!(service
        .status(&opted_in.id)
        .await
        .unwrap()
        .binding
        .is_none());
    assert!(transport.pushes.lock().unwrap().is_empty());
    assert_eq!(transport.create_workspace_calls.load(Ordering::SeqCst), 0);
    let outbox: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(outbox, 0);

    service.enable(&opted_in.id).await.unwrap();
    let status = service.status(&opted_in.id).await.unwrap();
    let binding = status.binding.unwrap();
    assert!(binding.sync_enabled);
    assert_eq!(binding.state, "active");
    assert_eq!(binding.initial_confirmed, binding.initial_total);
    assert!(!transport.pushes.lock().unwrap().is_empty());
    assert!(service.status(&local).await.unwrap().binding.is_none());
}

#[tokio::test]
async fn existing_off_is_preserved_by_activation_enable_and_restart() {
    for explicit in [false, true] {
        let db = database().await;
        let bus = CommandBus::from_db(db.clone()).await.unwrap();
        let local = bus.list_workspaces().await.unwrap().active_workspace_id;
        let transport = Arc::new(MockTransport::new());
        let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
        if explicit {
            service.set_global_sync_enabled(false).await.unwrap();
        } else {
            // An old default cannot be distinguished from an explicit OFF.
            sqlx::query("INSERT INTO cloud_sync_account_settings (account_id, updated_at) VALUES ('account-a', 'legacy')")
                .execute(db.pool()).await.unwrap();
        }
        service.activate_account_context().await.unwrap();
        service.enable(&local).await.unwrap();
        assert!(!service.global_sync_enabled().await.unwrap());
        let status = service.status(&local).await.unwrap();
        assert!(status.binding.unwrap().sync_enabled);
        assert!(status.pending_count > 0);
        assert!(transport.pushes.lock().unwrap().is_empty());
        let (restarted, _, _) = SyncRuntime::build(db.clone(), transport.clone());
        restarted.activate_account_context().await.unwrap();
        assert!(!restarted.global_sync_enabled().await.unwrap());
        restarted.sync_all().await.unwrap();
        assert!(transport.pushes.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn download_keeps_new_default_or_explicit_global_pause() {
    for paused in [false, true] {
        let db = database().await;
        let transport = Arc::new(MockTransport::new());
        transport.cursor.store(1, Ordering::SeqCst);
        transport.snapshots.lock().unwrap().push_back(SnapshotPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-1".into(),
            at_cursor: 1,
            current_cursor: 1,
            next_page_token: None,
            items: vec![SnapshotItem {
                entity_type: "workspace".into(),
                entity_id: "workspace-remote".into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: 1,
                payload: workspace_payload("Downloaded"),
            }],
        });
        let (service, _, _) = SyncRuntime::build(db, transport.clone());
        if paused {
            service.set_global_sync_enabled(false).await.unwrap();
        }
        let id = service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await
            .unwrap();
        assert_eq!(service.global_sync_enabled().await.unwrap(), !paused);
        let binding = service.status(&id).await.unwrap().binding.unwrap();
        assert!(binding.sync_enabled);
        assert_eq!(binding.state, "active");
        service.sync_all().await.unwrap();
        assert_eq!(transport.changes_calls.load(Ordering::SeqCst) > 0, !paused);
    }
}

#[tokio::test]
async fn explicit_workspace_pause_survives_global_resume_and_relogin() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&id).await.unwrap();
    service.disable(&id).await.unwrap();
    let calls = transport.changes_calls.load(Ordering::SeqCst);
    service.set_global_sync_enabled(false).await.unwrap();
    service.pause_current_account().await.unwrap();
    service.activate_account_context().await.unwrap();
    service.set_global_sync_enabled(true).await.unwrap();
    service.sync_all().await.unwrap();
    let binding = service.status(&id).await.unwrap().binding.unwrap();
    assert!(!binding.sync_enabled);
    assert_eq!(binding.state, "paused");
    assert_eq!(transport.changes_calls.load(Ordering::SeqCst), calls);
}
