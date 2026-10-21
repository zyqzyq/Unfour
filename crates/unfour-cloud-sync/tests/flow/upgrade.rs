use super::lifecycle::{fixture, queue};
use super::support::{CommandBus, CommandBusExtensions};
use super::*;
use std::sync::atomic::Ordering;
use unfour_cloud_sync::{sync_entity_descriptor, ParentDependency, SyncEntityType, SyncRuntime};

#[test]
fn production_registry_registers_flow_with_unchanged_protocol_and_schema() {
    let entity = SyncEntityType::parse("flow").unwrap();
    assert_eq!(entity, SyncEntityType::Flow);
    assert_eq!(
        SyncEntityType::try_from(DomainEntityType::FlowDefinition).unwrap(),
        entity
    );
    assert_eq!(
        DomainEntityType::from(entity),
        DomainEntityType::FlowDefinition
    );
    assert_eq!(
        serde_json::from_str::<SyncEntityType>("\"flow\"").unwrap(),
        entity
    );
    let descriptor = sync_entity_descriptor(entity);
    assert_eq!(descriptor.parent_dependency, ParentDependency::Workspace);
    assert_eq!(
        (
            descriptor.payload_schema_version,
            descriptor.reader_revision
        ),
        (1, 1)
    );
    assert!(descriptor.initial_upload && descriptor.supports_tombstone);
    assert_eq!(unfour_cloud_sync::PROTOCOL_VERSION, 5);
}

