use super::*;

#[cfg(feature = "ssh-native")]
pub(super) struct NativeTaskDriver {
    pub(super) handle: std::sync::Arc<russh::client::Handle<SshClientHandler>>,
    pub(super) sftp: Option<std::sync::Arc<russh_sftp::client::SftpSession>>,
    shared: Option<super::shared::SharedTaskSource>,
    sftp_bridge: Option<super::shared::TaskSftpBridge>,
}

#[cfg(feature = "ssh-native")]
impl NativeTaskDriver {
    pub(super) fn shared(source: super::shared::SharedTaskSource) -> Self {
        Self {
            handle: source.handle.clone(),
            shared: Some(source),
            sftp: None,
            sftp_bridge: None,
        }
    }
    pub(super) async fn open_task_channel(
        &self,
    ) -> Result<russh::Channel<russh::client::Msg>, TaskStepError> {
        if let Some(source) = &self.shared {
            return source.channel().await;
        }
        self.handle
            .channel_open_session()
            .await
            .map_err(task_transport_error("open task channel"))
    }
    pub(super) async fn connect(
        service: &SshService,
        connection: &SshConnection,
        temporary: Option<&TemporarySshTunnel>,
    ) -> Result<Self, String> {
        use crate::ssh::native_transport::NativeConnectError;

        if let Some(tunnel) = temporary {
            let handle = service
                .open_forwarded_connection(connection, None, tunnel)
                .await
                .map_err(|error| error.to_string())?;
            return Ok(Self {
                handle: std::sync::Arc::new(handle),
                sftp: None,
                shared: None,
                sftp_bridge: None,
            });
        }

        let mut handle = service
            .dial_native_connection(connection)
            .await
            .map_err(|error| match error {
                NativeConnectError::Failed(error) => {
                    format!("SSH connection failed: {}", sanitize_ssh_error(&error))
                }
                NativeConnectError::TimedOut => {
                    "SSH connection timed out after 15 seconds".to_string()
                }
            })?;
        service
            .authenticate_native(&mut handle, connection, None)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self {
            handle: std::sync::Arc::new(handle),
            sftp: None,
            shared: None,
            sftp_bridge: None,
        })
    }

    pub(super) async fn sftp(
        &mut self,
    ) -> Result<std::sync::Arc<russh_sftp::client::SftpSession>, TaskStepError> {
        if let Some(sftp) = &self.sftp {
            return Ok(sftp.clone());
        }
        let channel = self.open_task_channel().await?;
        if self.shared.is_some() {
            // Retain the independent close handle before any await, including
            // the subsystem ACK and SFTP version handshake.
            let (mut reader, bridge) = shared::TaskSftpBridge::new(channel);
            self.sftp_bridge = Some(bridge);
            let bridge = self
                .sftp_bridge
                .as_mut()
                .expect("SFTP bridge just installed");
            bridge.request().await?;
            let accepted = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    match reader.wait().await {
                        Some(russh::ChannelMsg::Success) => return Ok(()),
                        Some(russh::ChannelMsg::Failure) => {
                            return Err(shared::task_error("SSH_TASK_SFTP_UNSUPPORTED"))
                        }
                        Some(russh::ChannelMsg::Close) | None => {
                            return Err(shared::task_error("SSH_TASK_TRANSPORT_CLOSED"))
                        }
                        _ => {}
                    }
                }
            })
            .await
            .unwrap_or_else(|_| Err(shared::task_error("SSH_TASK_CHANNEL_TIMEOUT")));
            accepted?;
            let stream = bridge.start(reader);
            let sftp = std::sync::Arc::new(
                russh_sftp::client::SftpSession::new_with_config(
                    crate::ssh::temporary::probe::bounded_stream::BoundedSftpStream::new_with_limit(
                        stream,
                        512 * 1024,
                    ),
                    task_sftp_config(),
                )
                .await
                .map_err(|_| shared::task_error("SSH_TASK_SFTP_FAILED"))?,
            );
            sftp.set_timeout(30);
            self.sftp = Some(sftp.clone());
            return Ok(sftp);
        }
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(task_transport_error("request SFTP subsystem"))?;
        let sftp = std::sync::Arc::new(
            russh_sftp::client::SftpSession::new_with_config(
                channel.into_stream(),
                task_sftp_config(),
            )
            .await
            .map_err(|error| TaskStepError::Failed {
                message: format!("initialize SFTP failed: {error}"),
                exit_code: None,
            })?,
        );
        sftp.set_timeout(30);
        self.sftp = Some(sftp.clone());
        Ok(sftp)
    }

    pub(super) async fn disconnect(mut self) {
        self.sftp.take();
        if let Some(bridge) = self.sftp_bridge.take() {
            bridge.close().await;
        }
        if self.shared.is_some() {
            return;
        }
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "SSH task complete", "en")
            .await;
    }
}

