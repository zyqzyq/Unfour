//! Explicit execution on an existing authenticated temporary target. No task
//! run, target binding, owner file or output log is persisted for this path.
use super::*;
use tokio::sync::watch;

pub(in crate::ssh) const TEMPORARY_TASK_MARKER: &str = "SSH_TASK_TEMPORARY_RUN";
const MAX_MEMORY_LOG: usize = 256 * 1024;

pub(in crate::ssh) struct MemoryTaskRecord {
    pub run: SshTaskRun,
    pub cancel: watch::Sender<bool>,
    pub log: String,
    truncated: bool,
}
impl MemoryTaskRecord {
    pub(in crate::ssh) fn append(&mut self, event: &SshTaskRunEvent) {
        if self.truncated {
            return;
        }
        let line = log_format::encode_event(event);
        let available = MAX_MEMORY_LOG.saturating_sub(self.log.len());
        if line.len() <= available {
            self.log.push_str(&line);
        } else {
            let mut end = available;
            while !line.is_char_boundary(end) {
                end -= 1;
            }
            self.log.push_str(&line[..end]);
            self.log.push_str("\n[temporary output truncated]\n");
            self.truncated = true;
        }
    }
}

impl SshService {
    pub async fn run_temporary_task(
        &self,
        input: SshTaskRunInput,
        handle: &str,
    ) -> AppResult<SshTaskRun> {
        self.run_temporary_task_with_mode(input, handle, "exec")
            .await
    }

    pub async fn run_temporary_task_with_mode(
        &self,
        input: SshTaskRunInput,
        handle: &str,
        mode: &str,
    ) -> AppResult<SshTaskRun> {
        if !matches!(mode, "exec" | "terminal") {
            return Err(AppError::Validation(
                "SSH_TASK_EXECUTION_MODE_INVALID".into(),
            ));
        }
        validate_workspace_id(&input.workspace_id)?;
        if input.connection_id.is_some() {
            return Err(AppError::Validation(
                "SSH_TASK_TEMPORARY_TARGET_CONFLICT".into(),
            ));
        }
        let detail = self.get_task(&input.workspace_id, &input.task_id).await?;
        let mut secrets = task_secret_values(&input.inputs, &input.secret_input_names)?;
        secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
        // Preserve revision/input validation, but never retain a recall context.
        let _ = context::run_context(&input, &detail, &secrets)?;
        let steps = resolve_enabled_steps(&detail.steps, &input.inputs)?;
        if steps.is_empty() {
            return Err(AppError::Validation("SSH task has no enabled steps".into()));
        }
        if mode == "terminal" && steps.iter().any(|step| step.step_type != "command") {
            return Err(AppError::Validation(
                "SSH_TASK_TERMINAL_COMMAND_ONLY".into(),
            ));
        }
        validate_task_transfer_paths(&steps)?;
        let run = SshTaskRun {
            legacy_recovery_available: false,
            context: None,
            warnings: if mode == "terminal" {
                vec![TEMPORARY_TASK_MARKER.into(), "SSH_TASK_TERMINAL_RUN".into()]
            } else {
                vec![TEMPORARY_TASK_MARKER.into()]
            },
            id: unfour_core::id::new_id(),
            workspace_id: input.workspace_id.clone(),
            task_id: detail.task.id,
            connection_id: None,
            status: "running".into(),
            started_at: Utc::now().to_rfc3339(),
            finished_at: None,
            error_message: None,
            log_path: String::new(),
        };
        let (cancel, cancel_rx) = watch::channel(false);
        let (source, terminal) = self.register_temporary_task(
            &input.workspace_id,
            handle,
            MemoryTaskRecord {
                run: run.clone(),
                cancel: cancel.clone(),
                log: log_format::HEADER.to_owned(),
                truncated: false,
            },
            mode == "terminal",
        )?;
        self.observe_temporary_task(
            &run.workspace_id,
            &run_event(&run.id, &run.task_id, "running", None),
        );
        let service = self.clone();
        let job = run.clone();
        tokio::spawn(async move {
            let mut parent = source.parent.clone();
            let (finished, mut done) = watch::channel(false);
            let monitor = tokio::spawn(async move {
                tokio::select! {
                    _ = parent.wait_for(|closed| *closed) => { cancel.send_replace(true); },
                    _ = done.changed() => {},
                }
            });
            let mut cancel_rx = cancel_rx;
            let mut emit = |event| {
                let event = task_run_event_from_runner(&job.id, &job.task_id, event, &secrets);
                service.observe_temporary_task(&job.workspace_id, &event);
            };
            let detail_step_for_recovery = steps.first().cloned();
            let outcome = if let Some(lease) = terminal {
                let mut driver = super::terminal::TerminalTaskDriver::new(lease);
                let mut outcome =
                    execute_serial(steps, &mut driver, &mut cancel_rx, &mut emit).await;
                if !driver.finish().await {
                    if let Some(step) = detail_step_for_recovery.as_ref() {
                        emit(RunnerEvent::Driver(
                            step.clone(),
                            DriverEvent::Output {
                                stream: "diagnostic".into(),
                                data: "SSH_TASK_TERMINAL_RECOVERY_REQUIRED\n".into(),
                            },
                        ));
                    }
                    if outcome.error.is_none() {
                        if outcome.status != "cancelled" {
                            outcome.status = "failed".into();
                        }
                        outcome.error = Some("SSH_TASK_TERMINAL_RECOVERY_REQUIRED".into());
                    }
                }
                outcome
            } else {
                let mut driver = NativeTaskDriver::shared(source);
                let outcome = execute_serial(steps, &mut driver, &mut cancel_rx, &mut emit).await;
                driver.disconnect().await;
                outcome
            };
            finished.send_replace(true);
            let _ = monitor.await;
            let outcome = redact_task_run_outcome(outcome, &secrets);
            service.observe_temporary_task(
                &job.workspace_id,
                &run_event(&job.id, &job.task_id, &outcome.status, outcome.error),
            );
        });
        Ok(run)
    }
}

pub(super) fn validate_task_transfer_paths(steps: &[SshTaskStep]) -> AppResult<()> {
    let mut downloaded = std::collections::HashSet::new();
    for step in steps {
        if matches!(step.step_type.as_str(), "upload" | "download") {
            let path = step.config_json["localPath"].as_str().unwrap_or_default();
            let resolved_path = std::env::current_dir()?.join(path);
            let result = unfour_core::local_path::check_runtime(path, step.step_type == "download");
            let result = if result == Err("pathMissing") && downloaded.contains(&resolved_path) {
                Ok(())
            } else {
                result
            };
            result.map_err(|code| {
                AppError::Validation(format!("LOCAL_PATH_UNAVAILABLE:{code}:{}", step.name))
            })?;
            if step.step_type == "download" {
                downloaded.insert(resolved_path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
