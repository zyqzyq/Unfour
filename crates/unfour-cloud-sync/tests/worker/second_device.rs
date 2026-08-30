//! Two independent local stores connected by captured production wire payloads.
//! This verifies client compatibility, not the hosted server's reconciliation.
use super::support::*;

#[tokio::test]
async fn pushed_data_bootstraps_second_device_then_tombstone_survives_restart_without_echo() {
    let first_db = database().await;
    let first_transport = Arc::new(MockTransport::new());
    *first_transport.created_workspace_id.lock().unwrap() = Some("cloud-1".into());
    let (first, hook, _) = SyncRuntime::build(first_db.clone(), first_transport.clone());
    let first_bus = CommandBus::from_db_with_extensions(
        first_db.clone(),
        CommandBusExtensions::new(vec![hook]),
    )
    .await
    .unwrap();
    let workspace = first_bus
        .create_workspace("Relay workspace".into())
        .await
        .unwrap();
    let variables = first_bus
        .workspace_variable_create(
            workspace.id.clone(),
            variable(None, "SHARED", "first-device-value", false),
        )
        .await
        .unwrap();
    let variable_id = variables.id;
    first_bus
        .workspace_variable_create(
            workspace.id.clone(),
            variable(None, "TOKEN", "second-device-secret-canary", true),
        )
        .await
        .unwrap();
    first.enable(&workspace.id).await.unwrap();
    let operations = first_transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|request| request.operations.clone())
        .collect::<Vec<_>>();
    assert!(!serde_json::to_string(&operations)
        .unwrap()
        .contains("second-device-secret-canary"));
    let cursor = first_transport.cursor.load(Ordering::SeqCst) as i64;

    let second_db = database().await;
    let second_transport = Arc::new(MockTransport::new());
    *second_transport.roots.lock().unwrap() = vec![workspace.id.clone()];
    second_transport
        .cursor
        .store(cursor as u64, Ordering::SeqCst);
    second_transport
        .snapshots
        .lock()
        .unwrap()
        .push_back(SnapshotPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-1".into(),
            at_cursor: cursor,
            current_cursor: cursor,
            items: operations
                .iter()
                .map(|operation| SnapshotItem {
                    entity_type: operation.entity_type,
                    entity_id: operation.entity_id.clone(),
                    parent_entity_id: operation.parent_entity_id.clone(),
                    server_version: operation.base_version + 1,
                    payload_schema_version: operation.payload_schema_version,
                    payload: operation.payload.clone().unwrap(),
                })
                .collect(),
            next_page_token: None,
        });
    let (second, hook, _) = SyncRuntime::build(second_db.clone(), second_transport.clone());
    let second_bus = CommandBus::from_db_with_extensions(
        second_db.clone(),
        CommandBusExtensions::new(vec![hook]),
    )
    .await
    .unwrap();
    assert_eq!(
        second
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await
            .unwrap(),
        workspace.id
    );
    let downloaded = second_bus
        .workspace_variables_list(workspace.id.clone())
        .await
        .unwrap();
    assert!(downloaded
        .iter()
        .any(|item| item.id == variable_id && item.value == "first-device-value"));
    assert!(!serde_json::to_string(&downloaded)
        .unwrap()
        .contains("second-device-secret-canary"));

    first_transport.pushes.lock().unwrap().clear();
    first_bus
        .workspace_variable_delete(workspace.id.clone(), variable_id.clone())
        .await
        .unwrap();
    first.sync_workspace(&workspace.id).await.unwrap();
    let tombstone = first_transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|request| request.operations.iter())
        .find(|operation| operation.entity_id == variable_id)
        .unwrap()
        .clone();
    assert_eq!(tombstone.operation, SyncOperation::Delete);
    second_transport
        .changes
        .lock()
        .unwrap()
        .push_back(ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-1".into(),
            current_cursor: cursor + 1,
            next_cursor: cursor + 1,
            changes: vec![RemoteChange {
                cursor: cursor + 1,
                operation_id: tombstone.operation_id,
                entity_type: tombstone.entity_type,
                entity_id: tombstone.entity_id,
                parent_entity_id: tombstone.parent_entity_id,
                operation: tombstone.operation,
                server_version: tombstone.base_version + 1,
                payload_schema_version: tombstone.payload_schema_version,
                payload: tombstone.payload,
                deleted_at: Some("2026-08-30T00:00:00Z".into()),
            }],
        });
    second.sync_workspace(&workspace.id).await.unwrap();
    let (restarted, _, _) = SyncRuntime::build(second_db.clone(), second_transport.clone());
    restarted.sync_workspace(&workspace.id).await.unwrap();
    assert!(!second_bus
        .workspace_variables_list(workspace.id.clone())
        .await
        .unwrap()
        .iter()
        .any(|item| item.id == variable_id));
    let status = restarted.status(&workspace.id).await.unwrap();
    assert_eq!(status.binding.unwrap().last_pulled_cursor, cursor + 1);
    assert!(
        second_transport.pushes.lock().unwrap().is_empty(),
        "remote snapshot/tombstone must not echo"
    );
    let state: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM cloud_sync_outbox), (SELECT COUNT(*) FROM cloud_sync_snapshot_staging)",
    ).fetch_one(second_db.pool()).await.unwrap();
    assert_eq!(state, (0, 0));
}

#[tokio::test]
async fn account_switch_during_snapshot_discards_old_account_data_and_staging() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let before = bus.list_workspaces().await.unwrap();
    let transport = Arc::new(MockTransport::new());
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        next_page_token: None,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Workspace,
            entity_id: "workspace-remote".into(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: 1,
            payload: workspace_payload("Private account A workspace"),
        }],
    });
    let gate = Arc::new(Barrier::new(2));
    *transport.snapshot_barrier.lock().unwrap() = Some(gate.clone());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    let download = tokio::spawn(async move {
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), gate.wait())
        .await
        .unwrap();
    transport.switch_account("account-b");
    gate.wait().await;
    assert!(matches!(
        download.await.unwrap(),
        Err(SyncError::AccountChanged)
    ));
    let after = bus.list_workspaces().await.unwrap();
    assert_eq!(after.active_workspace_id, before.active_workspace_id);
    assert_eq!(after.workspaces.len(), before.workspaces.len());
    let state: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM cloud_sync_workspace_bindings), (SELECT COUNT(*) FROM cloud_sync_outbox), (SELECT COUNT(*) FROM cloud_sync_snapshot_staging)",
    ).fetch_one(db.pool()).await.unwrap();
    assert_eq!(state, (0, 0, 0));
}
