//! Atomic boundaries spanning outbox heads, attempt results, remote apply and staging.

use super::support::*;
use unfour_cloud_sync::{PushResult, PushResultStatus, SyncService};

async fn active_runtime() -> (LocalDb, CommandBus, SyncService, Arc<MockTransport>, String) {
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
    (db, bus, service, transport, workspace_id)
}

#[tokio::test]
async fn old_push_success_preserves_a_new_local_head_and_does_not_acknowledge_pull_cursor() {
    let (db, bus, service, _, workspace_id) = active_runtime().await;
    let local = bus
        .workspace_variable_create(workspace_id.clone(), variable(None, "EDIT", "first", false))
        .await
        .unwrap();
    let repository = service.repository();
    let binding = repository
        .binding("account-a", &workspace_id)
        .await
        .unwrap()
        .unwrap();
    let entries = repository
        .due_outbox("account-a", &binding.cloud_workspace_id, Utc::now(), 50)
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    repository
        .mark_in_flight(&entries, "old-worker", Utc::now())
        .await
        .unwrap();
    bus.workspace_variable_update(
        workspace_id.clone(),
        local.id.clone(),
        variable(Some(local.id.clone()), "EDIT", "second", false),
    )
    .await
    .unwrap();
    let newer: (String, String) = sqlx::query_as(
        "SELECT operation_id, canonical_payload_json FROM cloud_sync_outbox WHERE entity_id = ?1",
    )
    .bind(&local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_ne!(newer.0, entries[0].operation_id);

    repository
        .apply_push_results(
            &binding,
            &entries,
            &[PushResult {
                operation_id: entries[0].operation_id.clone(),
                server_version: 7,
                cursor: 99,
                status: PushResultStatus::Applied,
            }],
            &unfour_cloud_sync::SystemClock,
        )
        .await
        .unwrap();

    let head: (String, String, String, i64) = sqlx::query_as(
        "SELECT operation_id, canonical_payload_json, status, base_version FROM cloud_sync_outbox WHERE entity_id = ?1",
    ).bind(&local.id).fetch_one(db.pool()).await.unwrap();
    assert_eq!(head, (newer.0, newer.1, "pending".into(), 7));
    let attempt: (String, i64, i64) = sqlx::query_as(
        "SELECT status, result_server_version, result_cursor FROM cloud_sync_attempts WHERE operation_id = ?1",
    ).bind(&entries[0].operation_id).fetch_one(db.pool()).await.unwrap();
    assert_eq!(attempt, ("applied".into(), 7, 99));
    assert_eq!(
        repository
            .binding("account-a", &workspace_id)
            .await
            .unwrap()
            .unwrap()
            .last_pulled_cursor,
        binding.last_pulled_cursor
    );
}

#[tokio::test]
async fn malformed_push_results_roll_back_earlier_acknowledgements_and_checkpoint() {
    let (db, bus, service, _, workspace_id) = active_runtime().await;
    for key in ["FIRST", "SECOND"] {
        bus.workspace_variable_create(workspace_id.clone(), variable(None, key, "value", false))
            .await
            .unwrap();
    }
    let repository = service.repository();
    let binding = repository
        .binding("account-a", &workspace_id)
        .await
        .unwrap()
        .unwrap();
    let entries = repository
        .due_outbox("account-a", &binding.cloud_workspace_id, Utc::now(), 50)
        .await
        .unwrap();
    assert_eq!(entries.len(), 2);
    repository
        .mark_in_flight(&entries, "batch-worker", Utc::now())
        .await
        .unwrap();
    let result = repository
        .apply_push_results(
            &binding,
            &entries,
            &[
                PushResult {
                    operation_id: entries[0].operation_id.clone(),
                    server_version: 5,
                    cursor: 10,
                    status: PushResultStatus::Applied,
                },
                PushResult {
                    operation_id: "unknown-operation".into(),
                    server_version: 6,
                    cursor: 11,
                    status: PushResultStatus::NoOp,
                },
            ],
            &unfour_cloud_sync::SystemClock,
        )
        .await;
    assert_eq!(result, Err(SyncError::InvalidData));

    for entry in &entries {
        let state: (String, String, Option<i64>, i64) = sqlx::query_as(
            r#"SELECT outbox.status, attempt.status, attempt.result_server_version,
                      (SELECT COUNT(*) FROM cloud_sync_entity_state WHERE entity_id = outbox.entity_id)
               FROM cloud_sync_outbox AS outbox JOIN cloud_sync_attempts AS attempt
                 ON attempt.operation_id = outbox.operation_id
               WHERE outbox.operation_id = ?1"#,
        ).bind(&entry.operation_id).fetch_one(db.pool()).await.unwrap();
        assert_eq!(state, ("in_flight".into(), "in_flight".into(), None, 0));
    }
    let after = repository
        .binding("account-a", &workspace_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.initial_confirmed, binding.initial_confirmed);
    assert_eq!(
        after.initialization_checkpoint,
        binding.initialization_checkpoint
    );
    assert_eq!(after.last_pulled_cursor, binding.last_pulled_cursor);
}

#[tokio::test]
async fn failed_core_pull_rolls_back_own_ack_and_cross_domain_writes_then_retries() {
    let (db, bus, service, transport, workspace_id) = active_runtime().await;
    let local = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "LOCAL", "preserved", false),
        )
        .await
        .unwrap();
    let binding = service
        .repository()
        .binding("account-a", &workspace_id)
        .await
        .unwrap()
        .unwrap();
    let entries = service
        .repository()
        .due_outbox("account-a", &binding.cloud_workspace_id, Utc::now(), 50)
        .await
        .unwrap();
    service
        .repository()
        .mark_in_flight(&entries, "unconfirmed-worker", Utc::now())
        .await
        .unwrap();
    let base = binding.last_pulled_cursor;
    let page = ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
        current_cursor: base + 3,
        next_cursor: base + 3,
        changes: vec![
            remote_variable_change(
                &workspace_id,
                base + 1,
                &entries[0].operation_id,
                &local.id,
                "LOCAL",
                "preserved",
            ),
            remote_variable_change(
                &workspace_id,
                base + 2,
                "remote-variable-op",
                "remote-variable",
                "REMOTE",
                "remote",
            ),
            RemoteChange {
                cursor: base + 3,
                operation_id: "remote-collection-op".into(),
                entity_type: SyncEntityType::ApiCollection,
                entity_id: "remote-collection".into(),
                parent_entity_id: Some(workspace_id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(serde_json::json!({
                    "name": " ", "description": null,
                    "createdAt": "2026-08-21T00:00:00Z", "updatedAt": "2026-08-21T00:00:00Z"
                })),
                deleted_at: None,
            },
        ],
    };
    transport.changes.lock().unwrap().push_back(page.clone());
    assert_eq!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Core)
    );
    let rolled_back: (String, String, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT status FROM cloud_sync_outbox WHERE operation_id = ?1),
             (SELECT status FROM cloud_sync_attempts WHERE operation_id = ?1),
             (SELECT COUNT(*) FROM workspace_variables WHERE id = 'remote-variable'),
             (SELECT COUNT(*) FROM api_collections WHERE id = 'remote-collection'),
             (SELECT COUNT(*) FROM cloud_sync_entity_state
              WHERE entity_id IN (?2, 'remote-variable', 'remote-collection'))"#,
    )
    .bind(&entries[0].operation_id)
    .bind(&local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        rolled_back,
        ("in_flight".into(), "in_flight".into(), 0, 0, 0)
    );
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        base
    );

    let mut repaired = page;
    repaired.changes[2].payload.as_mut().unwrap()["name"] = serde_json::json!("Remote collection");
    transport.changes.lock().unwrap().push_back(repaired);
    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    restarted.sync_workspace(&workspace_id).await.unwrap();
    let committed: (String, String, String, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT status FROM cloud_sync_attempts WHERE operation_id = ?1),
             (SELECT value FROM workspace_variables WHERE id = 'remote-variable'),
             (SELECT name FROM api_collections WHERE id = 'remote-collection'),
             (SELECT COUNT(*) FROM cloud_sync_outbox)"#,
    )
    .bind(&entries[0].operation_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        committed,
        (
            "applied".into(),
            "remote".into(),
            "Remote collection".into(),
            0
        )
    );
    assert_eq!(
        restarted
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        base + 3
    );
    assert!(
        transport.pushes.lock().unwrap().is_empty(),
        "remote apply must not echo"
    );
}

