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
        emit(DriverEvent::Output {
            stream: "command".to_string(),
            data: format!("$ download {remote_path} -> {}\n", config.local_path),
        });
        let target = resolve_download_local_target(&config.local_path)?;
        if target.exists() && !config.overwrite {
            return Err(TaskStepError::Failed {
                message: "local target already exists".to_string(),
                exit_code: None,
            });
        }
        if let Some(parent) = target.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|error| TaskStepError::Failed {
                        message: format!("create local download directory failed: {error}"),
                        exit_code: None,
                    })?;
            }
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
        let mut progress_throttle = TransferProgressThrottle::new();
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
                if progress_throttle.should_emit(std::time::Instant::now()) {
                    emit(transfer_progress("download", transferred, total, started));
                }
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

/// Refuse bare directories so download never renames/overwrites a folder as if it
/// were the destination file.
fn resolve_download_local_target(local_path: &str) -> Result<std::path::PathBuf, TaskStepError> {
    let trimmed = local_path.trim();
    if trimmed.is_empty() {
        return Err(TaskStepError::Failed {
            message: "local download path is empty".to_string(),
            exit_code: None,
        });
    }
    if trimmed.ends_with('/') || trimmed.ends_with('\\') {
        return Err(TaskStepError::Failed {
            message: "local download path must include a file name, not only a directory"
                .to_string(),
            exit_code: None,
        });
    }
    let target = std::path::PathBuf::from(trimmed);
    if target.is_dir() {
        return Err(TaskStepError::Failed {
            message: "local download path is a directory; append a file name".to_string(),
            exit_code: None,
        });
    }
    Ok(target)
}

#[cfg(feature = "ssh-native")]
async fn replace_local_download(
    part: &std::path::Path,
    target: &std::path::Path,
    step_id: &str,
    overwrite: bool,
) -> Result<(), TaskStepError> {
    if target.is_dir() {
        return Err(TaskStepError::Failed {
            message: "local download path is a directory; append a file name".to_string(),
            exit_code: None,
        });
    }
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

#[cfg(test)]
mod tests {
    use super::resolve_download_local_target;

    #[test]
    fn rejects_trailing_directory_separators() {
        let err = resolve_download_local_target(r"C:\Downloads\").unwrap_err();
        assert!(
            matches!(err, super::TaskStepError::Failed { message, .. } if message.contains("file name"))
        );
        let err = resolve_download_local_target("/tmp/out/").unwrap_err();
        assert!(
            matches!(err, super::TaskStepError::Failed { message, .. } if message.contains("file name"))
        );
    }

    #[test]
    fn rejects_empty_paths() {
        let err = resolve_download_local_target("   ").unwrap_err();
        assert!(
            matches!(err, super::TaskStepError::Failed { message, .. } if message.contains("empty"))
        );
    }

    #[test]
    fn accepts_file_paths() {
        let target = resolve_download_local_target(r"C:\Downloads\archive.tar").unwrap();
        assert_eq!(
            target,
            std::path::PathBuf::from(r"C:\Downloads\archive.tar")
        );
    }
}
