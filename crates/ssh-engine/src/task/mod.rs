use super::*;
#[cfg(feature = "ssh-native")]
use unfour_core::models::SshTaskRunEvent;
use unfour_core::models::{
    SshTask, SshTaskCancelInput, SshTaskCleanupInput, SshTaskCleanupResult, SshTaskCommandConfig,
    SshTaskDetail, SshTaskDownloadConfig, SshTaskLocalBinding, SshTaskRun, SshTaskRunInput,
    SshTaskSaveInput, SshTaskStep, SshTaskStepInput, SshTaskUploadConfig,
};

#[cfg(any(feature = "ssh-native", test))]
mod command_step;
#[cfg(feature = "ssh-native")]
mod download_step;
#[cfg(feature = "ssh-native")]
mod events;
#[cfg(feature = "ssh-native")]
mod native;
#[cfg(any(feature = "ssh-native", test))]
mod runner;
mod storage;
mod template;
#[cfg(feature = "ssh-native")]
mod upload_step;

#[cfg(feature = "ssh-native")]
use events::*;
#[cfg(feature = "ssh-native")]
use native::*;
#[cfg(feature = "ssh-native")]
use runner::*;
use template::*;
#[cfg(feature = "ssh-native")]
use upload_step::{io_step_error, sftp_step_error};

#[cfg(feature = "ssh-native")]
pub(super) struct TaskRunRuntime {
    workspace_id: String,
    cancel_tx: tokio::sync::watch::Sender<bool>,
}

impl SshService {
    pub async fn run_task(&self, input: SshTaskRunInput) -> AppResult<SshTaskRun> {
        validate_workspace_id(&input.workspace_id)?;
        let detail = self.get_task(&input.workspace_id, &input.task_id).await?;
        let connection_id = input
            .connection_id
            .or_else(|| {
                detail
                    .local_binding
                    .as_ref()
                    .and_then(|binding| binding.default_connection_id.clone())
            })
            .or_else(|| {
                detail
                    .local_binding
                    .as_ref()
                    .and_then(|binding| binding.last_used_connection_id.clone())
            })
            .ok_or_else(|| {
                AppError::Validation("SSH task run requires a connection".to_string())
            })?;
        let connection = self
            .get_connection(&input.workspace_id, &connection_id)
            .await?;
        let steps = resolve_enabled_steps(&detail.steps, &input.inputs)?;
        if steps.is_empty() {
            return Err(AppError::Validation(
                "SSH task has no enabled steps".to_string(),
            ));
        }

        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (connection, steps);
            return Err(AppError::Unsupported(
                "SSH task execution requires a build with the ssh-native feature".to_string(),
            ));
        }

