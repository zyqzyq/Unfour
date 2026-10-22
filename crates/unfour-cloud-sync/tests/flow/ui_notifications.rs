use super::lifecycle::{fixture, queue};
use super::*;
use std::sync::Arc;

#[tokio::test]
async fn remote_flow_commit_notifies_open_views_and_quiet_sync_does_not() {
    let (_, bus, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    service.set_workspace_change_callback(Arc::new(move |workspace_id| {
        sender.send(workspace_id.to_string()).unwrap();
    }));
    queue(
        &service,
        &transport,
        &ws,
        change(&ws, FLOW_ID, Some(payload()), 30),
    )
    .await;
    service.sync_workspace(&ws).await.unwrap();
    assert_eq!(receiver.try_recv().unwrap(), ws);
    assert_eq!(
        bus.get_flow(ws.clone(), FLOW_ID.into()).await.unwrap().name,
        "Deploy"
    );
    service.sync_workspace(&ws).await.unwrap();
    assert!(receiver.try_recv().is_err());
    queue(&service, &transport, &ws, change(&ws, FLOW_ID, None, 31)).await;
    service.sync_workspace(&ws).await.unwrap();
    assert_eq!(receiver.try_recv().unwrap(), ws);
    assert!(bus.get_flow(ws, FLOW_ID.into()).await.is_err());
}

#[tokio::test]
async fn rolled_back_remote_flow_does_not_notify_open_views() {
    let (db, bus, service, transport, ws) = fixture().await;
    service.enable(&ws).await.unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    service.set_workspace_change_callback(Arc::new(move |workspace_id| {
        sender.send(workspace_id.to_string()).unwrap();
    }));
    // Fail cursor persistence after domain materialization but before commit.
    sqlx::query("CREATE TRIGGER fail_cursor BEFORE UPDATE OF last_pulled_cursor ON cloud_sync_workspace_bindings BEGIN SELECT RAISE(FAIL, 'rollback fixture'); END")
        .execute(db.pool()).await.unwrap();
    queue(
        &service,
        &transport,
        &ws,
        change(&ws, FLOW_ID, Some(payload()), 30),
    )
    .await;
    assert!(service.sync_workspace(&ws).await.is_err());
    assert!(receiver.try_recv().is_err());
    assert!(bus.get_flow(ws, FLOW_ID.into()).await.is_err());
}

#[tokio::test]
async fn unresolved_conflict_stays_quiet_and_using_remote_notifies_views() {
    let (_, bus, service, transport, ws) = fixture().await;
    let local = bus.save_flow(definition(&ws)).await.unwrap();
    service.enable(&ws).await.unwrap();
    let mut edited = local.clone();
    edited.name = "Local draft".into();
    bus.save_flow(edited).await.unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    service.set_workspace_change_callback(Arc::new(move |workspace_id| {
        sender.send(workspace_id.to_string()).unwrap();
    }));
    queue(
        &service,
        &transport,
        &ws,
        change(&ws, &local.id, Some(payload()), 30),
    )
    .await;
    assert_eq!(
        service.sync_workspace(&ws).await.unwrap_err(),
        SyncError::Conflict
    );
    assert!(receiver.try_recv().is_err());
    service
        .use_remote(&ws, unfour_cloud_sync::SyncEntityType::Flow, &local.id)
        .await
        .unwrap();
    assert_eq!(receiver.try_recv().unwrap(), ws);
    assert_eq!(bus.get_flow(ws, local.id).await.unwrap().name, "Deploy");
}
