use super::*;
use chrono::Utc;
use unfour_core::models::{SshConnectionInput, SshTaskSaveInput, SshTaskStepInput};

fn command_step(id: Option<String>, name: &str, position: i64, command: &str) -> SshTaskStepInput {
    SshTaskStepInput {
        id,
        name: name.into(),
        step_type: "command".into(),
        position,
        enabled: true,
        config_version: Some(1),
        config_json: serde_json::json!({
            "command": command,
            "workingDirectory": "",
            "timeoutSeconds": 30,
            "continueOnError": false
        }),
    }
}

fn task_input(workspace_id: &str, name: &str) -> SshTaskSaveInput {
    SshTaskSaveInput {
        id: None,
        workspace_id: workspace_id.into(),
        name: name.into(),
        description: "Cloud task".into(),
        default_connection_id: None,
        steps: vec![
            command_step(None, "Build", 0, "cargo build"),
            command_step(None, "Restart", 1, "systemctl restart app"),
        ],
    }
}

fn three_step_task(workspace_id: &str, name: &str, steps: &[&str]) -> SshTaskSaveInput {
    SshTaskSaveInput {
        id: None,
        workspace_id: workspace_id.into(),
        name: name.into(),
        description: "Cloud task".into(),
        default_connection_id: None,
        steps: steps
            .iter()
            .enumerate()
            .map(|(index, step_name)| {
                command_step(None, step_name, index as i64, &format!("echo {step_name}"))
            })
            .collect(),
    }
}

fn pushed_operations(transport: &MockTransport) -> Vec<unfour_cloud_sync::PushOperation> {
    transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|request| request.operations.iter().cloned())
        .collect()
}

fn clear_pushes(transport: &MockTransport) {
    transport.pushes.lock().unwrap().clear();
}

fn remote_task(cursor: i64, operation_id: &str) -> RemoteChange {
    RemoteChange {
        cursor,
        operation_id: operation_id.into(),
        entity_type: SyncEntityType::SshTask,
        entity_id: "remote-task".into(),
        parent_entity_id: None,
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: 1,
        payload: Some(serde_json::json!({
            "name": "Remote task",
            "description": "Restored without device state",
            "sortOrder": 0,
            "createdAt": "2026-08-17T00:00:00Z",
            "updatedAt": "2026-08-17T00:00:00Z"
        })),
        deleted_at: None,
    }
}

fn remote_step(cursor: i64, operation_id: &str) -> RemoteChange {
    RemoteChange {
        cursor,
        operation_id: operation_id.into(),
        entity_type: SyncEntityType::SshTaskStep,
        entity_id: "remote-step".into(),
        parent_entity_id: Some("remote-task".into()),
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: 1,
        payload: Some(serde_json::json!({
            "taskId": "remote-task",
            "name": "Restart",
            "stepType": "command",
            "position": 0,
            "enabled": true,
            "configVersion": 1,
            "configJson": {
                "command": "systemctl restart app",
                "workingDirectory": "",
                "timeoutSeconds": 30,
                "continueOnError": false
            },
            "createdAt": "2026-08-17T00:00:00Z",
            "updatedAt": "2026-08-17T00:00:00Z"
        })),
        deleted_at: None,
    }
}

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