        #[cfg(feature = "ssh-native")]
        {
            self.record_task_connection_use(&input.workspace_id, &detail.task.id, &connection_id)
                .await?;
            let run_id = unfour_core::id::new_id();
            let log_path = self.task_log_path(&run_id)?;
            let mut log = TaskLogWriter::create(&log_path)?;
            let run = SshTaskRun {
                id: run_id.clone(),
                workspace_id: input.workspace_id.clone(),
                task_id: detail.task.id.clone(),
                connection_id: Some(connection_id),
                status: "running".to_string(),
                started_at: Utc::now().to_rfc3339(),
                finished_at: None,
                error_message: None,
                log_path: log_path.to_string_lossy().to_string(),
            };
            self.insert_task_run(&run).await?;
            let started_event = run_event(&run.id, &run.task_id, "running", None);
            log.write_event(&started_event);
            self.emit_task_run_event(&started_event);

            let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
            self.task_runs
                .lock()
                .map_err(|_| AppError::Config("SSH task run lock poisoned".to_string()))?
                .insert(
                    run_id.clone(),
                    TaskRunRuntime {
                        workspace_id: input.workspace_id.clone(),
                        cancel_tx,
                    },
                );

            let service = self.clone();
            let run_for_task = run.clone();
            tokio::spawn(async move {
                service
                    .execute_task_background(run_for_task, connection, steps, cancel_rx, log)
                    .await;
            });
            Ok(run)
        }
    }

    pub async fn cancel_task_run(&self, input: SshTaskCancelInput) -> AppResult<SshTaskRun> {
        validate_workspace_id(&input.workspace_id)?;
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = input;
            return Err(AppError::Unsupported(
                "SSH task execution requires a build with the ssh-native feature".to_string(),
            ));
        }
        #[cfg(feature = "ssh-native")]
        {
            let cancel_tx = {
                let runs = self
                    .task_runs
                    .lock()
                    .map_err(|_| AppError::Config("SSH task run lock poisoned".to_string()))?;
                runs.get(&input.run_id)
                    .filter(|runtime| runtime.workspace_id == input.workspace_id)
                    .map(|runtime| runtime.cancel_tx.clone())
                    .ok_or_else(|| AppError::NotFound("running SSH task".to_string()))?
            };
            let _ = cancel_tx.send(true);
            self.get_task_run(&input.workspace_id, &input.run_id).await
        }
    }

    #[cfg(feature = "ssh-native")]
    async fn execute_task_background(
        &self,
        run: SshTaskRun,
        connection: SshConnection,
        steps: Vec<SshTaskStep>,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
        mut log: TaskLogWriter,
    ) {
        let outcome = match NativeTaskDriver::connect(self, &connection).await {
            Ok(mut driver) => {
                let run_id = run.id.clone();
                let task_id = run.task_id.clone();
                let outcome = execute_serial(steps, &mut driver, &mut cancel_rx, |runner_event| {
                    let event = match runner_event {
                        RunnerEvent::StepStarted(step) => {
                            step_event(&run_id, &task_id, &step, "running", None, None, None)
                        }
                        RunnerEvent::Driver(step, DriverEvent::Output { stream, data }) => {
                            output_event(&run_id, &task_id, &step, &stream, data)
                        }
                        RunnerEvent::Driver(step, DriverEvent::Transfer(progress)) => {
                            transfer_event(&run_id, &task_id, &step, &progress)
                        }
                        RunnerEvent::StepFinished {
                            step,
                            status,
                            duration_ms,
                            exit_code,
                            error,
                        } => step_event(
                            &run_id,
                            &task_id,
                            &step,
                            &status,
                            Some(duration_ms),
                            exit_code,
                            error,
                        ),
                    };
                    log.write_event(&event);
                    self.emit_task_run_event(&event);
                })
                .await;
                driver.disconnect().await;
                outcome
            }
            Err(TaskStepError::Cancelled) => TaskRunOutcome {
                status: "cancelled".to_string(),
                error: None,
            },
            Err(TaskStepError::Failed { message, .. }) => TaskRunOutcome {
                status: "failed".to_string(),
                error: Some(message),
            },
        };

        let final_event = run_event(
            &run.id,
            &run.task_id,
            &outcome.status,
            outcome.error.clone(),
        );
        log.write_event(&final_event);
        let _ = self
            .finish_task_run(
                &run.workspace_id,
                &run.id,
                &outcome.status,
                outcome.error.as_deref(),
            )
            .await;
        self.emit_task_run_event(&final_event);
        if let Ok(mut runs) = self.task_runs.lock() {
            runs.remove(&run.id);
        }
        let _ = self
            .cleanup_task_retention(&run.workspace_id, &run.task_id)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn service() -> (SshService, String) {
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

    fn docker_export_input(workspace_id: String) -> SshTaskSaveInput {
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

    fn edit_input(detail: &SshTaskDetail) -> SshTaskSaveInput {
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

    async fn connection(service: &SshService, workspace_id: &str) -> SshConnection {
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

    #[tokio::test]
    async fn task_crud_persists_ordered_steps_without_parameter_records() {
        let (service, workspace_id) = service().await;
        let saved = service
            .save_task(docker_export_input(workspace_id.clone()))
            .await
            .unwrap();
        for id in [&workspace_id, &saved.task.id] {
            let parsed = uuid::Uuid::parse_str(id).unwrap();
            assert_eq!(parsed.get_version_num(), 7);
        }
        assert_eq!(saved.steps.len(), 5);
        assert_eq!(saved.steps[3].step_type, "download");
        assert!(saved.steps.iter().all(|step| {
            uuid::Uuid::parse_str(&step.id).is_ok_and(|id| id.get_version_num() == 7)
                && step.config_version == 1
                && step.config_json.get("version").is_none()
        }));
        assert_eq!(
            detected_inputs(&saved.steps).unwrap(),
            vec![
                "source_image",
                "target_image",
                "archive_name",
                "local_output_dir"
            ]
        );

        let copy = service
            .duplicate_task(workspace_id.clone(), saved.task.id.clone())
            .await
            .unwrap();
        assert_ne!(copy.task.id, saved.task.id);
        assert_eq!(
            service
                .list_tasks(workspace_id.clone())
                .await
                .unwrap()
                .len(),
            2
        );

        service
            .delete_task(workspace_id.clone(), saved.task.id)
            .await
            .unwrap();
        assert_eq!(service.list_tasks(workspace_id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn task_delete_soft_deletes_templates_and_removes_local_state() {
        let (service, workspace_id) = service().await;
        let connection = connection(&service, &workspace_id).await;
        let mut input = docker_export_input(workspace_id.clone());
        input.default_connection_id = Some(connection.id.clone());
        let saved = service.save_task(input).await.unwrap();
        assert_eq!(
            saved
                .local_binding
                .as_ref()
                .and_then(|binding| binding.default_connection_id.as_deref()),
            Some(connection.id.as_str())
        );

        let removed_step_id = saved.steps[0].id.clone();
        let mut update = edit_input(&saved);
        update.steps.remove(0);
        let updated = service.save_task(update).await.unwrap();
        assert_eq!(updated.steps.len(), 4);
        let deleted_at: Option<String> = sqlx::query_scalar(
            "SELECT deleted_at FROM ssh_task_step WHERE workspace_id = ?1 AND id = ?2",
        )
        .bind(&workspace_id)
        .bind(&removed_step_id)
        .fetch_one(service.db.pool())
        .await
        .unwrap();
        assert!(deleted_at.is_some());

        let run_id = unfour_core::id::new_id();
        std::fs::create_dir_all(&*service.task_log_dir).unwrap();
        let log_path = service.task_log_dir.join(format!("{run_id}.log"));
        std::fs::write(&log_path, "local task output").unwrap();
        sqlx::query(
            r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, status, started_at, finished_at, log_path
            ) VALUES (?1, ?2, ?3, 'success', ?4, ?4, ?5)
            "#,
        )
        .bind(&run_id)
        .bind(&workspace_id)
        .bind(&saved.task.id)
        .bind(Utc::now().to_rfc3339())
        .bind(log_path.to_string_lossy().to_string())
        .execute(service.db.pool())
        .await
        .unwrap();

        service
            .delete_task(workspace_id.clone(), saved.task.id.clone())
            .await
            .unwrap();
        assert!(service
            .get_task(&workspace_id, &saved.task.id)
            .await
            .is_err());
        let active_steps: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ssh_task_step WHERE task_id = ?1 AND deleted_at IS NULL",
        )
        .bind(&saved.task.id)
        .fetch_one(service.db.pool())
        .await
        .unwrap();
        let bindings: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_local_binding WHERE task_id = ?1")
                .bind(&saved.task.id)
                .fetch_one(service.db.pool())
                .await
                .unwrap();
        let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_run WHERE task_id = ?1")
            .bind(&saved.task.id)
            .fetch_one(service.db.pool())
            .await
            .unwrap();
        assert_eq!(active_steps, 0);
        assert_eq!(bindings, 0);
        assert_eq!(runs, 0);
        assert!(!log_path.exists());

        let insert_error = sqlx::query(
            r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, status, started_at, finished_at, log_path
            ) VALUES (?1, ?2, ?3, 'success', ?4, ?4, ?5)
            "#,
        )
        .bind(unfour_core::id::new_id())
        .bind(&workspace_id)
        .bind(&saved.task.id)
        .bind(Utc::now().to_rfc3339())
        .bind(log_path.to_string_lossy().to_string())
        .execute(service.db.pool())
        .await
        .unwrap_err();
        assert!(insert_error
            .to_string()
            .contains("must reference an active task"));
        let _ = std::fs::remove_dir(&*service.task_log_dir);
    }

    #[tokio::test]
    async fn local_binding_is_optional_and_tracks_default_and_last_used_connections() {
        let (service, workspace_id) = service().await;
        let saved = service
            .save_task(docker_export_input(workspace_id.clone()))
            .await
            .unwrap();
        assert!(saved.local_binding.is_none());
        let error = service
            .run_task(SshTaskRunInput {
                workspace_id: workspace_id.clone(),
                task_id: saved.task.id.clone(),
                connection_id: None,
                inputs: std::collections::BTreeMap::new(),
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("requires a connection"));

        let default = connection(&service, &workspace_id).await;
        let mut update = edit_input(&saved);
        update.default_connection_id = Some(default.id.clone());
        let updated = service.save_task(update).await.unwrap();
        let binding = updated.local_binding.unwrap();
        assert_eq!(
            binding.default_connection_id.as_deref(),
            Some(default.id.as_str())
        );
        assert!(binding.last_used_connection_id.is_none());
        let task_json = serde_json::to_value(&updated.task).unwrap();
        assert!(task_json.get("defaultConnectionId").is_none());
        let task_columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('ssh_task') ORDER BY cid")
                .fetch_all(service.db.pool())
                .await
                .unwrap();
        assert!(!task_columns
            .iter()
            .any(|name| name == "default_connection_id"));

        let last_used = connection(&service, &workspace_id).await;
        service
            .record_task_connection_use(&workspace_id, &saved.task.id, &last_used.id)
            .await
            .unwrap();
        let binding = service
            .get_task(&workspace_id, &saved.task.id)
            .await
            .unwrap()
            .local_binding
            .unwrap();
        assert_eq!(
            binding.default_connection_id.as_deref(),
            Some(default.id.as_str())
        );
        assert_eq!(
            binding.last_used_connection_id.as_deref(),
            Some(last_used.id.as_str())
        );
    }

    #[tokio::test]
    async fn ordinary_step_updates_preserve_config_version_and_unknown_versions_fail() {
        let (service, workspace_id) = service().await;
        let saved = service
            .save_task(docker_export_input(workspace_id.clone()))
            .await
            .unwrap();
        let mut update = edit_input(&saved);
        update.steps[0].config_version = None;
        update.steps[0].config_json["command"] =
            serde_json::json!("docker pull --quiet {{source_image}}");
        let updated = service.save_task(update).await.unwrap();
        assert_eq!(updated.steps[0].config_version, 1);

        let mut invalid = edit_input(&updated);
        invalid.steps.push(SshTaskStepInput {
            id: None,
            name: "Future command".to_string(),
            step_type: "command".to_string(),
            position: invalid.steps.len() as i64,
            enabled: true,
            config_version: Some(99),
            config_json: serde_json::json!({
                "command": "true",
                "workingDirectory": "",
                "timeoutSeconds": 30,
                "continueOnError": false
            }),
        });
        let error = service.save_task(invalid).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported SSH task command config version: 99"));
    }

    #[tokio::test]
    async fn task_runs_remain_local_and_can_be_physically_cleared() {
        let (service, workspace_id) = service().await;
        let saved = service
            .save_task(docker_export_input(workspace_id.clone()))
            .await
            .unwrap();
        let run_id = unfour_core::id::new_id();
        sqlx::query(
            r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, status, started_at, log_path
            ) VALUES (?1, ?2, ?3, 'success', ?4, ?5)
            "#,
        )
        .bind(&run_id)
        .bind(&workspace_id)
        .bind(&saved.task.id)
        .bind(Utc::now().to_rfc3339())
        .bind(format!("{run_id}.log"))
        .execute(service.db.pool())
        .await
        .unwrap();
        assert_eq!(uuid::Uuid::parse_str(&run_id).unwrap().get_version_num(), 7);

        let result = service
            .clear_task_runs(SshTaskCleanupInput {
                workspace_id: workspace_id.clone(),
                task_id: Some(saved.task.id),
            })
            .await
            .unwrap();
        assert_eq!(result.deleted_runs, 1);
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1")
                .bind(workspace_id)
                .fetch_one(service.db.pool())
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }
}
