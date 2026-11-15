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
        unfour_core::local_path::check_runtime(&config.local_path, true).map_err(|code| {
            TaskStepError::Failed {
                message: format!("LOCAL_PATH_UNAVAILABLE:{code}"),
                exit_code: None,
            }
        })?;
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
        let (mut local, part) = stage_task_download(&target).await?;
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
            return Err(error);
        }
        replace_local_download(part, target, config.overwrite).await?;
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
async fn stage_task_download(
    target: &std::path::Path,
) -> Result<(tokio::fs::File, tempfile::TempPath), TaskStepError> {
    let parent = target
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_owned();
    let staged = tokio::task::spawn_blocking(move || {
        let mut builder = tempfile::Builder::new();
        builder.prefix(".unfour-task-download-").suffix(".part");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o666));
        }
        builder.tempfile_in(parent)
    })
    .await
    .map_err(|error| TaskStepError::Failed {
        message: error.to_string(),
        exit_code: None,
    })?
    .map_err(io_step_error("create local download file"))?;
    let (file, part) = staged.into_parts();
    Ok((tokio::fs::File::from_std(file), part))
}

#[cfg(feature = "ssh-native")]
async fn replace_local_download(
    part: tempfile::TempPath,
    target: std::path::PathBuf,
    overwrite: bool,
) -> Result<(), TaskStepError> {
    // The publication itself enforces overwrite policy; an exists check cannot
    // protect against a second run publishing between check and rename.
    tokio::task::spawn_blocking(move || {
        if overwrite {
            part.persist(target)
        } else {
            part.persist_noclobber(target)
        }
    })
    .await
    .map_err(|error| TaskStepError::Failed {
        message: error.to_string(),
        exit_code: None,
    })?
    .map_err(|error| io_step_error("finalize local download")(error.error))
}

#[cfg(test)]
mod tests {
    use super::resolve_download_local_target;

    #[tokio::test]
    async fn concurrent_download_staging_is_owned_and_publication_does_not_clobber() {
        use super::{replace_local_download, stage_task_download};
        use tokio::io::AsyncWriteExt;
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("result");
        let (mut first, first_path) = stage_task_download(&target).await.unwrap();
        let (mut second, second_path) = stage_task_download(&target).await.unwrap();
        assert_ne!(&*first_path, &*second_path);
        first.write_all(b"first").await.unwrap();
        second.write_all(b"second").await.unwrap();
        drop(first);
        drop(second);
        let (left, right) = tokio::join!(
            replace_local_download(first_path, target.clone(), false),
            replace_local_download(second_path, target.clone(), false)
        );
        assert_ne!(left.is_ok(), right.is_ok());
        let bytes = std::fs::read(&target).unwrap();
        assert_eq!(
            bytes,
            if left.is_ok() {
                b"first".as_slice()
            } else {
                b"second".as_slice()
            }
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

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