#[tokio::test]
async fn initial_ssh_task_upload_uses_core_canonical_snapshots_and_topology() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let connection = seed
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace_id.clone(),
            name: "Device SSH".into(),
            host: "device.internal".into(),
            port: Some(22),
            username: "developer".into(),
            auth_kind: "private-key".into(),
            key_path: Some(r"C:\Users\alice\.ssh\id_ed25519".into()),
            credential_ref: Some("device-credential-ref".into()),
            secret: None,
        })
        .await
        .unwrap();
    let literal_local_path = r"C:\Users\alice\artifact.tar";
    let created = seed
        .save_ssh_task(SshTaskSaveInput {
            id: None,
            workspace_id: workspace_id.clone(),
            name: "Upload artifact".into(),
            description: "Canonical snapshot fixture".into(),
            default_connection_id: Some(connection.id.clone()),
            steps: vec![SshTaskStepInput {
                id: None,
                name: "Upload".into(),
                step_type: "upload".into(),
                position: 0,
                enabled: true,
                config_version: Some(1),
                config_json: serde_json::json!({
                    "localPath": literal_local_path,
                    "remotePath": "/tmp/artifact.tar",
                    "overwrite": true
                }),
            }],
        })
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO ssh_task_run (
             id, workspace_id, task_id, connection_id, status, started_at,
             finished_at, error_message, log_path
           ) VALUES ('device-run', ?1, ?2, ?3, 'success', '2026-08-17T00:00:00Z',
             '2026-08-17T00:01:00Z', NULL, 'device-only.log')"#,
    )
    .bind(&workspace_id)
    .bind(&created.task.id)
    .bind(&connection.id)
    .execute(db.pool())
    .await
    .unwrap();

    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();

    let operations = pushed_operations(&transport);
    let binding = service
        .repository()
        .binding("account-a", &workspace_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.ssh_task_v3_bootstrap_state, "completed");
    assert_eq!(binding.initial_total as usize, operations.len());
    let task_index = operations
        .iter()
        .position(|operation| operation.entity_id == created.task.id)
        .unwrap();
    let connection_index = operations
        .iter()
        .position(|operation| operation.entity_id == connection.id)
        .unwrap();
    let step_index = operations
        .iter()
        .position(|operation| operation.entity_id == created.steps[0].id)
        .unwrap();
    assert!(task_index < step_index);
    assert_eq!(SyncEntityType::SshTask.topology_rank(), 1);
    assert_eq!(SyncEntityType::SshTaskStep.topology_rank(), 2);

    let connection_operation = &operations[connection_index];
    assert_eq!(connection_operation.entity_type, SyncEntityType::Connection);
    assert!(connection_operation.parent_entity_id.is_none());
    assert_payload_keys(
        connection_operation.payload.as_ref().unwrap(),
        &[
            "config",
            "connectionType",
            "createdAt",
            "host",
            "id",
            "name",
            "port",
            "updatedAt",
            "workspaceId",
        ],
    );
    assert_eq!(
        connection_operation.payload.as_ref().unwrap()["config"],
        serde_json::json!({
            "kind": "ssh",
            "username": "developer",
            "authMethod": "private-key"
        })
    );

    let task = &operations[task_index];
    assert_eq!(task.entity_type, SyncEntityType::SshTask);
    assert!(task.parent_entity_id.is_none());
    assert_payload_keys(
        task.payload.as_ref().unwrap(),
        &["createdAt", "description", "name", "sortOrder", "updatedAt"],
    );
    let step = &operations[step_index];
    assert_eq!(step.entity_type, SyncEntityType::SshTaskStep);
    assert_eq!(
        step.parent_entity_id.as_deref(),
        Some(created.task.id.as_str())
    );
    assert_eq!(
        step.payload.as_ref().unwrap()["taskId"],
        created.task.id.as_str()
    );
    assert_payload_keys(
        step.payload.as_ref().unwrap(),
        &[
            "configJson",
            "configVersion",
            "createdAt",
            "enabled",
            "name",
            "position",
            "stepType",
            "taskId",
            "updatedAt",
        ],
    );
    let canonical_local_path = step.payload.as_ref().unwrap()["configJson"]["localPath"]
        .as_str()
        .unwrap();
    assert!(canonical_local_path.starts_with("{{local_path_"));

    let serialized = serde_json::to_string(&operations).unwrap();
    for device_local in [
        literal_local_path,
        r"C:\Users\alice\.ssh\id_ed25519",
        "device-credential-ref",
        "device-run",
        "device-only.log",
        "defaultConnectionId",
        "lastUsedConnectionId",
        "runtimeInputValue",
        "transferProgress",
        "executionResult",
    ] {
        assert!(!serialized.contains(device_local), "leaked {device_local}");
    }
}

#[tokio::test]
async fn local_ssh_task_edits_and_deletes_share_push_retry_and_ordering() {
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
        .save_ssh_task(task_input(&workspace_id, "Deploy"))
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    let operations = pushed_operations(&transport);
    assert_eq!(operations[0].entity_type, SyncEntityType::SshTask);
    assert!(operations[1..]
        .iter()
        .all(|operation| operation.entity_type == SyncEntityType::SshTaskStep));
    assert!(operations
        .iter()
        .all(|operation| operation.payload.is_some()));

    clear_pushes(&transport);
    let updated = bus
        .save_ssh_task(SshTaskSaveInput {
            id: Some(created.task.id.clone()),
            workspace_id: workspace_id.clone(),
            name: "Deploy production".into(),
            description: "Updated cloud task".into(),
            default_connection_id: None,
            steps: vec![
                command_step(
                    Some(created.steps[1].id.clone()),
                    "Restart first",
                    0,
                    "systemctl restart app --no-block",
                ),
                command_step(
                    Some(created.steps[0].id.clone()),
                    "Build second",
                    1,
                    "cargo build --release",
                ),
            ],
        })
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    let operations = pushed_operations(&transport);
    let task = operations
        .iter()
        .find(|operation| operation.entity_id == created.task.id)
        .unwrap();
    assert_eq!(task.payload.as_ref().unwrap()["name"], "Deploy production");
    for step in &updated.steps {
        let operation = operations
            .iter()
            .find(|operation| operation.entity_id == step.id)
            .unwrap();
        assert_eq!(
            operation.payload.as_ref().unwrap()["position"],
            step.position
        );
        assert_eq!(
            operation.parent_entity_id.as_deref(),
            Some(created.task.id.as_str())
        );
    }

    clear_pushes(&transport);
    let removed_id = updated.steps[1].id.clone();
    bus.save_ssh_task(SshTaskSaveInput {
        id: Some(updated.task.id.clone()),
        workspace_id: workspace_id.clone(),
        name: updated.task.name.clone(),
        description: updated.task.description.clone(),
        default_connection_id: None,
        steps: vec![command_step(
            Some(updated.steps[0].id.clone()),
            "Restart only",
            0,
            "systemctl restart app",
        )],
    })
    .await
    .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    let operations = pushed_operations(&transport);
    let delete = operations
        .iter()
        .find(|operation| operation.entity_id == removed_id)
        .unwrap();
    assert_eq!(delete.operation, SyncOperation::Delete);
    assert!(delete.payload.is_none());

    clear_pushes(&transport);
    bus.delete_ssh_task(workspace_id.clone(), created.task.id.clone())
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    let operations = pushed_operations(&transport);
    assert_eq!(operations.len(), 2);
    assert_eq!(operations[0].entity_type, SyncEntityType::SshTaskStep);
    assert_eq!(operations[1].entity_type, SyncEntityType::SshTask);
    assert!(operations
        .iter()
        .all(|operation| operation.operation == SyncOperation::Delete));
}

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

