use super::*;

#[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TaskTransferProgress {
    pub direction: String,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub bytes_per_second: u64,
}

#[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DriverEvent {
    Output { stream: String, data: String },
    Transfer(TaskTransferProgress),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TaskStepResult {
    pub exit_code: Option<i32>,
}

#[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TaskStepError {
    Failed {
        message: String,
        exit_code: Option<i32>,
    },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RunnerEvent {
    StepStarted(SshTaskStep),
    Driver(SshTaskStep, DriverEvent),
    StepFinished {
        step: SshTaskStep,
        status: String,
        duration_ms: u64,
        exit_code: Option<i32>,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TaskRunOutcome {
    pub status: String,
    pub error: Option<String>,
}

#[allow(async_fn_in_trait)]
pub(super) trait TaskStepDriver {
    async fn execute_step(
        &mut self,
        step: &SshTaskStep,
        cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) -> Result<TaskStepResult, TaskStepError>;
}

pub(super) async fn execute_serial<D, E>(
    steps: Vec<SshTaskStep>,
    driver: &mut D,
    cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
    mut emit: E,
) -> TaskRunOutcome
where
    D: TaskStepDriver,
    E: FnMut(RunnerEvent) + Send,
{
    for step in steps {
        if *cancel_rx.borrow() {
            return TaskRunOutcome {
                status: "cancelled".to_string(),
                error: None,
            };
        }
        emit(RunnerEvent::StepStarted(step.clone()));
        let started = std::time::Instant::now();
        let mut driver_emit = |event| emit(RunnerEvent::Driver(step.clone(), event));
        let result = driver
            .execute_step(&step, cancel_rx, &mut driver_emit)
            .await;
        let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        match result {
            Ok(result) => {
                emit(RunnerEvent::StepFinished {
                    step,
                    status: "success".to_string(),
                    duration_ms,
                    exit_code: result.exit_code,
                    error: None,
                });
            }
            Err(TaskStepError::Cancelled) => {
                emit(RunnerEvent::StepFinished {
                    step,
                    status: "cancelled".to_string(),
                    duration_ms,
                    exit_code: None,
                    error: None,
                });
                return TaskRunOutcome {
                    status: "cancelled".to_string(),
                    error: None,
                };
            }
            Err(TaskStepError::Failed { message, exit_code }) => {
                let continue_on_error = step_continue_on_error(&step);
                emit(RunnerEvent::StepFinished {
                    step: step.clone(),
                    status: "failed".to_string(),
                    duration_ms,
                    exit_code,
                    error: Some(message.clone()),
                });
                if !continue_on_error {
                    return TaskRunOutcome {
                        status: "failed".to_string(),
                        error: Some(message),
                    };
                }
            }
        }
    }
    TaskRunOutcome {
        status: "success".to_string(),
        error: None,
    }
}

fn step_continue_on_error(step: &SshTaskStep) -> bool {
    if step.step_type != "command" {
        return false;
    }
    parse_command_config(step.config_version, &step.config_json)
        .map(|config| config.continue_on_error)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct FakeDriver {
        calls: Arc<Mutex<Vec<String>>>,
        wait_on: Option<String>,
    }

    impl TaskStepDriver for FakeDriver {
        async fn execute_step(
            &mut self,
            step: &SshTaskStep,
            cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
            _emit: &mut (dyn FnMut(DriverEvent) + Send),
        ) -> Result<TaskStepResult, TaskStepError> {
            self.calls.lock().unwrap().push(step.name.clone());
            if self.wait_on.as_deref() == Some(&step.name) {
                tokio::select! {
                    _ = cancel_rx.changed() => Err(TaskStepError::Cancelled),
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                        Ok(TaskStepResult { exit_code: Some(0) })
                    }
                }
            } else {
                Ok(TaskStepResult { exit_code: Some(0) })
            }
        }
    }

    fn step(name: &str, position: i64) -> SshTaskStep {
        SshTaskStep {
            id: format!("step-{position}"),
            workspace_id: "workspace".to_string(),
            task_id: "task".to_string(),
            name: name.to_string(),
            step_type: "command".to_string(),
            position,
            enabled: true,
            config_version: 1,
            config_json: serde_json::json!({
                "command": "true",
                "workingDirectory": "",
                "timeoutSeconds": 30,
                "continueOnError": false
            }),
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        }
    }

    #[tokio::test]
    async fn executes_steps_strictly_in_position_order() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut driver = FakeDriver {
            calls: calls.clone(),
            wait_on: None,
        };
        let (_cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
        let outcome = execute_serial(
            vec![step("Pull", 0), step("Tag", 1), step("Save", 2)],
            &mut driver,
            &mut cancel_rx,
            |_| {},
        )
        .await;

        assert_eq!(outcome.status, "success");
        assert_eq!(&*calls.lock().unwrap(), &["Pull", "Tag", "Save"]);
    }

    #[tokio::test]
    async fn cancellation_stops_current_and_all_following_steps() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut driver = FakeDriver {
            calls: calls.clone(),
            wait_on: Some("Long running".to_string()),
        };
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            let _ = cancel_tx.send(true);
        });

        let outcome = execute_serial(
            vec![step("Long running", 0), step("Must not run", 1)],
            &mut driver,
            &mut cancel_rx,
            |_| {},
        )
        .await;

        assert_eq!(outcome.status, "cancelled");
        assert_eq!(&*calls.lock().unwrap(), &["Long running"]);
    }
}
