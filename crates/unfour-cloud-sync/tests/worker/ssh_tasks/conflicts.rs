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
            entity_type: SyncEntityType::SshTask.as_str().into(),
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
            entity_type: SyncEntityType::SshTaskStep.as_str().into(),
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
            entity_type: SyncEntityType::SshTask.as_str().into(),
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
            entity_type: SyncEntityType::SshTaskStep.as_str().into(),
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

fn workspace_delete_tombstone(
    cursor: i64,
    entity_type: SyncEntityType,
    entity_id: String,
    parent_entity_id: Option<String>,
) -> RemoteChange {
    RemoteChange {
        cursor,
        operation_id: "remote-workspace-delete".into(),
        entity_type: entity_type.as_str().into(),
        entity_id,
        parent_entity_id,
        operation: SyncOperation::Delete,
        server_version: 2,
        payload_schema_version: 1,
        payload: None,
        deleted_at: Some("2026-09-06T10:00:00Z".into()),
    }
}

#[tokio::test]
async fn use_remote_workspace_delete_must_resolve_ssh_child_conflicts() {
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
        .save_ssh_task(task_input(&workspace_id, "Task"))
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    bus.save_ssh_task(SshTaskSaveInput {
        id: Some(created.task.id.clone()),
        workspace_id: workspace_id.clone(),
        name: "Edited task".into(),
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
                config_json: serde_json::json!({
                    "command": "echo local edit",
                    "workingDirectory": "",
                    "timeoutSeconds": 30,
                    "continueOnError": false
                }),
            })
            .collect(),
    })
    .await
    .unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let cursor = binding.last_pulled_cursor;
    let mut changes: Vec<RemoteChange> = created
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            workspace_delete_tombstone(
                cursor + index as i64 + 1,
                SyncEntityType::SshTaskStep,
                step.id.clone(),
                Some(created.task.id.clone()),
            )
        })
        .collect();
    let after_steps = cursor + created.steps.len() as i64;
    changes.push(workspace_delete_tombstone(
        after_steps + 1,
        SyncEntityType::SshTask,
        created.task.id.clone(),
        None,
    ));
    changes.push(workspace_delete_tombstone(
        after_steps + 2,
        SyncEntityType::Workspace,
        workspace_id.clone(),
        None,
    ));
    let delete_cursor = after_steps + 2;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
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
    let before = service.status(&workspace_id).await.unwrap();
    assert_eq!(
        before.conflict_count,
        created.steps.len() as i64 + 2,
        "step, task, and workspace conflicts must all be open"
    );
    assert!(before.pending_count >= 1);

    service
        .use_remote(&workspace_id, SyncEntityType::Workspace, &workspace_id)
        .await
        .expect("workspace use-remote must recognize its cross-domain cascade materialization");
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.conflict_count, 0);
    assert_eq!(status.pending_count, 0);
    let tombstoned: (Option<String>, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT deleted_at FROM workspaces WHERE id = ?1),
             (SELECT COUNT(*) FROM ssh_task WHERE id = ?2 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM ssh_task_step WHERE task_id = ?2 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM cloud_sync_outbox
              WHERE entity_id IN (?1, ?2) OR parent_entity_id = ?2)"#,
    )
    .bind(&workspace_id)
    .bind(&created.task.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(tombstoned.0.is_some());
    assert_eq!(tombstoned.1, 0);
    assert_eq!(tombstoned.2, 0);
    assert_eq!(tombstoned.3, 0);
}

#[tokio::test]
async fn use_remote_workspace_delete_must_resolve_already_tombstoned_ssh_children() {
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
        .save_ssh_task(task_input(&workspace_id, "Already deleted child"))
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    bus.save_ssh_task(SshTaskSaveInput {
        id: Some(created.task.id.clone()),
        workspace_id: workspace_id.clone(),
        name: "Edited task".into(),
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
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let cursor = binding.last_pulled_cursor;
    let step_id = created.steps[0].id.clone();
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
        current_cursor: cursor + 3,
        next_cursor: cursor + 3,
        changes: vec![
            workspace_delete_tombstone(
                cursor + 1,
                SyncEntityType::SshTaskStep,
                step_id.clone(),
                Some(created.task.id.clone()),
            ),
            workspace_delete_tombstone(
                cursor + 2,
                SyncEntityType::SshTask,
                created.task.id.clone(),
                None,
            ),
            workspace_delete_tombstone(
                cursor + 3,
                SyncEntityType::Workspace,
                workspace_id.clone(),
                None,
            ),
        ],
    });
    transport
        .cursor
        .store((cursor + 3) as u64, Ordering::SeqCst);
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Conflict
    );
    sqlx::query("UPDATE ssh_task_step SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2")
        .bind("2026-09-06T09:00:00Z")
        .bind(&step_id)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        3
    );

    service
        .use_remote(&workspace_id, SyncEntityType::Workspace, &workspace_id)
        .await
        .expect("already-tombstoned children must count as equivalent cascade materialization");
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.conflict_count, 0);
    assert_eq!(status.pending_count, 0);
    let remaining_live: (i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM ssh_task WHERE id = ?1 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM ssh_task_step WHERE id = ?2 AND deleted_at IS NULL)"#,
    )
    .bind(&created.task.id)
    .bind(&step_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining_live, (0, 0));
}