#[cfg(feature = "ssh-native")]
impl TaskStepDriver for NativeTaskDriver {
    async fn execute_step(
        &mut self,
        step: &SshTaskStep,
        cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) -> Result<TaskStepResult, TaskStepError> {
        if self.shared.is_some() && matches!(step.step_type.as_str(), "upload" | "download") {
            let mut stop = cancel_rx.clone();
            let operation = async {
                if step.step_type == "upload" {
                    self.run_upload_step(step, cancel_rx, emit).await
                } else {
                    self.run_download_step(step, cancel_rx, emit).await
                }
            };
            tokio::pin!(operation);
            return tokio::select! {
                biased;
                _ = async { let _ = stop.wait_for(|stopped| *stopped).await; } => {
                    // Allow owned staging cleanup, then stop the independent I/O
                    // bridge. Cancellation cannot hang on a server's window.
                    tokio::time::timeout(std::time::Duration::from_secs(5), &mut operation).await
                        .unwrap_or(Err(TaskStepError::OutcomeUncertain { cancelled: true }))
                },
                result = &mut operation => result,
            };
        }
        match step.step_type.as_str() {
            "command" => self.run_command_step(step, cancel_rx, emit).await,
            "upload" => self.run_upload_step(step, cancel_rx, emit).await,
            "download" => self.run_download_step(step, cancel_rx, emit).await,
            kind => Err(TaskStepError::Failed {
                message: format!("unsupported SSH task step type: {kind}"),
                exit_code: None,
            }),
        }
    }
}

#[cfg(feature = "ssh-native")]
fn task_sftp_config() -> russh_sftp::client::Config {
    russh_sftp::client::Config {
        max_packet_len: 256 * 1024,
        max_concurrent_writes: 64,
        request_timeout_secs: 30,
    }
}

#[cfg(feature = "ssh-native")]
fn task_transport_error(operation: &'static str) -> impl FnOnce(russh::Error) -> TaskStepError {
    move |error| TaskStepError::Failed {
        message: format!("{operation} failed: {error}"),
        exit_code: None,
    }
}

pub(super) fn normalize_task_remote_path(path: &str) -> Result<String, TaskStepError> {
    if path.is_empty() || path.contains('\0') || !path.starts_with('/') {
        return Err(TaskStepError::Failed {
            message: "remote path must be an absolute POSIX path".to_string(),
            exit_code: None,
        });
    }
    let mut components = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            value => components.push(value),
        }
    }
    if components.is_empty() {
        return Err(TaskStepError::Failed {
            message: "remote file path cannot be the root directory".to_string(),
            exit_code: None,
        });
    }
    Ok(format!("/{}", components.join("/")))
}

pub(super) fn transfer_progress(
    direction: &str,
    transferred_bytes: u64,
    total_bytes: u64,
    started: std::time::Instant,
) -> DriverEvent {
    DriverEvent::Transfer(TaskTransferProgress {
        direction: direction.to_string(),
        transferred_bytes,
        total_bytes,
        bytes_per_second: (transferred_bytes as f64 / started.elapsed().as_secs_f64().max(0.001))
            as u64,
    })
}
