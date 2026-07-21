use super::*;

#[cfg(feature = "ssh-native")]
impl NativeTaskDriver {
    pub(super) async fn run_download_step(
        &mut self,
        step: &SshTaskStep,
        cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
        emit: &mut (dyn FnMut(DriverEvent) + Send),
    ) -> Result<TaskStepResult, TaskStepError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let config =
            parse_download_config(step.config_version, &step.config_json).map_err(|error| {
                TaskStepError::Failed {
                    message: error.to_string(),
                    exit_code: None,
                }
            })?;
        let remote_path = normalize_task_remote_path(&config.remote_path)?;
        let target = std::path::PathBuf::from(&config.local_path);
        if target.exists() && !config.overwrite {
            return Err(TaskStepError::Failed {
                message: "local target already exists".to_string(),
                exit_code: None,
            });
        }
        let mut part_name = target.as_os_str().to_os_string();
        part_name.push(format!(".unfour-task-part-{}", step.id));
        let part = std::path::PathBuf::from(part_name);
        let sftp = self.sftp().await?;
        let total = sftp
            .metadata(remote_path.clone())
            .await
            .map_err(sftp_step_error("read remote download metadata"))?
            .len();
        let mut remote = sftp
            .open(remote_path)
            .await
            .map_err(sftp_step_error("open remote download file"))?;
        let mut local = tokio::fs::File::create(&part)
            .await
            .map_err(io_step_error("create local download file"))?;
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
                    result = remote.read(&mut buffer) => result.map_err(io_step_error("read remote download file"))?,
                };
                if read == 0 {
                    break;
                }
                tokio::select! {
                    changed = cancel_rx.changed() => {
                        let _ = changed;
                        return Err(TaskStepError::Cancelled);
                    }
                    result = local.write_all(&buffer[..read]) => {
                        result.map_err(io_step_error("write local download file"))?;
                    }
                }
                transferred = transferred.saturating_add(read as u64);
                emit(transfer_progress("download", transferred, total, started));
            }
            local.flush().await.map_err(io_step_error("flush local download file"))?;
            Ok::<(), TaskStepError>(())
        }
        .await;
        drop(local);
        if let Err(error) = copy_result {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(error);
        }
        replace_local_download(&part, &target, &step.id, config.overwrite).await?;
        emit(transfer_progress("download", total, total, started));
        Ok(TaskStepResult { exit_code: None })
    }
}

#[cfg(feature = "ssh-native")]
async fn replace_local_download(
    part: &std::path::Path,
    target: &std::path::Path,
    step_id: &str,
    overwrite: bool,
) -> Result<(), TaskStepError> {
    let mut backup_name = target.as_os_str().to_os_string();
    backup_name.push(format!(".unfour-task-backup-{step_id}"));
    let backup = std::path::PathBuf::from(backup_name);
    let had_target = target.exists();
    if had_target {
        if !overwrite {
            return Err(TaskStepError::Failed {
                message: "local target already exists".to_string(),
                exit_code: None,
            });
        }
        tokio::fs::rename(target, &backup)
            .await
            .map_err(io_step_error("backup existing local download"))?;
    }
    if let Err(error) = tokio::fs::rename(part, target).await {
        if had_target {
            let _ = tokio::fs::rename(&backup, target).await;
        }
        return Err(TaskStepError::Failed {
            message: format!("finalize local download failed: {error}"),
            exit_code: None,
        });
    }
    if had_target {
        let _ = tokio::fs::remove_file(backup).await;
    }
    Ok(())
}
