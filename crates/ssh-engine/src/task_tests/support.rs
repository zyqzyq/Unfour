use super::super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub(super) async fn service() -> (SshService, String) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let db = LocalDb::from_pool(pool);
    db.migrate().await.unwrap();
    let workspace_id = unfour_core::id::new_id();
    sqlx::query(
        r#"
            INSERT INTO workspaces (
              id, name, is_default, created_at, updated_at, revision, sync_status
            ) VALUES (?1, 'Tasks', 1, ?2, ?2, 1, 'local')
            "#,
    )
    .bind(&workspace_id)
    .bind(Utc::now().to_rfc3339())
    .execute(db.pool())
    .await
    .unwrap();
    let task_log_dir = std::env::temp_dir().join(format!(
        "unfour-ssh-task-tests-{}",
        unfour_core::id::new_id()
    ));
    (
        SshService::new(db, SecretStore::in_memory("ssh-task-test"))
            .with_task_log_dir(task_log_dir),
        workspace_id,
    )
}

pub(super) fn docker_export_input(workspace_id: String) -> SshTaskSaveInput {
    let commands = [
        ("Pull image", "docker pull {{source_image}}"),
        ("Tag image", "docker tag {{source_image}} {{target_image}}"),
        (
            "Save image",
            "docker save {{target_image}} -o /tmp/{{archive_name}}.tar",
        ),
    ];
    let mut steps = commands
        .into_iter()
        .enumerate()
        .map(|(position, (name, command))| SshTaskStepInput {
            id: None,
            name: name.to_string(),
            step_type: "command".to_string(),
            position: position as i64,
            enabled: true,
            config_version: None,
            config_json: serde_json::json!({
                "command": command,
                "workingDirectory": "",
                "timeoutSeconds": 300,
                "continueOnError": false
            }),
        })
        .collect::<Vec<_>>();
    steps.push(SshTaskStepInput {
        id: None,
        name: "Download archive".to_string(),
        step_type: "download".to_string(),
        position: 3,
        enabled: true,
        config_version: None,
        config_json: serde_json::json!({
            "remotePath": "/tmp/{{archive_name}}.tar",
            "localPath": "{{local_output_dir}}/{{archive_name}}.tar",
            "overwrite": true
        }),
    });
    steps.push(SshTaskStepInput {
        id: None,
        name: "Remove remote archive".to_string(),
        step_type: "command".to_string(),
        position: 4,
        enabled: true,
        config_version: None,
        config_json: serde_json::json!({
            "command": "rm -f /tmp/{{archive_name}}.tar",
            "workingDirectory": "",
            "timeoutSeconds": 300,
            "continueOnError": false
        }),
    });
    SshTaskSaveInput {
        id: None,
        workspace_id,
        name: "Docker Image Export".to_string(),
        description: "Export a Docker image for offline use".to_string(),
        default_connection_id: None,
        steps,
    }
}

pub(super) fn edit_input(detail: &SshTaskDetail) -> SshTaskSaveInput {
    SshTaskSaveInput {
        id: Some(detail.task.id.clone()),
        workspace_id: detail.task.workspace_id.clone(),
        name: detail.task.name.clone(),
        description: detail.task.description.clone(),
        default_connection_id: detail
            .local_binding
            .as_ref()
            .and_then(|binding| binding.default_connection_id.clone()),
        steps: detail
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
    }
}

pub(super) async fn connection(service: &SshService, workspace_id: &str) -> SshConnection {
    service
        .save_connection(unfour_core::models::SshConnectionInput {
            id: None,
            workspace_id: workspace_id.to_string(),
            name: "Task host".to_string(),
            host: "127.0.0.1".to_string(),
            port: Some(22),
            username: "tester".to_string(),
            auth_kind: "none".to_string(),
            key_path: None,
            credential_ref: None,
            secret: None,
        })
        .await
        .unwrap()
}