#[tokio::test]
async fn late_snapshot_failure_rolls_back_connections_and_cleans_staging_before_retry() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    let workspace_id = "workspace-remote";
    let item = |entity_type, entity_id: &str, parent: Option<&str>, payload| SnapshotItem {
        entity_type,
        entity_id: entity_id.into(),
        parent_entity_id: parent.map(str::to_string),
        server_version: 3,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload,
    };
    let first = SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 8,
        current_cursor: 8,
        next_page_token: Some("next".into()),
        items: vec![
            item(
                SyncEntityType::Workspace,
                workspace_id,
                None,
                workspace_payload("Atomic download"),
            ),
            item(
                SyncEntityType::Connection,
                "snapshot-ssh",
                None,
                serde_json::json!({
                    "id": "snapshot-ssh", "workspaceId": workspace_id, "connectionType": "ssh",
                    "name": "Remote SSH", "host": "remote.example.test", "port": 22,
                    "config": {"kind": "ssh", "username": "deploy", "authMethod": "private-key"},
                    "createdAt": "2026-08-21T00:00:00Z", "updatedAt": "2026-08-21T00:00:00Z"
                }),
            ),
        ],
    };
    let second = SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 8,
        current_cursor: 8,
        next_page_token: None,
        items: vec![item(
            SyncEntityType::ApiCollection,
            "snapshot-collection",
            Some(workspace_id),
            serde_json::json!({"name": " ", "description": null,
                "createdAt": "2026-08-21T00:00:00Z", "updatedAt": "2026-08-21T00:00:00Z"}),
        )],
    };
    transport
        .snapshots
        .lock()
        .unwrap()
        .extend([first.clone(), second.clone()]);
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert_eq!(
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await,
        Err(SyncError::Core)
    );
    let leftovers: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT (SELECT COUNT(*) FROM workspaces), (SELECT COUNT(*) FROM connections),
             (SELECT COUNT(*) FROM cloud_sync_workspace_bindings),
             (SELECT COUNT(*) FROM cloud_sync_entity_state),
             (SELECT COUNT(*) FROM cloud_sync_snapshot_staging)"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(leftovers, (0, 0, 0, 0, 0));

    let mut repaired = second;
    repaired.items[0].payload["name"] = serde_json::json!("Remote collection");
    transport
        .snapshots
        .lock()
        .unwrap()
        .extend([first, repaired]);
    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport);
    assert_eq!(
        restarted
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await
            .unwrap(),
        workspace_id
    );
    let committed: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT (SELECT last_pulled_cursor FROM cloud_sync_workspace_bindings),
             (SELECT COUNT(*) FROM connections WHERE id = 'snapshot-ssh' AND credential_ref IS NULL),
             (SELECT COUNT(*) FROM cloud_sync_entity_state WHERE server_version = 3),
             (SELECT COUNT(*) FROM cloud_sync_outbox),
             (SELECT COUNT(*) FROM cloud_sync_snapshot_staging)"#,
    ).fetch_one(db.pool()).await.unwrap();
    assert_eq!(committed, (8, 1, 3, 0, 0));
}
