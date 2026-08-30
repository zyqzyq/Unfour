use super::*;

#[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
const TRANSFER_PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

#[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
pub(super) struct TransferProgressThrottle {
    last_emitted_at: Option<std::time::Instant>,
}

#[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
impl TransferProgressThrottle {
    pub(super) fn new() -> Self {
        Self {
            last_emitted_at: None,
        }
    }

    pub(super) fn should_emit(&mut self, now: std::time::Instant) -> bool {
        let due = self
            .last_emitted_at
            .map(|last| now.duration_since(last) >= TRANSFER_PROGRESS_INTERVAL)
            .unwrap_or(true);
        if due {
            self.last_emitted_at = Some(now);
        }
        due
    }
}

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

    #[test]
    fn throttles_intermediate_transfer_progress() {
        let started = std::time::Instant::now();
        let mut throttle = TransferProgressThrottle::new();

        assert!(throttle.should_emit(started));
        assert!(!throttle.should_emit(started + std::time::Duration::from_millis(99)));
        assert!(throttle.should_emit(started + std::time::Duration::from_millis(100)));
    }

    #[derive(Default)]
    struct FakeDriver {
        calls: Arc<Mutex<Vec<String>>>,
        wait_on: Option<String>,
        fail_on: Option<String>,
        entered: Option<Arc<tokio::sync::Notify>>,
    }

    impl TaskStepDriver for FakeDriver {
        async fn execute_step(
            &mut self,
            step: &SshTaskStep,
            cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
            _emit: &mut (dyn FnMut(DriverEvent) + Send),
        ) -> Result<TaskStepResult, TaskStepError> {
            self.calls.lock().unwrap().push(step.name.clone());
            if self.fail_on.as_deref() == Some(&step.name) {
                return Err(TaskStepError::Failed {
                    message: "step failed".into(),
                    exit_code: Some(42),
                });
            }
            if self.wait_on.as_deref() == Some(&step.name) {
                if let Some(entered) = &self.entered {
                    entered.notify_one();
                }
                cancel_rx
                    .changed()
                    .await
                    .expect("cancellation sender remains alive");
                Err(TaskStepError::Cancelled)
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
            ..Default::default()
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
        let entered = Arc::new(tokio::sync::Notify::new());
        let mut driver = FakeDriver {
            calls: calls.clone(),
            wait_on: Some("Long running".to_string()),
            entered: Some(entered.clone()),
            ..Default::default()
        };
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            entered.notified().await;
            let _ = cancel_tx.send(true);
        });

        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            execute_serial(
                vec![step("Long running", 0), step("Must not run", 1)],
                &mut driver,
                &mut cancel_rx,
                |_| {},
            ),
        )
        .await
        .expect("cancelled step must finish without a timer race");

        assert_eq!(outcome.status, "cancelled");
        assert_eq!(&*calls.lock().unwrap(), &["Long running"]);
    }

    #[tokio::test]
    async fn failed_step_stops_following_work_unless_continue_on_error_is_explicit() {
        for continue_on_error in [false, true] {
            let mut driver = FakeDriver {
                fail_on: Some("Fail".into()),
                ..Default::default()
            };
            let mut failed = step("Fail", 0);
            failed.config_json["continueOnError"] = serde_json::json!(continue_on_error);
            let (_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
            let mut events = Vec::new();
            let outcome = execute_serial(
                vec![failed, step("Next", 1)],
                &mut driver,
                &mut cancel_rx,
                |event| events.push(event),
            )
            .await;
            assert_eq!(
                outcome.status,
                if continue_on_error {
                    "success"
                } else {
                    "failed"
                }
            );
            assert_eq!(
                outcome.error,
                if continue_on_error {
                    None
                } else {
                    Some("step failed".into())
                }
            );
            assert_eq!(
                *driver.calls.lock().unwrap(),
                if continue_on_error {
                    vec!["Fail", "Next"]
                } else {
                    vec!["Fail"]
                }
            );
            assert!(
                matches!(&events[1], RunnerEvent::StepFinished { status, exit_code: Some(42), error: Some(message), .. } if status == "failed" && message == "step failed")
            );
            assert_eq!(events.len(), if continue_on_error { 4 } else { 2 });
        }
    }

    #[tokio::test]
    async fn cancellation_before_run_has_no_driver_or_step_event_side_effects() {
        let mut driver = FakeDriver::default();
        let (_tx, mut cancel_rx) = tokio::sync::watch::channel(true);
        let mut events = Vec::new();
        let outcome = execute_serial(
            vec![step("Must not run", 0)],
            &mut driver,
            &mut cancel_rx,
            |event| events.push(event),
        )
        .await;
        assert_eq!(outcome.status, "cancelled");
        assert!(driver.calls.lock().unwrap().is_empty());
        assert!(events.is_empty());
    }
}
