#[cfg(feature = "ssh-native")]
use super::*;
#[cfg(not(feature = "ssh-native"))]
use super::{TaskStepError, TaskStepResult};

#[cfg(feature = "ssh-native")]
const TASK_COMMAND_MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[cfg(feature = "ssh-native")]
impl NativeTaskDriver {
    pub(super) async fn run_command_step(
        &mut self,
        step: &SshTaskStep,
        cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) -> Result<TaskStepResult, TaskStepError> {
        let config =
            parse_command_config(step.config_version, &step.config_json).map_err(|error| {
                TaskStepError::Failed {
                    message: error.to_string(),
                    exit_code: None,
                }
            })?;
        let command = if config.working_directory.trim().is_empty() {
            config.command
        } else {
            format!(
                "cd -- {} && {}",
                shell_quote(config.working_directory.trim()),
                config.command
            )
        };
        let command = validate_task_command(&command).map_err(|error| TaskStepError::Failed {
            message: error.to_string(),
            exit_code: None,
        })?;
        let mut channel =
            self.handle
                .channel_open_session()
                .await
                .map_err(|error| TaskStepError::Failed {
                    message: format!("open command channel failed: {error}"),
                    exit_code: None,
                })?;
        channel
            .exec(true, command.as_bytes())
            .await
            .map_err(|error| TaskStepError::Failed {
                message: format!("execute command failed: {error}"),
                exit_code: None,
            })?;

        let mut stdout = StreamBuffer::new();
        let mut stderr = StreamBuffer::new();
        let mut exit_code = None;
        let mut exit_signal = None;
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(
            config.timeout_seconds.clamp(1, 3_600),
        ));
        tokio::pin!(timeout);
        loop {
            tokio::select! {
                changed = cancel_rx.changed() => {
                    let _ = changed;
                    let _ = channel.close().await;
                    return Err(TaskStepError::Cancelled);
                }
                _ = &mut timeout => {
                    let _ = channel.close().await;
                    return Err(TaskStepError::Failed {
                        message: format!("Command timed out after {} seconds", config.timeout_seconds),
                        exit_code,
                    });
                }
                message = channel.wait() => {
                    match message {
                        Some(russh::ChannelMsg::Data { data }) => {
                            stdout.push(&data[..], "stdout", emit);
                        }
                        Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                            if ext == 1 {
                                stderr.push(&data[..], "stderr", emit);
                            } else {
                                stdout.push(&data[..], "stdout", emit);
                            }
                        }
                        Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                            exit_code = Some(exit_status as i32);
                        }
                        Some(russh::ChannelMsg::ExitSignal {
                            signal_name,
                            error_message,
                            ..
                        }) => {
                            exit_signal = Some(if error_message.trim().is_empty() {
                                format!("signal {signal_name:?}")
                            } else {
                                format!("signal {signal_name:?}: {error_message}")
                            });
                        }
                        // EOF only means stdout/stderr are done. ExitStatus often
                        // arrives afterwards; breaking here used to drop the code and
                        // mark failed docker/shell commands as success via unwrap_or(0).
                        Some(russh::ChannelMsg::Eof) => {}
                        Some(russh::ChannelMsg::Close) | None => break,
                        _ => {}
                    }
                }
            }
        }
        let _ = channel.close().await;
        stdout.flush("stdout", emit);
        stderr.flush("stderr", emit);
        if stdout.truncated || stderr.truncated {
            emit(DriverEvent::Output {
                stream: "stderr".to_string(),
                data: "\n[command output truncated]\n".to_string(),
            });
        }
        command_step_outcome(exit_code, exit_signal.as_deref())
    }
}

/// Decide command-step success from the SSH channel completion signals.
fn command_step_outcome(
    exit_code: Option<i32>,
    exit_signal: Option<&str>,
) -> Result<TaskStepResult, TaskStepError> {
    if let Some(signal) = exit_signal {
        return Err(TaskStepError::Failed {
            message: format!("Command terminated by {signal}"),
            exit_code,
        });
    }
    match exit_code {
        Some(0) => Ok(TaskStepResult {
            exit_code: Some(0),
        }),
        Some(code) => Err(TaskStepError::Failed {
            message: format!("Command exited with status {code}"),
            exit_code: Some(code),
        }),
        None => Err(TaskStepError::Failed {
            message: "Command finished without an exit status".to_string(),
            exit_code: None,
        }),
    }
}

#[cfg(feature = "ssh-native")]
struct StreamBuffer {
    total: usize,
    truncated: bool,
    pending: String,
}

#[cfg(feature = "ssh-native")]
impl StreamBuffer {
    fn new() -> Self {
        Self {
            total: 0,
            truncated: false,
            pending: String::new(),
        }
    }

    fn push(&mut self, data: &[u8], stream: &str, emit: &mut (dyn FnMut(DriverEvent) + Send)) {
        if self.truncated || data.is_empty() {
            return;
        }
        let remaining = TASK_COMMAND_MAX_OUTPUT_BYTES.saturating_sub(self.total);
        if remaining == 0 {
            self.truncated = true;
            return;
        }
        let take = remaining.min(data.len());
        if take < data.len() {
            self.truncated = true;
        }
        self.total += take;
        self.pending
            .push_str(&String::from_utf8_lossy(&data[..take]));
        while let Some(index) = self.pending.find('\n') {
            let line = self.pending.drain(..=index).collect::<String>();
            emit_redacted(stream, &line, emit);
        }
    }

    fn flush(&mut self, stream: &str, emit: &mut (dyn FnMut(DriverEvent) + Send)) {
        if self.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        emit_redacted(stream, &pending, emit);
    }
}

#[cfg(feature = "ssh-native")]
fn emit_redacted(stream: &str, data: &str, emit: &mut (dyn FnMut(DriverEvent) + Send)) {
    if data.is_empty() {
        return;
    }
    let (redacted, _) = unfour_core::redaction::redact_sensitive_lines(data);
    if redacted.is_empty() {
        return;
    }
    emit(DriverEvent::Output {
        stream: stream.to_string(),
        data: redacted,
    });
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_working_directories_for_posix_shells() {
        assert_eq!(shell_quote("/tmp/a b"), "'/tmp/a b'");
        assert_eq!(shell_quote("/tmp/user's"), "'/tmp/user'\\''s'");
    }

    #[test]
    fn missing_exit_status_is_failure_not_success() {
        let err = command_step_outcome(None, None).unwrap_err();
        assert!(matches!(
            err,
            TaskStepError::Failed {
                exit_code: None,
                message
            } if message.contains("without an exit status")
        ));
    }

    #[test]
    fn non_zero_exit_status_is_failure() {
        let err = command_step_outcome(Some(1), None).unwrap_err();
        assert!(matches!(
            err,
            TaskStepError::Failed {
                exit_code: Some(1),
                ..
            }
        ));
    }

    #[test]
    fn zero_exit_status_is_success() {
        assert_eq!(
            command_step_outcome(Some(0), None).unwrap(),
            TaskStepResult {
                exit_code: Some(0)
            }
        );
    }

    #[test]
    fn exit_signal_is_failure_even_with_zero_code() {
        let err = command_step_outcome(Some(0), Some("signal SIGKILL")).unwrap_err();
        assert!(matches!(err, TaskStepError::Failed { .. }));
    }
}
