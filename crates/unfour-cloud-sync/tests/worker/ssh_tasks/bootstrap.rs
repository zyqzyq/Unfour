//! One-time v2-to-v3 task backfill and rollback.

use super::*;

async fn mark_binding_for_v3_bootstrap(db: &LocalDb, workspace_id: &str) {
    sqlx::query(
        r#"UPDATE cloud_sync_workspace_bindings
           SET ssh_task_v3_bootstrap_state = 'pending'
           WHERE account_id = 'account-a' AND local_workspace_id = ?1"#,
    )
    .bind(workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn existing_v2_binding_backfills_ssh_tasks_once_before_pull() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);

    let created = seed
        .save_ssh_task(task_input(&workspace_id, "Pre-v3 deploy"))
        .await
        .unwrap();
    mark_binding_for_v3_bootstrap(&db, &workspace_id).await;

    service.enable(&workspace_id).await.unwrap();
    let operations = pushed_operations(&transport);
    assert_eq!(operations.len(), 1 + created.steps.len());
    assert_eq!(operations[0].entity_id, created.task.id);
    assert!(operations[1..]
        .iter()
        .all(|operation| operation.entity_type == SyncEntityType::SshTaskStep));
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.ssh_task_v3_bootstrap_state, "completed");

    clear_pushes(&transport);
    service.enable(&workspace_id).await.unwrap();
    assert!(pushed_operations(&transport).is_empty());
    let outbox_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE local_workspace_id = ?1")
            .bind(&workspace_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn ssh_task_v3_backfill_rolls_back_partial_intents_and_retries() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    service.enable(&workspace_id).await.unwrap();

    let created = seed
        .save_ssh_task(task_input(&workspace_id, "Rollback deploy"))
        .await
        .unwrap();
    let step_id = created.steps.last().unwrap().id.clone();
    let valid_config: String =
        sqlx::query_scalar("SELECT config_json FROM ssh_task_step WHERE id = ?1")
            .bind(&step_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    sqlx::query("UPDATE ssh_task_step SET config_json = '{' WHERE id = ?1")
        .bind(&step_id)
        .execute(db.pool())
        .await
        .unwrap();
    mark_binding_for_v3_bootstrap(&db, &workspace_id).await;

    assert_eq!(
        service.enable(&workspace_id).await.unwrap_err(),
        SyncError::Core
    );
    let rolled_back: (String, i64) = sqlx::query_as(
        r#"SELECT ssh_task_v3_bootstrap_state,
                  (SELECT COUNT(*) FROM cloud_sync_outbox
                   WHERE local_workspace_id = ?1)
           FROM cloud_sync_workspace_bindings
           WHERE account_id = 'account-a' AND local_workspace_id = ?1"#,
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rolled_back, ("pending".into(), 0));

    sqlx::query("UPDATE ssh_task_step SET config_json = ?1 WHERE id = ?2")
        .bind(valid_config)
        .bind(step_id)
        .execute(db.pool())
        .await
        .unwrap();
    service.enable(&workspace_id).await.unwrap();
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .ssh_task_v3_bootstrap_state,
        "completed"
    );
}

#[tokio::test]
async fn remote_same_id_conflicts_with_v3_backfilled_local_ssh_task() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let created = seed
        .save_ssh_task(task_input(&workspace_id, "Keep pre-v3 local"))
        .await
        .unwrap();
    mark_binding_for_v3_bootstrap(&db, &workspace_id).await;
    let cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    let mut remote = remote_task(cursor + 1, "remote-same-task-id");
    remote.entity_id = created.task.id.clone();
    remote.payload.as_mut().unwrap()["name"] = serde_json::json!("Remote overwrite");
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor + 1,
        next_cursor: cursor + 1,
        changes: vec![remote],
    });
    transport
        .cursor
        .store((cursor + 1) as u64, Ordering::SeqCst);

    assert_eq!(
        service.enable(&workspace_id).await.unwrap_err(),
        SyncError::Conflict
    );
    assert!(pushed_operations(&transport).is_empty());
    let local_name: String = sqlx::query_scalar("SELECT name FROM ssh_task WHERE id = ?1")
        .bind(&created.task.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(local_name, "Keep pre-v3 local");
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.conflict_count, 1);
    assert_eq!(
        status.binding.unwrap().ssh_task_v3_bootstrap_state,
        "completed"
    );
}
