#[cfg(feature = "ssh-native")]
use super::*;
#[cfg(not(feature = "ssh-native"))]
use super::{TaskStepError, TaskStepResult};

#[cfg(feature = "ssh-native")]
pub(super) mod stream;
#[cfg(feature = "ssh-native")]
use stream::{emit_diagnostic, emit_redacted, StreamBuffer};

#[cfg(feature = "ssh-native")]
const EXIT_DRAIN_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

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
        let mut channel = self.open_task_channel().await?;
        // One execution deadline covers both the bounded client send queue and
        // the remote response. Channel acquisition has its own transport bound.
        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_secs(config.timeout_seconds.clamp(1, 3_600));
        // The exec future enqueues the request. Cancelling before the send
        // completes leaves no remote command to account for.
        let sent =
            send_exec_request(channel.exec(true, command.as_bytes()), cancel_rx, deadline).await;
        if let Err(error) = sent {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), channel.close()).await;
            return Err(error);
        }
        emit_redacted("command", &format!("$ {command}\n"), emit);

        let interrupt = async {
            tokio::select! {
                _ = cancel_rx.wait_for(|cancelled| *cancelled) => true,
                _ = tokio::time::sleep_until(deadline) => false,
            }
        };
        let result = read_command_result(&mut channel, interrupt, emit).await;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), channel.close()).await;
        result
    }
}

#[cfg(feature = "ssh-native")]
async fn send_exec_request(
    send: impl std::future::Future<Output = Result<(), russh::Error>>,
    cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
    deadline: tokio::time::Instant,
) -> Result<(), TaskStepError> {
    tokio::select! {
        biased;
        _ = cancel_rx.wait_for(|cancelled| *cancelled) => Err(TaskStepError::Cancelled),
        _ = tokio::time::sleep_until(deadline) => Err(TaskStepError::Failed {
            message: "SSH_TASK_EXEC_QUEUE_TIMEOUT".into(),
            exit_code: None,
        }),
        sent = send => sent.map_err(|error| TaskStepError::Failed {
            message: format!("execute command failed: {error}"),
            exit_code: None,
        }),
    }
}

#[cfg(feature = "ssh-native")]
trait CommandMessages: Send {
    fn next_message(
        &mut self,
    ) -> impl std::future::Future<Output = Option<russh::ChannelMsg>> + Send;
}

#[cfg(feature = "ssh-native")]
impl CommandMessages for russh::Channel<russh::client::Msg> {
    fn next_message(
        &mut self,
    ) -> impl std::future::Future<Output = Option<russh::ChannelMsg>> + Send {
        self.wait()
    }
}

#[cfg(feature = "ssh-native")]
async fn read_command_result(
    channel: &mut impl CommandMessages,
    interrupt: impl std::future::Future<Output = bool> + Send,
    emit: &mut (dyn FnMut(DriverEvent) + Send),
) -> Result<TaskStepResult, TaskStepError> {
    read_command_result_with_grace(channel, interrupt, emit, EXIT_DRAIN_GRACE).await
}

