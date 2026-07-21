use super::*;

#[cfg(feature = "ssh-native")]
pub(super) struct NativeTaskDriver {
    pub(super) handle: russh::client::Handle<SshClientHandler>,
    pub(super) sftp: Option<std::sync::Arc<russh_sftp::client::SftpSession>>,
}

#[cfg(feature = "ssh-native")]
impl NativeTaskDriver {
    pub(super) async fn connect(
        service: &SshService,
        connection: &SshConnection,
    ) -> Result<Self, TaskStepError> {
        let config = Arc::new(native_client_config());
        let handler = SshClientHandler {
            host_key_store: HostKeyStore::new(service.db.pool().clone()),
            workspace_id: connection.workspace_id.clone(),
            host: connection.host.clone(),
            port: connection.port,
        };
        let address = format!("{}:{}", connection.host, connection.port);
        let mut handle = match tokio::time::timeout(
            std::time::Duration::from_secs(15),
            russh::client::connect(config, address, handler),
        )
        .await
        {
            Ok(Ok(handle)) => handle,
            Ok(Err(error)) => {
                return Err(TaskStepError::Failed {
                    message: format!("SSH connection failed: {}", sanitize_ssh_error(&error)),
                    exit_code: None,
                });
            }
            Err(_) => {
                return Err(TaskStepError::Failed {
                    message: "SSH connection timed out after 15 seconds".to_string(),
                    exit_code: None,
                });
            }
        };
        service
            .authenticate_native(&mut handle, connection, None)
            .await
            .map_err(|error| TaskStepError::Failed {
                message: error.to_string(),
                exit_code: None,
            })?;
        Ok(Self { handle, sftp: None })
    }

    pub(super) async fn sftp(
        &mut self,
    ) -> Result<std::sync::Arc<russh_sftp::client::SftpSession>, TaskStepError> {
        if let Some(sftp) = &self.sftp {
            return Ok(sftp.clone());
        }
        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(task_transport_error("open SFTP channel"))?;
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
