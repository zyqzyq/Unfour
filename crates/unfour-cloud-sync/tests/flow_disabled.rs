//! Compiles the production library (without cfg(test)): shipping gates matter
//! independently from the unit-build registration rehearsal.
#[allow(dead_code, unused_imports)]
#[path = "worker/support/mod.rs"]
mod support;
use support::*;
use unfour_core::domain::DomainEntityType;

#[test]
fn production_registry_and_wire_type_exclude_flow() {
    assert!(SyncEntityType::parse("flow").is_err());
    assert!(SyncEntityType::try_from(DomainEntityType::FlowDefinition).is_err());
    assert!(unfour_cloud_sync::SYNC_ENTITY_REGISTRY
        .iter()
        .all(|entry| entry.wire_name != "flow"));
    assert!(serde_json::from_str::<SyncEntityType>("\"flow\"").is_err());
}

#[tokio::test]
async fn production_local_flow_keeps_one_mib_and_literal_semantics_when_bound() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let ws = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let flow = serde_json::from_value(serde_json::json!({
        "id":"", "workspaceId":ws, "name":"Local", "revision":0,
        "inputs":[{"name":"label","type":"string","default":"unchanged literal", "description":"x".repeat(70_000)}],
        "steps":[{"id":"wait","name":"Wait","kind":"wait","durationMs":0,"timeoutMs":1000}]
    })).unwrap();
    let flow = bus.save_flow(flow).await.unwrap();
    service.enable(&ws).await.unwrap();
    let edited = bus.save_flow(flow).await.unwrap();
    assert_eq!(
        edited.inputs[0].default,
        Some(serde_json::json!("unchanged literal"))
    );
    assert_eq!(edited.inputs[0].description.as_ref().unwrap().len(), 70_000);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM cloud_sync_outbox WHERE entity_type='flow'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert!(transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|p| &p.operations)
        .all(|p| p.entity_type.as_str() != "flow"));
}

#[tokio::test]
async fn production_pull_retains_flow_without_business_apply_or_push() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let ws = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&ws).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let cursor = service
        .status(&ws)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor
        + 1;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor,
        next_cursor: cursor,
        changes: vec![RemoteChange {
            cursor,
            operation_id: "flow-from-future".into(),
            entity_type: "flow".into(),
            entity_id: "00000000-0000-4000-8000-000000000001".into(),
            parent_entity_id: Some(ws.clone()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: 1,
            payload: Some(serde_json::json!({"name":"Future flow", "inputs":[], "steps":[]})),
            deleted_at: None,
        }],
    });
    assert_eq!(
        service.sync_workspace(&ws).await.unwrap_err(),
        SyncError::CompatibilityWaiting
    );
    assert_eq!(
        service
            .status(&ws)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        cursor
    );
    let counts:(i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM flow_definitions), (SELECT count(*) FROM cloud_sync_remote_entity WHERE entity_type='flow' AND compatibility_state='deferred_compatibility'), (SELECT count(*) FROM cloud_sync_outbox WHERE entity_type='flow')")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (0, 1, 0));
    assert!(transport.pushes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn production_snapshot_retains_flow_without_materialization() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(2, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 2,
        current_cursor: 2,
        items: vec![
            SnapshotItem {
                entity_type: "workspace".into(),
                entity_id: "workspace-remote".into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: 1,
                payload: workspace_payload("Remote"),
            },
            SnapshotItem {
                entity_type: "flow".into(),
                entity_id: "00000000-0000-4000-8000-000000000001".into(),
                parent_entity_id: Some("workspace-remote".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({"name":"Future flow","inputs":[],"steps":[]}),
            },
        ],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    service
        .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
        .await
        .unwrap();
    let counts:(i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM flow_definitions), (SELECT count(*) FROM cloud_sync_remote_entity WHERE entity_type='flow')")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (0, 1));
    assert_eq!(
        service
            .status("workspace-remote")
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_error
            .as_deref(),
        Some("cloud_sync_compatibility_waiting")
    );
}
