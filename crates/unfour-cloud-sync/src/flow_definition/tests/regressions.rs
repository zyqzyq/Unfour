use super::lifecycle::{fixture, queue};
use super::*;
use crate::SyncEntityType;

#[tokio::test]
async fn local_tombstone_conflict_can_keep_delete_or_restore_remote() {
    for keep_local in [true, false] {
        let (db, bus, service, transport, ws) = fixture().await;
        let flow = bus.save_flow(definition(&ws)).await.unwrap();
        service.enable(&ws).await.unwrap();
        bus.delete_flow(ws.clone(), flow.id.clone(), flow.revision)
            .await
            .unwrap();
        queue(
            &service,
            &transport,
            &ws,
            change(&ws, &flow.id, Some(payload()), 30),
        )
        .await;
        assert_eq!(
            service.sync_workspace(&ws).await.unwrap_err(),
            SyncError::Conflict
        );
        if keep_local {
            service
                .keep_local(&ws, SyncEntityType::Flow, &flow.id)
                .await
                .unwrap();
            assert!(bus.get_flow(ws.clone(), flow.id.clone()).await.is_err());
            let pushes = transport.pushes.lock().unwrap();
            let last = pushes
                .iter()
                .flat_map(|p| &p.operations)
                .filter(|p| p.entity_type == SyncEntityType::Flow)
                .last()
                .unwrap();
            assert_eq!(last.operation, SyncOperation::Delete);
            assert_eq!(last.base_version, 30);
        } else {
            service
                .use_remote(&ws, SyncEntityType::Flow, &flow.id)
                .await
                .unwrap();
            assert_eq!(
                bus.get_flow(ws.clone(), flow.id.clone())
                    .await
                    .unwrap()
                    .revision,
                flow.revision + 2
            );
        }
        assert!(bus.save_flow(flow).await.is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM cloud_sync_outbox WHERE entity_type='flow'"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            0
        );
    }
}

#[tokio::test]
async fn repair_captures_preexisting_flow_and_unsafe_edits_roll_back() {
    let (db, bus, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    // Simulates a workspace bound by an older client that did not capture Flow.
    let flow = unfour_flow_engine::FlowService::new(db.clone())
        .save(definition(&ws))
        .await
        .unwrap();
    service.sync_workspace(&ws).await.unwrap();
    assert!(transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|p| &p.operations)
        .any(|p| p.entity_type == SyncEntityType::Flow && p.entity_id == flow.id));
    let mut unsafe_flow = flow.clone();
    unsafe_flow.inputs[0].secret = false;
    unsafe_flow.inputs[0].name = "label".into();
    unsafe_flow.inputs[0].default = Some(json!("opaque literal"));
    let error = bus.save_flow(unsafe_flow).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("cloud_sync_flow_unsafe_definition"));
    assert_eq!(
        bus.get_flow(ws, flow.id).await.unwrap().revision,
        flow.revision
    );
}

#[tokio::test]
async fn snapshot_preserves_unknown_local_step_fields_for_rejection() {
    let (db, bus, _, _, ws) = fixture().await;
    let flow = bus.save_flow(definition(&ws)).await.unwrap();
    sqlx::query("UPDATE flow_definitions SET definition_json=json_set(definition_json, '$.steps[0].futureExecution', 1) WHERE id=?")
        .bind(&flow.id).execute(db.pool()).await.unwrap();
    let snapshot = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::FlowDefinition,
            &ws,
            &flow.id,
        ))
        .await
        .unwrap();
    let DomainSnapshot::FlowDefinition(snapshot) = snapshot else {
        panic!()
    };
    assert_eq!(
        canonical_payload(&snapshot).unwrap_err(),
        SyncError::InvalidData
    );
}