#[tokio::test]
async fn use_remote_workspace_delete_must_resolve_unmaterialized_future_child() {
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
        .save_ssh_task(SshTaskSaveInput {
            id: None,
            workspace_id: workspace_id.clone(),
            name: "Task".into(),
            description: String::new(),
            default_connection_id: None,
            steps: vec![SshTaskStepInput {
                id: None,
                name: "Step".into(),
                step_type: "command".into(),
                position: 0,
                enabled: true,
                config_version: Some(1),
                config_json: serde_json::json!({
                    "command": "echo original",
                    "workingDirectory": "",
                    "timeoutSeconds": 30,
                    "continueOnError": false
                }),
            }],
        })
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    bus.save_ssh_task(SshTaskSaveInput {
        id: Some(created.task.id.clone()),
        workspace_id: workspace_id.clone(),
        name: "Edited task".into(),
        description: String::new(),
        default_connection_id: None,
        steps: vec![SshTaskStepInput {
            id: Some(created.steps[0].id.clone()),
            name: created.steps[0].name.clone(),
            step_type: created.steps[0].step_type.clone(),
            position: created.steps[0].position,
            enabled: created.steps[0].enabled,
            config_version: Some(created.steps[0].config_version),
            config_json: serde_json::json!({
                "command": "echo local edit",
                "workingDirectory": "",
                "timeoutSeconds": 30,
                "continueOnError": false
            }),
        }],
    })
    .await
    .unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let cursor = binding.last_pulled_cursor;
    let future_step_id = "unmaterialized-future-step";
    let mut future_step_tombstone = workspace_delete_tombstone(
        cursor + 2,
        SyncEntityType::SshTaskStep,
        future_step_id.into(),
        Some(created.task.id.clone()),
    );
    future_step_tombstone.payload_schema_version = 2;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id,
        current_cursor: cursor + 5,
        next_cursor: cursor + 5,
        changes: vec![
            RemoteChange {
                cursor: cursor + 1,
                operation_id: "future-step-create".into(),
                entity_type: SyncEntityType::SshTaskStep.as_str().into(),
                entity_id: future_step_id.into(),
                parent_entity_id: Some(created.task.id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: 2,
                payload: Some(serde_json::json!({"future": true})),
                deleted_at: None,
            },
            future_step_tombstone,
            workspace_delete_tombstone(
                cursor + 3,
                SyncEntityType::SshTaskStep,
                created.steps[0].id.clone(),
                Some(created.task.id.clone()),
            ),
            workspace_delete_tombstone(
                cursor + 4,
                SyncEntityType::SshTask,
                created.task.id.clone(),
                None,
            ),
            workspace_delete_tombstone(
                cursor + 5,
                SyncEntityType::Workspace,
                workspace_id.clone(),
                None,
            ),
        ],
    });
    transport
        .cursor
        .store((cursor + 5) as u64, Ordering::SeqCst);
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Conflict
    );
    let before = service.status(&workspace_id).await.unwrap();
    assert_eq!(before.conflict_count, 4);
    assert!(before.pending_count >= 1);
    let unmaterialized: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_step WHERE id = ?1")
            .bind(future_step_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        unmaterialized, 0,
        "future schema step must never land locally"
    );

    service
        .use_remote(&workspace_id, SyncEntityType::Workspace, &workspace_id)
        .await
        .expect("absent local rows for scoped delete tombstones are already equivalent");
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.conflict_count, 0);
    assert_eq!(status.pending_count, 0);
    let tombstoned: (Option<String>, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT deleted_at FROM workspaces WHERE id = ?1),
             (SELECT COUNT(*) FROM ssh_task WHERE id = ?2 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM ssh_task_step WHERE task_id = ?2 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM ssh_task_step WHERE id = ?3),
             (SELECT COUNT(*) FROM cloud_sync_outbox
              WHERE entity_id IN (?1, ?2, ?3) OR parent_entity_id = ?2)"#,
    )
    .bind(&workspace_id)
    .bind(&created.task.id)
    .bind(future_step_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(tombstoned.0.is_some());
    assert_eq!(tombstoned.1, 0);
    assert_eq!(tombstoned.2, 0);
    assert_eq!(tombstoned.3, 0);
    assert_eq!(tombstoned.4, 0);
}
