//! Aggregate task deletes versus standalone step deletes and local intent.

use super::*;

#[tokio::test]
async fn remote_task_delete_conflict_scopes_over_local_steps_and_keep_local_restores_topology() {
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
    let created = bus
        .save_ssh_task(task_input(&workspace_id, "Keep local"))
        .await
        .unwrap();

    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: 1,
        next_cursor: 1,
        changes: vec![RemoteChange {
            cursor: 1,
            operation_id: "remote-task-delete-conflict".into(),
            entity_type: SyncEntityType::SshTask,
            entity_id: created.task.id.clone(),
            parent_entity_id: None,
            operation: SyncOperation::Delete,
            server_version: 1,
            payload_schema_version: 1,
            payload: None,
            deleted_at: Some("2026-08-17T02:00:00Z".into()),
        }],
    });
    transport.cursor.store(1, Ordering::SeqCst);
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Conflict
    );
    assert!(pushed_operations(&transport).is_empty());
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        1
    );

    service
        .keep_local(&workspace_id, SyncEntityType::SshTask, &created.task.id)
        .await
        .unwrap();
    let operations = pushed_operations(&transport);
    assert_eq!(operations[0].entity_type, SyncEntityType::SshTask);
    assert!(operations[1..]
        .iter()
        .all(|operation| operation.entity_type == SyncEntityType::SshTaskStep));
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        0
    );
}

#[tokio::test]
async fn children_first_remote_task_delete_keeps_all_steps_after_task_only_keep_local() {
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
        .save_ssh_task(task_input(&workspace_id, "Original task"))
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    clear_pushes(&transport);

    bus.save_ssh_task(SshTaskSaveInput {
        id: Some(created.task.id.clone()),
        workspace_id: workspace_id.clone(),
        name: "Local task rename".into(),
        description: created.task.description.clone(),
        default_connection_id: None,
        steps: created
            .steps
            .iter()
            .map(|step| SshTaskStepInput {
                id: Some(step.id.clone()),
                name: step.name.clone(),
                step_type: step.step_type.clone(),
                position: step.position,
                enabled: step.enabled,
                config_version: Some(step.config_version),
                config_json: step.config_json.clone(),
            })
            .collect(),
    })
    .await
    .unwrap();
    let pending: Vec<(String, String)> = sqlx::query_as(
        "SELECT entity_type, entity_id FROM cloud_sync_outbox WHERE local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(pending, vec![("sshTask".into(), created.task.id.clone())]);

    let aggregate_operation_id = "remote-ssh-task-aggregate-delete";
    let changes = created
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| RemoteChange {
            cursor: index as i64 + 1,
            operation_id: aggregate_operation_id.into(),
            entity_type: SyncEntityType::SshTaskStep,
            entity_id: step.id.clone(),
            parent_entity_id: Some(created.task.id.clone()),
            operation: SyncOperation::Delete,
            server_version: 2,
            payload_schema_version: 1,
            payload: None,
            deleted_at: Some("2026-08-18T01:00:00Z".into()),
        })
        .chain(std::iter::once(RemoteChange {
            cursor: created.steps.len() as i64 + 1,
            operation_id: aggregate_operation_id.into(),
            entity_type: SyncEntityType::SshTask,
            entity_id: created.task.id.clone(),
            parent_entity_id: None,
            operation: SyncOperation::Delete,
            server_version: 2,
            payload_schema_version: 1,
            payload: None,
            deleted_at: Some("2026-08-18T01:00:00Z".into()),
        }))
        .collect::<Vec<_>>();
    let delete_cursor = changes.len() as i64;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: delete_cursor,
        next_cursor: delete_cursor,
        changes,
    });
    transport
        .cursor
        .store(delete_cursor as u64, Ordering::SeqCst);

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Conflict
    );
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        created.steps.len() as i64 + 1
    );
    let before_resolution: (Option<String>, i64) = sqlx::query_as(
        r#"SELECT task.deleted_at,
                  (SELECT COUNT(*) FROM ssh_task_step
                   WHERE task_id = task.id AND deleted_at IS NULL)
           FROM ssh_task AS task WHERE task.id = ?1"#,
    )
    .bind(&created.task.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(before_resolution, (None, created.steps.len() as i64));

    service
        .keep_local(&workspace_id, SyncEntityType::SshTask, &created.task.id)
        .await
        .unwrap();
    let survived: (String, Option<String>, i64) = sqlx::query_as(
        r#"SELECT task.name, task.deleted_at,
                  (SELECT COUNT(*) FROM ssh_task_step
                   WHERE task_id = task.id AND deleted_at IS NULL)
           FROM ssh_task AS task WHERE task.id = ?1"#,
    )
    .bind(&created.task.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        survived,
        ("Local task rename".into(), None, created.steps.len() as i64)
    );
    let survived_steps: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT id, name, position FROM ssh_task_step WHERE task_id = ?1 AND deleted_at IS NULL ORDER BY position",
    )
    .bind(&created.task.id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    let expected_steps = created
        .steps
        .iter()
        .map(|step| (step.id.clone(), step.name.clone(), step.position))
        .collect::<Vec<_>>();
    assert_eq!(survived_steps, expected_steps);
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        0
    );
}

#[tokio::test]
async fn standalone_remote_step_delete_still_applies_with_local_task_metadata_intent() {
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
        .save_ssh_task(task_input(&workspace_id, "Original task"))
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();

    bus.save_ssh_task(SshTaskSaveInput {
        id: Some(created.task.id.clone()),
        workspace_id: workspace_id.clone(),
        name: "Local task rename".into(),
        description: created.task.description.clone(),
        default_connection_id: None,
        steps: created
            .steps
            .iter()
            .map(|step| SshTaskStepInput {
                id: Some(step.id.clone()),
                name: step.name.clone(),
                step_type: step.step_type.clone(),
                position: step.position,
                enabled: step.enabled,
                config_version: Some(step.config_version),
                config_json: step.config_json.clone(),
            })
            .collect(),
    })
    .await
    .unwrap();
    let removed_step_id = created.steps[0].id.clone();
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: 1,
        next_cursor: 1,
        changes: vec![RemoteChange {
            cursor: 1,
            operation_id: "remote-standalone-step-delete".into(),
            entity_type: SyncEntityType::SshTaskStep,
            entity_id: removed_step_id.clone(),
            parent_entity_id: Some(created.task.id.clone()),
            operation: SyncOperation::Delete,
            server_version: 2,
            payload_schema_version: 1,
            payload: None,
            deleted_at: Some("2026-08-18T01:00:00Z".into()),
        }],
    });
    transport.cursor.store(1, Ordering::SeqCst);

    service.sync_workspace(&workspace_id).await.unwrap();
    let state: (String, Option<String>, i64) = sqlx::query_as(
        r#"SELECT task.name,
                  (SELECT deleted_at FROM ssh_task_step WHERE id = ?2),
                  (SELECT COUNT(*) FROM ssh_task_step
                   WHERE task_id = task.id AND deleted_at IS NULL)
           FROM ssh_task AS task WHERE task.id = ?1"#,
    )
    .bind(&created.task.id)
    .bind(&removed_step_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state.0, "Local task rename");
    assert!(state.1.is_some());
    assert_eq!(state.2, created.steps.len() as i64 - 1);
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        0
    );
}
