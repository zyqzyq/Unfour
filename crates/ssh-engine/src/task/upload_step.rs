use super::*;

#[cfg(feature = "ssh-native")]
impl NativeTaskDriver {
    pub(super) async fn run_upload_step(
        &mut self,
        step: &SshTaskStep,
        cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) -> Result<TaskStepResult, TaskStepError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let config =
            parse_upload_config(step.config_version, &step.config_json).map_err(|error| {
                TaskStepError::Failed {
                    message: error.to_string(),
                    exit_code: None,
                }
            })?;
        let remote_path = normalize_task_remote_path(&config.remote_path)?;
        let mut local = tokio::fs::File::open(&config.local_path)
            .await
            .map_err(|error| TaskStepError::Failed {
                message: format!("open local upload file failed: {error}"),
                exit_code: None,
            })?;
        let total = local
            .metadata()
            .await
            .map_err(|error| TaskStepError::Failed {
                message: format!("read local upload metadata failed: {error}"),
                exit_code: None,
            })?
            .len();
        let sftp = self.sftp().await?;
        if !config.overwrite
            && sftp
                .try_exists(remote_path.clone())
                .await
                .map_err(sftp_step_error("check remote upload target"))?
        {
            return Err(TaskStepError::Failed {
                message: "remote target already exists".to_string(),
                exit_code: None,
            });
        }
        let temp_path = format!("{remote_path}.unfour-task-uploading-{}", step.id);
        let mut remote = sftp
            .create(temp_path.clone())
            .await
            .map_err(sftp_step_error("create remote upload file"))?;
        let started = std::time::Instant::now();
        let mut transferred = 0_u64;
        let mut buffer = vec![0_u8; 256 * 1024];
        let copy_result = async {
            loop {
                let read = tokio::select! {
                    changed = cancel_rx.changed() => {
                        let _ = changed;
                        return Err(TaskStepError::Cancelled);
                    }
                    result = local.read(&mut buffer) => result.map_err(io_step_error("read local upload file"))?,
                };
                if read == 0 {
                    break;
                }
                tokio::select! {
                    changed = cancel_rx.changed() => {
                        let _ = changed;
                        return Err(TaskStepError::Cancelled);
                    }
                    result = remote.write_all(&buffer[..read]) => {
                        result.map_err(io_step_error("write remote upload file"))?;
                    }
                }
                transferred = transferred.saturating_add(read as u64);
                emit(transfer_progress("upload", transferred, total, started));
            }
            remote.flush().await.map_err(io_step_error("flush remote upload file"))?;
            remote.shutdown().await.map_err(io_step_error("close remote upload file"))?;
            Ok::<(), TaskStepError>(())
        }
        .await;
        if let Err(error) = copy_result {
            let _ = sftp.remove_file(temp_path).await;
            return Err(error);
        }
        replace_remote_file(&sftp, &temp_path, &remote_path, &step.id, config.overwrite).await?;
        emit(transfer_progress("upload", total, total, started));
        Ok(TaskStepResult { exit_code: None })
    }
}

#[cfg(feature = "ssh-native")]
async fn replace_remote_file(
    sftp: &russh_sftp::client::SftpSession,
    temp_path: &str,
    target_path: &str,
    step_id: &str,
    overwrite: bool,
) -> Result<(), TaskStepError> {
    let backup = format!("{target_path}.unfour-task-backup-{step_id}");
    let had_backup = overwrite
        && sftp
            .rename(target_path.to_string(), backup.clone())
            .await
            .is_ok();
    if let Err(error) = sftp
        .rename(temp_path.to_string(), target_path.to_string())
        .await
    {
        if had_backup {
            let _ = sftp.rename(backup.clone(), target_path.to_string()).await;
        }
        return Err(TaskStepError::Failed {
            message: format!("finalize remote upload failed: {error}"),
            exit_code: None,
        });
    }
    if had_backup {
        let _ = sftp.remove_file(backup).await;
    }
    Ok(())
}

#[cfg(feature = "ssh-native")]
pub(super) fn sftp_step_error(
    operation: &'static str,
) -> impl FnOnce(russh_sftp::client::error::Error) -> TaskStepError {
    move |error| TaskStepError::Failed {
        message: format!("{operation} failed: {error}"),
        exit_code: None,
    }
}

#[cfg(feature = "ssh-native")]
pub(super) fn io_step_error(
    operation: &'static str,
) -> impl FnOnce(std::io::Error) -> TaskStepError {
    move |error| TaskStepError::Failed {
        message: format!("{operation} failed: {error}"),
        exit_code: None,
    }
}
