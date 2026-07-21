#[cfg(feature = "ssh-native")]
use super::*;

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
        let command =
            validate_one_shot_command(&command).map_err(|error| TaskStepError::Failed {
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

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = None;
        let mut truncated = false;
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
                            append_capped(&mut stdout, &data[..], &mut truncated);
                        }
                        Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                            if ext == 1 {
                                append_capped(&mut stderr, &data[..], &mut truncated);
                            } else {
                                append_capped(&mut stdout, &data[..], &mut truncated);
                            }
                        }
                        Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                            exit_code = Some(exit_status as i32);
                        }
                        Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) | None => break,
                        _ => {}
                    }
                }
            }
        }
        let _ = channel.close().await;
        let (stdout, _) = redact_sensitive_lines(&String::from_utf8_lossy(&stdout));
        let (stderr, _) = redact_sensitive_lines(&String::from_utf8_lossy(&stderr));
        if !stdout.is_empty() {
            emit(DriverEvent::Output {
                stream: "stdout".to_string(),
                data: stdout,
            });
        }
        if !stderr.is_empty() {
            emit(DriverEvent::Output {
                stream: "stderr".to_string(),
                data: stderr,
            });
        }
        if truncated {
            emit(DriverEvent::Output {
                stream: "stderr".to_string(),
                data: "\n[command output truncated]\n".to_string(),
            });
        }
        if exit_code.unwrap_or(0) != 0 {
            return Err(TaskStepError::Failed {
                message: format!("Command exited with status {}", exit_code.unwrap_or(-1)),
                exit_code,
            });
        }
        Ok(TaskStepResult { exit_code })
    }
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
}
