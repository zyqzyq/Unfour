use super::lifecycle::{fixture, queue};
use super::support::{database, workspace_payload, CommandBus, MockTransport};
use super::*;
use crate::{
    DownloadDecision, SnapshotItem, SnapshotPage, SyncEntityType, SyncRuntime, PROTOCOL_VERSION,
};
use std::sync::{atomic::Ordering, Arc};

#[tokio::test]
async fn snapshot_download_installs_flow_after_workspace_without_history() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(2, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 2,
        current_cursor: 2,
        // Deliberately send Flow before its workspace in the same snapshot.
        items: vec![
            SnapshotItem {
                entity_type: WIRE_NAME.into(),
                entity_id: FLOW_ID.into(),
                parent_entity_id: Some("workspace-remote".into()),
                server_version: 4,
                payload_schema_version: 1,
                payload: payload(),
            },
            SnapshotItem {
                entity_type: "workspace".into(),
                entity_id: "workspace-remote".into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: 1,
                payload: workspace_payload("Downloaded"),
            },
        ],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service
        .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
        .await
        .unwrap();
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let flow = bus
        .get_flow("workspace-remote".into(), FLOW_ID.into())
        .await
        .unwrap();
    assert_eq!(flow.revision, 1); // never the serverVersion 4
    assert_eq!(flow.name, "Deploy");
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM flow_runs), (SELECT count(*) FROM cloud_sync_outbox)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
    assert!(transport.pushes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn workspace_delete_scopes_flow_conflicts_and_cascades_history() {
    let (db, bus, service, transport, ws) = fixture().await;
    let flow = bus.save_flow(definition(&ws)).await.unwrap();
    sqlx::query("INSERT INTO flow_runs (id,workspace_id,flow_id,status,run_json,started_at,updated_at) VALUES ('history',?,?,'succeeded','local','now','now')")
        .bind(&ws).bind(&flow.id).execute(db.pool()).await.unwrap();
    service.enable(&ws).await.unwrap();
    let mut edited = flow.clone();
    edited.name = "Unsynced".into();
    let edited = bus.save_flow(edited).await.unwrap();
    let mut deletion = change(&ws, &ws, None, 50);
    deletion.entity_type = "workspace".into();
    deletion.parent_entity_id = None;
    queue(&service, &transport, &ws, deletion).await;
    assert_eq!(
        service.sync_workspace(&ws).await.unwrap_err(),
        SyncError::Conflict
    );
    assert_eq!(
        bus.get_flow(ws.clone(), flow.id.clone())
            .await
            .unwrap()
            .revision,
        edited.revision
    );
    service
        .use_remote(&ws, SyncEntityType::Workspace, &ws)
        .await
        .unwrap();
    let row: (i64, bool) =
        sqlx::query_as("SELECT revision, deleted_at IS NOT NULL FROM flow_definitions WHERE id=?")
            .bind(&flow.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(row, (edited.revision + 1, true));
    let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM flow_runs), (SELECT count(*) FROM cloud_sync_outbox WHERE entity_type='flow')")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (0, 0));
}

#[tokio::test]
async fn local_workspace_delete_captures_flow_tombstone_offline() {
    let (db, bus, service, _, ws) = fixture().await;
    let flow = bus.save_flow(definition(&ws)).await.unwrap();
    service.enable(&ws).await.unwrap();
    service.set_global_sync_enabled(false).await.unwrap();
    bus.create_workspace("Remaining".into()).await.unwrap();
    bus.delete_workspace(ws.clone()).await.unwrap();
    let row: (String, Option<String>, i64) = sqlx::query_as("SELECT operation,canonical_payload_json,content_revision FROM cloud_sync_outbox WHERE entity_type='flow' AND entity_id=?")
        .bind(&flow.id).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row, ("delete".into(), None, flow.revision + 1));
}

#[tokio::test]
async fn oversized_initial_upload_rolls_back_binding_and_incremental_edit_is_atomic() {
    let (db, bus, service, _, ws) = fixture().await;
    let mut big = definition(&ws);
    big.inputs[0].description = Some("x".repeat(MAX_PAYLOAD_BYTES));
    let saved = bus.save_flow(big).await.unwrap(); // local 1 MiB policy unchanged
    assert_eq!(
        service.enable(&ws).await.unwrap_err(),
        SyncError::FlowPayloadTooLarge
    );
    let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM cloud_sync_workspace_bindings), (SELECT count(*) FROM cloud_sync_outbox)")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (0, 0));
    let mut small = saved;
    small.inputs[0].description = None;
    let small = bus.save_flow(small).await.unwrap();
    service.enable(&ws).await.unwrap();
    let mut big = small.clone();
    big.inputs[0].description = Some("x".repeat(MAX_PAYLOAD_BYTES));
    assert!(bus
        .save_flow(big)
        .await
        .unwrap_err()
        .to_string()
        .contains("cloud_sync_flow_payload_too_large"));
    assert_eq!(
        bus.get_flow(ws, small.id).await.unwrap().revision,
        small.revision
    );
}

#[tokio::test]
async fn future_schema_is_retained_without_applying_or_dead_lettering() {
    let (db, _, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    let mut future = change(&ws, FLOW_ID, Some(payload()), 1);
    future.payload_schema_version = 2;
    queue(&service, &transport, &ws, future).await;
    assert_eq!(
        service.sync_workspace(&ws).await.unwrap_err(),
        SyncError::CompatibilityWaiting
    );
    let counts: (i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM flow_definitions), (SELECT count(*) FROM cloud_sync_remote_entity WHERE entity_type='flow' AND compatibility_state='deferred_compatibility'), (SELECT count(*) FROM cloud_sync_outbox WHERE status='dead')")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (0, 1, 0));
}
