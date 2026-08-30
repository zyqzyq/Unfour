//! Task/step permanent failures, blocked descendants and explicit recovery.

use super::*;

async fn outbox_row(db: &LocalDb, entity_id: &str) -> (String, Option<String>) {
    sqlx::query_as("SELECT status, last_error FROM cloud_sync_outbox WHERE entity_id = ?1")
        .bind(entity_id)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn ssh_task_step_permanent_failure_isolates_only_the_failed_step() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let created = bus
        .save_ssh_task(three_step_task(
            &workspace_id,
            "Deploy",
            &["StepA", "StepB", "StepC"],
        ))
        .await
        .unwrap();
    assert_eq!(created.steps.len(), 3);
    let failed_step = created.steps[1].id.clone();
    transport.fail_operation_once(&failed_step, "invalid_parent_entity");

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let isolated = service.status(&workspace_id).await.unwrap();
    assert_eq!(isolated.dead_count, 1);
    assert_eq!(isolated.pending_count, 3);
    assert_eq!(isolated.dead_letters[0].entity_id, failed_step);
    assert_eq!(isolated.dead_letters[0].error_code, "invalid_parent_entity");
    for entity_id in [
        created.task.id.as_str(),
        created.steps[0].id.as_str(),
        created.steps[2].id.as_str(),
    ] {
        assert_eq!(outbox_row(&db, entity_id).await, ("pending".into(), None));
    }

    let due = service
        .repository()
        .due_outbox(
            "account-a",
            &isolated.binding.as_ref().unwrap().cloud_workspace_id,
            Utc::now(),
            50,
        )
        .await
        .unwrap();
    let due_ids = due
        .iter()
        .map(|entry| entry.entity_id.as_str())
        .collect::<Vec<_>>();
    assert!(due_ids.contains(&created.task.id.as_str()));
    assert!(due_ids.contains(&created.steps[0].id.as_str()));
    assert!(due_ids.contains(&created.steps[2].id.as_str()));
    assert!(!due_ids.contains(&failed_step.as_str()));

    clear_pushes(&transport);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::DeadLetterBlocked)
    ));
    let after_retry = service.status(&workspace_id).await.unwrap();
    assert_eq!(after_retry.dead_count, 1);
    assert_eq!(after_retry.pending_count, 0);
    assert_eq!(after_retry.dead_letters[0].entity_id, failed_step);
    let surviving = pushed_operations(&transport);
    let surviving_ids = surviving
        .iter()
        .map(|operation| operation.entity_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(surviving[0].entity_id, created.task.id);
    assert!(surviving_ids.contains(&created.steps[0].id.as_str()));
    assert!(surviving_ids.contains(&created.steps[2].id.as_str()));
    assert!(!surviving_ids.contains(&failed_step.as_str()));
}

#[tokio::test]
async fn ssh_task_permanent_failure_keeps_steps_pending_and_blocked() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let created = bus
        .save_ssh_task(three_step_task(
            &workspace_id,
            "Deploy",
            &["StepA", "StepB", "StepC"],
        ))
        .await
        .unwrap();
    transport.fail_operation_once(&created.task.id, "invalid_sync_entity");

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let failed = service.status(&workspace_id).await.unwrap();
    assert_eq!(failed.dead_count, 1);
    assert_eq!(failed.dead_letters[0].entity_id, created.task.id);
    assert_eq!(failed.pending_count, 3);
    for step in &created.steps {
        assert_eq!(outbox_row(&db, &step.id).await, ("pending".into(), None));
    }
    let due = service
        .repository()
        .due_outbox(
            "account-a",
            &failed.binding.as_ref().unwrap().cloud_workspace_id,
            Utc::now(),
            50,
        )
        .await
        .unwrap();
    assert!(
        due.is_empty(),
        "dead SSH Task descendants must stay out of the due queue"
    );
    let push_count = transport.pushes.lock().unwrap().len();
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::DeadLetterBlocked)
    ));
    assert_eq!(transport.pushes.lock().unwrap().len(), push_count);

    service
        .retry_dead_letter_current_local(&workspace_id, &failed.dead_letters[0].operation_id)
        .await
        .unwrap();
    let repaired = service.status(&workspace_id).await.unwrap();
    assert_eq!(repaired.dead_count, 0);
    assert_eq!(repaired.pending_count, 0);
    let repaired_batch = transport
        .pushes
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("task recovery push");
    assert_eq!(repaired_batch.operations[0].entity_id, created.task.id);
    let pushed_ids = repaired_batch
        .operations
        .iter()
        .map(|operation| operation.entity_id.clone())
        .collect::<Vec<_>>();
    for step in &created.steps {
        assert!(pushed_ids.contains(&step.id));
    }
}

#[tokio::test]
async fn unknown_operation_id_uses_conservative_ssh_batch_fallback() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let created = bus
        .save_ssh_task(three_step_task(
            &workspace_id,
            "Deploy",
            &["StepA", "StepB", "StepC"],
        ))
        .await
        .unwrap();
    transport.fail_unknown_operation_once("not-in-this-batch", "invalid_sync_entity");

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 4);
    assert_eq!(status.pending_count, 0);
    for entity_id in [
        created.task.id.as_str(),
        created.steps[0].id.as_str(),
        created.steps[1].id.as_str(),
        created.steps[2].id.as_str(),
    ] {
        assert_eq!(outbox_row(&db, entity_id).await.0, "dead".to_string());
    }
}
