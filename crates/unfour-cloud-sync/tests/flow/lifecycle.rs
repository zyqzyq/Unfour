use super::support::{database, CommandBus, CommandBusExtensions, MockTransport};
use super::*;
use std::sync::{atomic::Ordering, Arc};
use unfour_cloud_sync::{ChangesPage, SyncEntityType, SyncRuntime, SyncService, PROTOCOL_VERSION};
use unfour_flow_engine::FlowService;

pub(super) async fn fixture() -> (
    unfour_local_storage::LocalDb,
    CommandBus,
    SyncService,
    Arc<MockTransport>,
    String,
) {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let ws = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    (db, bus, service, transport, ws)
}

pub(super) async fn queue(
    service: &SyncService,
    transport: &MockTransport,
    ws: &str,
    mut change: RemoteChange,
) {
    let binding = service.status(ws).await.unwrap().binding.unwrap();
    change.cursor = binding.last_pulled_cursor + 1;
    transport
        .cursor
        .store(change.cursor as u64, Ordering::SeqCst);
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
        current_cursor: change.cursor,
        next_cursor: change.cursor,
        changes: vec![change],
    });
}

async fn outbox_count(db: &unfour_local_storage::LocalDb) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM cloud_sync_outbox WHERE entity_type='flow' AND status='pending'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn initial_snapshot_offline_incremental_and_tombstone_use_existing_worker() {
    let (db, bus, service, transport, ws) = fixture().await;
    let flow = bus.save_flow(definition(&ws)).await.unwrap();
    service.enable(&ws).await.unwrap();
    let pushed = transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|p| p.operations.clone())
        .find(|p| p.entity_type == SyncEntityType::Flow)
        .unwrap();
    assert_eq!(pushed.entity_id, flow.id);
    assert_eq!(pushed.parent_entity_id.as_deref(), Some(ws.as_str()));
    assert_eq!(pushed.payload.as_ref().unwrap()["name"], "Deploy");
    assert_eq!(outbox_count(&db).await, 0);
    service.set_global_sync_enabled(false).await.unwrap();
    service.pause_current_account().await.unwrap();
    let mut edited = flow.clone();
    edited.name = "Offline".into();
    let edited = bus.save_flow(edited).await.unwrap();
    assert_eq!(outbox_count(&db).await, 1);
    let captured: String = sqlx::query_scalar(
        "SELECT canonical_payload_json FROM cloud_sync_outbox WHERE entity_type='flow'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&captured).unwrap()["name"],
        "Offline"
    );
    assert!(bus.save_flow(flow).await.is_err());
    service.activate_account_context().await.unwrap();
    service
        .repository()
        .set_global_sync_enabled("account-a", true, chrono::Utc::now())
        .await
        .unwrap();
    service.enable(&ws).await.unwrap();
    service.sync_workspace(&ws).await.unwrap();
    assert_eq!(outbox_count(&db).await, 0);
    bus.delete_flow(ws.clone(), edited.id.clone(), edited.revision)
        .await
        .unwrap();
    service.sync_workspace(&ws).await.unwrap();
    let pushes = transport.pushes.lock().unwrap();
    let delete = pushes
        .iter()
        .flat_map(|p| &p.operations)
        .filter(|p| p.entity_type == SyncEntityType::Flow)
        .last()
        .unwrap();
    assert_eq!(delete.operation, SyncOperation::Delete);
    assert!(delete.payload.is_none());
}

