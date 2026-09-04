//! Connection operation-level failures and conservative batch fallback.

use super::*;

#[tokio::test]
async fn permanent_connection_failure_isolated_from_batch_peers() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let connection = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Dead Connection",
            "private-key",
            Some(r"C:\device\dead-key"),
        ))
        .await
        .unwrap();
    let variable = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "SURVIVOR", "value", false),
        )
        .await
        .unwrap();
    transport.fail_operation_once(&connection.id, "invalid_sync_entity");
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Permanent
    );
    let failed = service.status(&workspace_id).await.unwrap();
    assert_eq!(failed.dead_count, 1);
    assert_eq!(failed.pending_count, 1);
    assert_eq!(failed.dead_letters[0].entity_type, "connection");
    assert_eq!(failed.dead_letters[0].entity_id, connection.id);

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::DeadLetterBlocked
    );
    let remaining = service.status(&workspace_id).await.unwrap();
    assert_eq!(remaining.dead_count, 1);
    assert_eq!(remaining.pending_count, 0);
    assert!(pushed_operations(&transport)
        .iter()
        .any(|operation| operation.entity_id == variable.id));
}

#[tokio::test]
async fn unknown_operation_id_preserves_the_atomic_batch_for_attention() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let bus = CommandBus::from_db_with_extensions(db, CommandBusExtensions::new(vec![hook]))
        .await
        .unwrap();
    bus.save_ssh_connection(ssh_input(
        &workspace_id,
        None,
        "Unknown Operation Connection",
        "private-key",
        Some(r"C:\device\unknown-operation-key"),
    ))
    .await
    .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "UNKNOWN_OPERATION_PEER", "value", false),
    )
    .await
    .unwrap();
    transport.fail_unknown_operation_once("not-in-this-batch", "invalid_sync_entity");

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Permanent
    );
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 0);
    assert_eq!(status.pending_count, 2);
    assert_eq!(status.binding.unwrap().state, "error");
}