#[cfg(feature = "ssh-native")]
async fn read_command_result_with_grace(
    channel: &mut impl CommandMessages,
    interrupt: impl std::future::Future<Output = bool> + Send,
    emit: &mut (dyn FnMut(DriverEvent) + Send),
    drain_grace: std::time::Duration,
) -> Result<TaskStepResult, TaskStepError> {
    let mut stdout = StreamBuffer::new();
    let mut stderr = StreamBuffer::new();
    let mut exit_code = None;
    let mut exit_signal = None;
    let mut interrupted = None;
    let mut remaining = 0;
    let mut eof = false;
    let mut closed = false;
    let mut exit_deadline = None;
    let mut prompt_reported = false;
    let mut prompt_deadline = None;
    tokio::pin!(interrupt);
    loop {
        let message = if interrupted.is_some() {
            // The transport can queue an exit status just before the deadline or cancel.
            // Poll only ready messages, up to one full channel queue even on a busy stream.
            if remaining == 0 {
                break;
            }
            remaining -= 1;
            tokio::select! {
                biased;
                message = tokio::task::unconstrained(channel.next_message()) => message,
                _ = std::future::ready(()) => break,
            }
        } else {
            tokio::select! {
                biased;
                cancelled = &mut interrupt => {
                    interrupted = Some(cancelled);
                    remaining = native_client_config().channel_buffer_size;
                    continue;
                }
                _ = wait_for_exit_drain(exit_deadline) => break,
                _ = wait_for_exit_drain(prompt_deadline) => {
                    prompt_reported = true;
                    prompt_deadline = None;
                    emit_diagnostic("SSH_TASK_INTERACTIVE_LOGIN_PROMPT", emit);
                    continue;
                }
                message = channel.next_message() => message,
            }
        };
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
                exit_deadline.get_or_insert(tokio::time::Instant::now() + drain_grace);
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
                exit_deadline.get_or_insert(tokio::time::Instant::now() + drain_grace);
            }
            // Exec is the only channel request awaiting a reply.
            // A refusal confirms the command did not start.
            Some(russh::ChannelMsg::Failure) => {
                return Err(TaskStepError::Failed {
                    message: "SSH server rejected the command request".to_string(),
                    exit_code: None,
                });
            }
            // EOF only means stdout/stderr are done. ExitStatus often
            // arrives afterwards; breaking here used to drop the code and
            // mark failed docker/shell commands as success via unwrap_or(0).
            Some(russh::ChannelMsg::Eof) => eof = true,
            Some(russh::ChannelMsg::Close) | None => {
                closed = true;
                break;
            }
            _ => {}
        }
        if !prompt_reported && (stdout.has_login_prompt() || stderr.has_login_prompt()) {
            // A packet ending in "login:" may be the prefix of a longer log
            // line. Allow continuation before reporting the observation.
            prompt_deadline
                .get_or_insert(tokio::time::Instant::now() + std::time::Duration::from_millis(250));
        } else {
            prompt_deadline = None;
        }
        // EOF establishes that no more stream data will arrive, but is not an
        // exit code. Accept either ordering without waiting for a missing Close.
        if eof && (exit_code.is_some() || exit_signal.is_some()) {
            break;
        }
    }
    if !prompt_reported && (stdout.has_login_prompt() || stderr.has_login_prompt()) {
        emit_diagnostic("SSH_TASK_INTERACTIVE_LOGIN_PROMPT", emit);
    }
    stdout.flush("stdout", emit);
    stderr.flush("stderr", emit);
    if stdout.truncated || stderr.truncated {
        emit(DriverEvent::Output {
            stream: "stderr".to_string(),
            data: "\n[command output truncated]\n".to_string(),
        });
    }
    if !eof && !closed && (exit_code.is_some() || exit_signal.is_some()) {
        emit_diagnostic("SSH_TASK_OUTPUT_INCOMPLETE", emit);
    }
    command_step_outcome(
        exit_code,
        exit_signal.as_deref(),
        interrupted.unwrap_or(false),
    )
}

#[cfg(feature = "ssh-native")]
async fn wait_for_exit_drain(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// Decide command-step success from the SSH channel completion signals.
fn command_step_outcome(
    exit_code: Option<i32>,
    exit_signal: Option<&str>,
    cancelled: bool,
) -> Result<TaskStepResult, TaskStepError> {
    if let Some(signal) = exit_signal {
        return Err(TaskStepError::Failed {
            message: format!("Command terminated by {signal}"),
            exit_code,
        });
    }
    match exit_code {
        Some(0) => Ok(TaskStepResult { exit_code: Some(0) }),
        Some(code) => Err(TaskStepError::Failed {
            message: format!("Command exited with status {code}"),
            exit_code: Some(code),
        }),
        None => Err(TaskStepError::OutcomeUncertain { cancelled }),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests;
