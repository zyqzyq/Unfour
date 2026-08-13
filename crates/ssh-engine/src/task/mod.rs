use super::*;
#[cfg(any(feature = "ssh-native", test))]
use unfour_core::models::SshTaskRunEvent;
use unfour_core::models::{
    SshTask, SshTaskCancelInput, SshTaskCleanupInput, SshTaskCleanupResult, SshTaskCommandConfig,
    SshTaskDetail, SshTaskDownloadConfig, SshTaskLocalBinding, SshTaskRun, SshTaskRunInput,
    SshTaskSaveInput, SshTaskStep, SshTaskStepInput, SshTaskUploadConfig, SshTasksReorderInput,
};

#[cfg(any(feature = "ssh-native", test))]
mod command_step;
#[cfg(feature = "ssh-native")]
mod download_step;
#[cfg(any(feature = "ssh-native", test))]
mod events;
#[cfg(feature = "ssh-native")]
mod native;
#[cfg(any(feature = "ssh-native", test))]
mod runner;
mod storage;
mod template;
#[cfg(feature = "ssh-native")]
mod upload_step;

#[cfg(any(feature = "ssh-native", test))]
use events::*;
#[cfg(feature = "ssh-native")]
use native::*;
#[cfg(any(feature = "ssh-native", test))]
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
        let secret_values = task_secret_values(&input.inputs, &input.secret_input_names)?;
        if steps.is_empty() {
            return Err(AppError::Validation(
                "SSH task has no enabled steps".to_string(),
            ));
        }

        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (connection, steps, secret_values);
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
                    .execute_task_background(
                        run_for_task,
                        connection,
                        steps,
                        secret_values,
                        cancel_rx,
                        log,
                    )
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
        secret_values: Vec<String>,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
        mut log: TaskLogWriter,
    ) {
        let outcome = match NativeTaskDriver::connect(self, &connection).await {
            Ok(mut driver) => {
                let run_id = run.id.clone();
                let task_id = run.task_id.clone();
                let outcome = execute_serial(steps, &mut driver, &mut cancel_rx, |runner_event| {
                    let event =
                        task_run_event_from_runner(&run_id, &task_id, runner_event, &secret_values);
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

        let outcome = redact_task_run_outcome(outcome, &secret_values);
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

#[cfg(any(feature = "ssh-native", test))]
fn task_run_event_from_runner(
    run_id: &str,
    task_id: &str,
    runner_event: RunnerEvent,
    secret_values: &[String],
) -> SshTaskRunEvent {
    match runner_event {
        RunnerEvent::StepStarted(step) => {
            step_event(run_id, task_id, &step, "running", None, None, None)
        }
        RunnerEvent::Driver(step, DriverEvent::Output { stream, data }) => output_event(
            run_id,
            task_id,
            &step,
            &stream,
            redact_task_secret_values(&data, secret_values),
        ),
        RunnerEvent::Driver(step, DriverEvent::Transfer(progress)) => {
            transfer_event(run_id, task_id, &step, &progress)
        }
        RunnerEvent::StepFinished {
            step,
            status,
            duration_ms,
            exit_code,
            error,
        } => step_event(
            run_id,
            task_id,
            &step,
            &status,
            Some(duration_ms),
            exit_code,
            error.map(|value| redact_task_secret_values(&value, secret_values)),
        ),
    }
}

#[cfg(any(feature = "ssh-native", test))]
fn redact_task_run_outcome(outcome: TaskRunOutcome, secret_values: &[String]) -> TaskRunOutcome {
    TaskRunOutcome {
        status: outcome.status,
        error: outcome
            .error
            .map(|value| redact_task_secret_values(&value, secret_values)),
    }
}

#[cfg(test)]
#[path = "../task_tests/mod.rs"]
mod task_tests;