#[tokio::test]
async fn pulled_ssh_tasks_apply_idempotently_without_bindings_runs_or_outbox_echo() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let start_cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: start_cursor + 2,
        next_cursor: start_cursor + 2,
        changes: vec![
            remote_task(start_cursor + 1, "remote-task-op"),
            remote_step(start_cursor + 2, "remote-step-op"),
        ],
    });
    transport
        .cursor
        .store((start_cursor + 2) as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();

    let restored: (String, String) = sqlx::query_as(
        "SELECT task.name, step.name FROM ssh_task task JOIN ssh_task_step step ON step.task_id = task.id WHERE task.id = 'remote-task' AND step.id = 'remote-step'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(restored, ("Remote task".into(), "Restart".into()));
    let device_state: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM ssh_task_local_binding WHERE task_id = 'remote-task'),
             (SELECT COUNT(*) FROM ssh_task_run WHERE task_id = 'remote-task'),
             (SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id IN ('remote-task', 'remote-step'))"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(device_state, (0, 0, 0));

    let duplicate_cursor = start_cursor + 3;
    let mut duplicate = remote_step(duplicate_cursor, "remote-step-op");
    duplicate.server_version = 1;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: duplicate_cursor,
        next_cursor: duplicate_cursor,
        changes: vec![duplicate],
    });
    transport
        .cursor
        .store(duplicate_cursor as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    let duplicate_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ssh_task_step WHERE id = 'remote-step' AND deleted_at IS NULL",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(duplicate_count, 1);

    let delete_cursor = duplicate_cursor + 2;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: delete_cursor,
        next_cursor: delete_cursor,
        changes: vec![
            RemoteChange {
                cursor: duplicate_cursor + 1,
                operation_id: "remote-task-delete".into(),
                entity_type: SyncEntityType::SshTask,
                entity_id: "remote-task".into(),
                parent_entity_id: None,
                operation: SyncOperation::Delete,
                server_version: 2,
                payload_schema_version: 1,
                payload: None,
                deleted_at: Some("2026-08-17T02:00:00Z".into()),
            },
            RemoteChange {
                cursor: delete_cursor,
                operation_id: "remote-step-delete".into(),
                entity_type: SyncEntityType::SshTaskStep,
                entity_id: "remote-step".into(),
                parent_entity_id: Some("remote-task".into()),
                operation: SyncOperation::Delete,
                server_version: 2,
                payload_schema_version: 1,
                payload: None,
                deleted_at: Some("2026-08-17T02:00:00Z".into()),
            },
        ],
    });
    transport
        .cursor
        .store(delete_cursor as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    let deleted: (bool, bool) = sqlx::query_as(
        r#"SELECT task.deleted_at IS NOT NULL, step.deleted_at IS NOT NULL
           FROM ssh_task task JOIN ssh_task_step step ON step.task_id = task.id
           WHERE task.id = 'remote-task' AND step.id = 'remote-step'"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(deleted, (true, true));
    let echoed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id IN ('remote-task', 'remote-step')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(echoed, 0);
}

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

#[tokio::test]
async fn pull_workspace_delete_cascades_ssh_tasks_and_steps() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let created = seed
        .save_ssh_task(three_step_task(
            &workspace_id,
            "Deploy",
            &["StepA", "StepB", "StepC"],
        ))
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    let base_cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base_cursor + 1,
        next_cursor: base_cursor + 1,
        changes: vec![RemoteChange {
            cursor: base_cursor + 1,
            operation_id: "remote-workspace-delete".into(),
            entity_type: SyncEntityType::Workspace,
            entity_id: workspace_id.clone(),
            parent_entity_id: None,
            operation: SyncOperation::Delete,
            server_version: 7,
            payload_schema_version: 1,
            payload: None,
            deleted_at: Some("2026-08-19T02:00:00Z".into()),
        }],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let tombstoned: (Option<String>, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT deleted_at FROM workspaces WHERE id = ?1),
             (SELECT COUNT(*) FROM ssh_task WHERE id = ?2 AND deleted_at IS NOT NULL),
             (SELECT COUNT(*) FROM ssh_task_step WHERE task_id = ?2 AND deleted_at IS NOT NULL)"#,
    )
    .bind(&workspace_id)
    .bind(&created.task.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(tombstoned.0.is_some());
    assert_eq!(tombstoned.1, 1);
    assert_eq!(tombstoned.2, created.steps.len() as i64);
}