#[tokio::test]
async fn remote_updates_delete_replay_and_history_are_isolated() {
    let (db, bus, service, transport, ws) = fixture().await;
    let local = bus.save_flow(definition(&ws)).await.unwrap();
    // Sentinel proves external definition writes do not copy, rewrite or delete runs.
    sqlx::query("INSERT INTO flow_runs (id,workspace_id,flow_id,status,run_json,started_at,updated_at) VALUES ('history',?,?,'succeeded','local-runtime-secret','now','now')")
        .bind(&ws).bind(&local.id).execute(db.pool()).await.unwrap();
    service.enable(&ws).await.unwrap();
    let mut remote = payload();
    remote["name"] = json!("Remote");
    let update = change(&ws, &local.id, Some(remote), 30);
    queue(&service, &transport, &ws, update.clone()).await;
    service.sync_workspace(&ws).await.unwrap();
    let updated = bus.get_flow(ws.clone(), local.id.clone()).await.unwrap();
    assert_eq!(updated.revision, local.revision + 1);
    assert_eq!(updated.name, "Remote");
    assert!(bus.save_flow(local.clone()).await.is_err());
    queue(&service, &transport, &ws, update).await;
    service.sync_workspace(&ws).await.unwrap();
    assert_eq!(
        bus.get_flow(ws.clone(), local.id.clone())
            .await
            .unwrap()
            .revision,
        updated.revision
    );
    for _ in 0..2 {
        queue(&service, &transport, &ws, change(&ws, &local.id, None, 31)).await;
        service.sync_workspace(&ws).await.unwrap();
    }
    assert!(bus.get_flow(ws.clone(), local.id.clone()).await.is_err());
    let rev: i64 = sqlx::query_scalar("SELECT revision FROM flow_definitions WHERE id=?")
        .bind(&local.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(rev, updated.revision + 1);
    assert_eq!(outbox_count(&db).await, 0);
    let history: String = sqlx::query_scalar("SELECT run_json FROM flow_runs WHERE id='history'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(history, "local-runtime-secret");
    let pushed_json = serde_json::to_string(&*transport.pushes.lock().unwrap()).unwrap();
    assert!(!pushed_json.contains("local-runtime-secret"));
    // Stale remote upsert cannot resurrect a newer tombstone.
    queue(
        &service,
        &transport,
        &ws,
        change(&ws, &local.id, Some(payload()), 30),
    )
    .await;
    service.sync_workspace(&ws).await.unwrap();
    assert!(bus.get_flow(ws.clone(), local.id.clone()).await.is_err());
    queue(
        &service,
        &transport,
        &ws,
        change(&ws, &local.id, Some(payload()), 32),
    )
    .await;
    service.sync_workspace(&ws).await.unwrap();
    assert_eq!(bus.get_flow(ws, local.id).await.unwrap().revision, rev + 1);
}

#[tokio::test]
async fn conflicts_keep_local_and_use_remote_replace_whole_documents() {
    for keep_local in [true, false] {
        for remote_delete in [true, false] {
            let (db, bus, service, transport, ws) = fixture().await;
            let local = bus.save_flow(definition(&ws)).await.unwrap();
            service.enable(&ws).await.unwrap();
            let mut edited = local.clone();
            edited.name = "Local edit".into();
            let edited = bus.save_flow(edited).await.unwrap();
            let mut remote = payload();
            remote["name"] = json!("Remote edit");
            remote["steps"][0]["durationMs"] = json!(20);
            queue(
                &service,
                &transport,
                &ws,
                change(&ws, &local.id, (!remote_delete).then_some(remote), 50),
            )
            .await;
            assert_eq!(
                service.sync_workspace(&ws).await.unwrap_err(),
                SyncError::Conflict
            );
            assert_eq!(
                bus.get_flow(ws.clone(), local.id.clone())
                    .await
                    .unwrap()
                    .name,
                "Local edit"
            );
            let views = service.conflicts(&ws).await.unwrap();
            assert_eq!(views.len(), 1);
            assert_eq!(
                views[0].local_payload.as_ref().unwrap()["name"],
                "Local edit"
            );
            if keep_local {
                service
                    .keep_local(&ws, SyncEntityType::Flow, &local.id)
                    .await
                    .unwrap();
                let value = bus.get_flow(ws.clone(), local.id.clone()).await.unwrap();
                assert_eq!(value.name, "Local edit");
                assert_eq!(value.revision, edited.revision);
                let operations = transport
                    .pushes
                    .lock()
                    .unwrap()
                    .iter()
                    .flat_map(|p| p.operations.clone())
                    .collect::<Vec<_>>();
                let last = operations
                    .iter()
                    .filter(|p| p.entity_type == SyncEntityType::Flow)
                    .last()
                    .unwrap();
                assert_eq!(last.base_version, 50);
                assert_eq!(last.payload.as_ref().unwrap()["steps"][0]["durationMs"], 0);
            } else {
                service
                    .use_remote(&ws, SyncEntityType::Flow, &local.id)
                    .await
                    .unwrap();
                let value = bus.get_flow(ws.clone(), local.id.clone()).await;
                if remote_delete {
                    assert!(value.is_err());
                } else {
                    let value = value.unwrap();
                    assert_eq!(value.name, "Remote edit");
                    assert_eq!(value.revision, edited.revision + 1);
                    assert_eq!(
                        serde_json::to_value(value).unwrap()["steps"][0]["durationMs"],
                        20
                    );
                }
            }
            assert!(service.conflicts(&ws).await.unwrap().is_empty());
            assert_eq!(outbox_count(&db).await, 0);
        }
    }
}

#[tokio::test]
async fn references_arrive_first_without_creating_resources_or_bypassing_validation() {
    let (db, bus, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    for capability in ["api", "ssh", "database"] {
        let id = uuid::Uuid::new_v4().to_string();
        let mut value = payload();
        value["inputs"] = json!([]);
        value["steps"] = json!([{"id":"call","name":"Call","timeoutMs":1000,"kind":"action",
            "action":{"capability":capability,"resourceId":RESOURCE_ID,"connectionId":RESOURCE_ID,"arguments":{}}}]);
        queue(&service, &transport, &ws, change(&ws, &id, Some(value), 1)).await;
        service.sync_workspace(&ws).await.unwrap();
        let flow = bus.get_flow(ws.clone(), id.clone()).await.unwrap();
        assert_eq!(
            serde_json::to_value(flow).unwrap()["steps"][0]["action"]["resourceId"],
            RESOURCE_ID
        );
        let result = bus
            .run_flow(unfour_core::models::FlowRunInput {
                workspace_id: ws.clone(),
                flow_id: id,
                environment_id: None,
                inputs: json!({}),
                secret_input_names: vec![],
                initiator: unfour_core::models::FlowInitiator::Human,
                confirm_effects: true,
            })
            .await;
        assert!(
            result.is_err()
                || result
                    .is_ok_and(|run| run.status != unfour_core::models::FlowRunStatus::Succeeded)
        );
    }
    for table in ["api_requests", "connections", "ssh_task"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE id=?"))
            .bind(RESOURCE_ID)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    assert_eq!(outbox_count(&db).await, 0);
}

#[tokio::test]
async fn direct_external_apply_is_idempotent_scoped_and_transactional() {
    let (db, bus, _, _, ws) = fixture().await;
    let remote = parse_change(&ws, &change(&ws, FLOW_ID, Some(payload()), 1)).unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert!(FlowService::apply_external_definition_on(
        &mut tx,
        &CommandContext::local("test"),
        remote.clone()
    )
    .await
    .is_err());
    let applied = FlowService::apply_external_definition_on(
        &mut tx,
        &CommandContext::external("test"),
        remote.clone(),
    )
    .await
    .unwrap();
    assert_eq!(applied.value, ExternalMaterialization::Applied(1));
    assert!(applied
        .mutations
        .iter()
        .all(|m| m.origin == MutationOrigin::External));
    let repeated = FlowService::apply_external_definition_on(
        &mut tx,
        &CommandContext::external("test"),
        remote,
    )
    .await
    .unwrap();
    assert_eq!(repeated.value, ExternalMaterialization::AlreadyEquivalent);
    tx.commit().await.unwrap();
    let other = bus.create_workspace("Other".into()).await.unwrap();
    for remote in [
        change(&other.id, FLOW_ID, Some(payload()), 1),
        change(&other.id, FLOW_ID, None, 2),
    ] {
        let mut tx = db.pool().begin().await.unwrap();
        assert!(FlowService::apply_external_definition_on(
            &mut tx,
            &CommandContext::external("test"),
            parse_change(&other.id, &remote).unwrap()
        )
        .await
        .is_err());
        tx.rollback().await.unwrap();
    }
    let mut tx = db.pool().begin().await.unwrap();
    FlowService::apply_external_definition_on(
        &mut tx,
        &CommandContext::external("test"),
        parse_change(&ws, &change(&ws, FLOW_ID, None, 2)).unwrap(),
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(bus.get_flow(ws, FLOW_ID.into()).await.unwrap().revision, 1);
}
