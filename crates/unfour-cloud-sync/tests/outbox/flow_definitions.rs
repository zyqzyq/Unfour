use super::*;

#[tokio::test]
async fn flow_mutations_capture_canonical_intent_and_trigger_sync_when_bound() {
    let (_seed, db, ws) = hooked_bus(vec![]).await;
    let (trigger, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let bus = CommandBus::from_db_with_extensions(
        db.clone(),
        CommandBusExtensions::new(vec![Arc::new(SyncOutboxHook::new(
            Arc::new(SequenceIds::default()),
            Arc::new(FixedClock),
            Some(trigger),
        ))]),
    )
    .await
    .unwrap();
    let flow = serde_json::from_value(serde_json::json!({"id":"", "workspaceId":ws,
        "name":"Local", "revision":0,"inputs":[],"steps":[{"id":"wait","name":"Wait",
        "kind":"wait","durationMs":0,"timeoutMs":1000}]}))
    .unwrap();
    let saved = bus.save_flow(flow).await.unwrap();
    let revised = bus.save_flow(saved).await.unwrap();
    bus.delete_flow(ws.clone(), revised.id.clone(), revised.revision)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM cloud_sync_outbox")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    for _ in 0..3 {
        assert_eq!(receiver.try_recv().unwrap(), ws);
    }
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        outbox_row(&db, &revised.id).await,
        ("flow".into(), "delete".into(), Some(ws.clone()))
    );

    // Initial upload captures live definitions; old tombstones are unnecessary
    // for a new cloud workspace. Bound-workspace deletions above are durable.
    let fresh = bus
        .create_workspace("Initial upload".into())
        .await
        .unwrap()
        .id;
    let flow = serde_json::from_value(serde_json::json!({"id":"", "workspaceId":fresh,
        "name":"Local", "revision":0,"inputs":[],"steps":[{"id":"wait","name":"Wait",
        "kind":"wait","durationMs":0,"timeoutMs":1000}]}))
    .unwrap();
    let live = bus.save_flow(flow).await.unwrap();
    let dead = bus.save_flow(live.clone()).await.unwrap();
    bus.delete_flow(fresh.clone(), dead.id.clone(), dead.revision)
        .await
        .unwrap();
    let mut new_flow = live.clone();
    new_flow.id.clear();
    let live = bus.save_flow(new_flow).await.unwrap();
    let repository = SyncRepository::new(db.pool().clone());
    create_registry_binding(
        &repository,
        &db,
        "account-1",
        0,
        &fresh,
        "cloud-2",
        &SequenceIds::default(),
        &FixedClock,
    )
    .await
    .unwrap();
    for (id, expected) in [(&live.id, 1), (&dead.id, 0)] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM cloud_sync_outbox WHERE entity_id=?"
            )
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
            expected
        );
    }
}