#[tokio::test]
async fn upgrade_replays_old_clients_retained_flow_and_tombstone_without_uploading() {
    for tombstone in [false, true] {
        let (db, bus, service, transport, ws) = fixture().await;
        service.enable(&ws).await.unwrap();
        let binding = service.status(&ws).await.unwrap().binding.unwrap();
        // Older clients knew neither Flow nor its entity state, but retained
        // the complete envelope and advanced their cursor in Protocol 5.
        let remote = change(&ws, FLOW_ID, (!tombstone).then(payload), 19);
        sqlx::query("INSERT INTO cloud_sync_remote_entity (account_id,cloud_workspace_id,entity_type,entity_id,parent_entity_id,server_version,payload_schema_version,operation,canonical_payload_json,deleted_at,operation_id,compatibility_state,created_at,updated_at) VALUES (?,?,'flow',?,?,19,1,?,?,?,?,'deferred_compatibility','now','now')")
            .bind(&binding.account_id).bind(&binding.cloud_workspace_id).bind(FLOW_ID).bind(&ws)
            .bind(remote.operation.as_str()).bind(remote.payload.as_ref().map(|p| serde_json::to_string(p).unwrap()))
            .bind(&remote.deleted_at).bind(&remote.operation_id).execute(db.pool()).await.unwrap();
        drop(service);
        transport.pushes.lock().unwrap().clear();
        let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
        for _ in 0..2 {
            service.sync_workspace(&ws).await.unwrap();
        }
        let current = bus.get_flow(ws.clone(), FLOW_ID.into()).await;
        if tombstone {
            assert!(current.is_err());
        } else {
            let current = current.unwrap();
            assert_eq!(current.name, "Deploy");
            assert_eq!(current.revision, 1);
        }
        let state: (i64, i64, String) = sqlx::query_as("SELECT server_version,applied_reader_revision,sync_status FROM cloud_sync_entity_state WHERE entity_type='flow' AND entity_id=?")
            .bind(FLOW_ID).fetch_one(db.pool()).await.unwrap();
        assert_eq!(state, (19, 1, "synced".into()));
        assert_eq!(
            service
                .status(&ws)
                .await
                .unwrap()
                .binding
                .unwrap()
                .last_pulled_cursor,
            binding.last_pulled_cursor
        );
        assert!(transport.pushes.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn uncertain_flow_retries_identical_intent_after_restart_and_then_is_quiet() {
    let (db, bus, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    let flow = bus.save_flow(definition(&ws)).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    transport.fail_pushes.store(1, Ordering::SeqCst);
    assert_eq!(
        service.sync_workspace(&ws).await.unwrap_err(),
        SyncError::Transport
    );
    assert_eq!(service.status(&ws).await.unwrap().uncertain_count, 1);
    let first = transport.pushes.lock().unwrap()[0].operations[0].clone();
    drop(service);
    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at=NULL WHERE entity_id=?")
        .bind(&flow.id)
        .execute(db.pool())
        .await
        .unwrap();
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.sync_workspace(&ws).await.unwrap();
    let retried = transport.pushes.lock().unwrap()[1].operations[0].clone();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(retried).unwrap()
    );
    let count = transport.pushes.lock().unwrap().len();
    service.sync_workspace(&ws).await.unwrap();
    assert_eq!(transport.pushes.lock().unwrap().len(), count);
    assert_eq!(service.status(&ws).await.unwrap().pending_count, 0);
    assert_eq!(
        bus.get_flow(ws, flow.id).await.unwrap().revision,
        flow.revision
    );
}

#[tokio::test]
async fn generic_external_page_reports_equivalent_flow_and_never_echoes_to_outbox() {
    let (db, _, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    let (_, hook, _) = SyncRuntime::build(db.clone(), transport);
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let page = ExternalApplyPage {
        flow_definitions: vec![unfour_cloud_sync::parse_remote_change(
            &ws,
            &change(&ws, FLOW_ID, Some(payload()), 99),
        )
        .unwrap()
        .flow_definitions
        .remove(0)],
        ..Default::default()
    };
    let first = bus.apply_external_page(page.clone()).await.unwrap();
    assert_eq!(first.applied_count, 1);
    assert_eq!(first.mutations[0].origin, MutationOrigin::External);
    assert_eq!(
        first.materialized_entities[0].entity_type,
        DomainEntityType::FlowDefinition
    );
    let duplicate = bus.apply_external_page(page).await.unwrap();
    assert_eq!(duplicate.applied_count, 0);
    assert_eq!(duplicate.materialized_entities.len(), 1);
    assert_eq!(bus.get_flow(ws, FLOW_ID.into()).await.unwrap().revision, 1);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cloud_sync_outbox WHERE entity_type='flow'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn future_flow_variant_waits_and_future_schema_tombstone_is_still_applied() {
    let (db, bus, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    let mut future = payload();
    future["steps"][0]["kind"] = json!("futureStep");
    queue(
        &service,
        &transport,
        &ws,
        change(&ws, FLOW_ID, Some(future), 10),
    )
    .await;
    assert_eq!(
        service.sync_workspace(&ws).await.unwrap_err(),
        SyncError::CompatibilityWaiting
    );
    assert!(bus.get_flow(ws.clone(), FLOW_ID.into()).await.is_err());
    let mut deleted = change(&ws, FLOW_ID, None, 11);
    deleted.payload_schema_version = 2;
    queue(&service, &transport, &ws, deleted).await;
    service.sync_workspace(&ws).await.unwrap();
    let state: (i64, String) = sqlx::query_as(
        "SELECT server_version,sync_status FROM cloud_sync_entity_state WHERE entity_type='flow'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, (11, "synced".into()));
}

#[tokio::test]
async fn upgrade_preserves_untracked_local_flow_before_remote_apply_or_workspace_delete() {
    for workspace_delete in [false, true] {
        let (db, bus, service, transport, ws) = fixture().await;
        service.enable(&ws).await.unwrap();
        let local = unfour_flow_engine::FlowService::new(db.clone())
            .save(definition(&ws))
            .await
            .unwrap();
        let mut remote = change(&ws, &local.id, Some(payload()), 29);
        remote.payload.as_mut().unwrap()["name"] = json!("Remote overwrite");
        if workspace_delete {
            remote = change(&ws, &ws, None, 29);
            remote.entity_type = "workspace".into();
            remote.parent_entity_id = None;
        }
        queue(&service, &transport, &ws, remote).await;
        assert_eq!(
            service.sync_workspace(&ws).await.unwrap_err(),
            SyncError::Conflict
        );
        assert_eq!(
            bus.get_flow(ws.clone(), local.id.clone())
                .await
                .unwrap()
                .name,
            local.name
        );
        let entity = if workspace_delete {
            SyncEntityType::Workspace
        } else {
            SyncEntityType::Flow
        };
        let id = if workspace_delete { &ws } else { &local.id };
        service.keep_local(&ws, entity, id).await.unwrap();
        assert_eq!(bus.get_flow(ws, local.id).await.unwrap().name, "Deploy");
    }
}

#[tokio::test]
async fn old_local_tombstone_is_captured_before_remote_resurrection() {
    let (db, bus, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    let legacy = unfour_flow_engine::FlowService::new(db.clone());
    let flow = legacy.save(definition(&ws)).await.unwrap();
    legacy.delete(&ws, &flow.id, flow.revision).await.unwrap();
    queue(
        &service,
        &transport,
        &ws,
        change(&ws, &flow.id, Some(payload()), 18),
    )
    .await;
    assert_eq!(
        service.sync_workspace(&ws).await.unwrap_err(),
        SyncError::Conflict
    );
    assert!(bus.get_flow(ws.clone(), flow.id.clone()).await.is_err());
    let pending: (String, i64) = sqlx::query_as(
        "SELECT operation,content_revision FROM cloud_sync_outbox WHERE entity_type='flow'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(pending, ("delete".into(), flow.revision + 1));
    service
        .keep_local(&ws, SyncEntityType::Flow, &flow.id)
        .await
        .unwrap();
    assert!(bus.get_flow(ws, flow.id).await.is_err());
}

#[tokio::test]
async fn one_page_update_delete_restore_materializes_only_the_last_document() {
    let (_, bus, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    let binding = service.status(&ws).await.unwrap().binding.unwrap();
    let base = binding.last_pulled_cursor;
    let mut last = payload();
    last["name"] = json!("Restored");
    let mut changes = vec![
        change(&ws, FLOW_ID, Some(payload()), 1),
        change(&ws, FLOW_ID, None, 2),
        change(&ws, FLOW_ID, Some(last), 3),
    ];
    for (index, change) in changes.iter_mut().enumerate() {
        change.cursor = base + index as i64 + 1;
    }
    transport.cursor.store((base + 3) as u64, Ordering::SeqCst);
    transport
        .changes
        .lock()
        .unwrap()
        .push_back(unfour_cloud_sync::ChangesPage {
            protocol_version: 5,
            cloud_workspace_id: binding.cloud_workspace_id,
            current_cursor: base + 3,
            next_cursor: base + 3,
            changes,
        });
    service.sync_workspace(&ws).await.unwrap();
    let flow = bus.get_flow(ws.clone(), FLOW_ID.into()).await.unwrap();
    assert_eq!((flow.name.as_str(), flow.revision), ("Restored", 1));
    let mut equivalent = payload();
    equivalent["name"] = json!("Restored");
    queue(
        &service,
        &transport,
        &ws,
        change(&ws, FLOW_ID, Some(equivalent), 4),
    )
    .await;
    service.sync_workspace(&ws).await.unwrap();
    assert_eq!(bus.get_flow(ws, FLOW_ID.into()).await.unwrap().revision, 1);
}
