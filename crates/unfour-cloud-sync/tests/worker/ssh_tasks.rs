//! Routine SSH Task upload, edits, external application and workspace deletion.

use super::support::*;
use unfour_core::models::{SshConnectionInput, SshTaskSaveInput, SshTaskStepInput};

#[path = "ssh_tasks/bootstrap.rs"]
mod bootstrap;
#[path = "ssh_tasks/conflicts.rs"]
mod conflicts;
#[path = "ssh_tasks/failures.rs"]
mod failures;

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
