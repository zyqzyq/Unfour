//! Generic live-entity repair for historical outbox gaps.

use super::support::*;

#[tokio::test]
async fn generic_repair_requeues_live_core_entities_redacts_secrets_and_is_idempotent() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    seed.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "SECRET", "must-not-sync", true),
    )
    .await
    .unwrap();
    let environment = seed
        .workspace_environment_create(workspace_id.clone(), "Test".into())
        .await
        .unwrap();
    let environment_variable = seed
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            variable(None, "HOST", "example.test", false),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();

    // Simulate the historical orphan shape without changing any Core row.
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM cloud_sync_entity_state")
        .execute(db.pool())
        .await
        .unwrap();

    service.sync_workspace(&workspace_id).await.unwrap();

    let pushes = transport.pushes.lock().unwrap();
    let operations = pushes
        .iter()
        .flat_map(|request| request.operations.iter())
        .collect::<Vec<_>>();
    assert!(operations.iter().any(|operation| {
        operation.entity_type == SyncEntityType::Workspace && operation.entity_id == workspace_id
    }));
    let environment_index = operations
        .iter()
        .position(|operation| {
            operation.entity_type == SyncEntityType::WorkspaceEnvironment
                && operation.entity_id == environment.id
        })
        .expect("environment repaired");
    let environment_variable_index = operations
        .iter()
        .position(|operation| {
            operation.entity_type == SyncEntityType::WorkspaceEnvironmentVariable
                && operation.entity_id == environment_variable.id
        })
        .expect("environment variable repaired");
    assert!(
        environment_index < environment_variable_index,
        "parent must be pushed before environment variable"
    );
    let secret = operations
        .iter()
        .find(|operation| {
            operation.entity_type == SyncEntityType::WorkspaceVariable
                && operation
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("key"))
                    .and_then(serde_json::Value::as_str)
                    == Some("SECRET")
        })
        .expect("secret variable repaired");
    assert!(secret
        .payload
        .as_ref()
        .and_then(|payload| payload.get("value"))
        .is_none());
    assert!(!serde_json::to_string(&*pushes)
        .unwrap()
        .contains("must-not-sync"));
    drop(pushes);

    let push_count = transport.pushes.lock().unwrap().len();
    service.sync_workspace(&workspace_id).await.unwrap();
    assert_eq!(transport.pushes.lock().unwrap().len(), push_count);
    let orphan_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE local_workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(orphan_count, 0);
}
