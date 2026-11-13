use super::*;

#[cfg(feature = "ssh-native")]
#[derive(Debug)]
pub(super) enum TransferRunError {
    Cancelled,
    Failed(String),
}

#[cfg(feature = "ssh-native")]
pub(super) fn ensure_not_cancelled(
    cancel_rx: &tokio::sync::watch::Receiver<bool>,
) -> Result<(), TransferRunError> {
    if *cancel_rx.borrow() {
        Err(TransferRunError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(feature = "ssh-native")]
pub(super) fn transfer_sftp_error(error: russh_sftp::client::error::Error) -> TransferRunError {
    TransferRunError::Failed(error.to_string())
}

#[cfg(feature = "ssh-native")]
pub(super) async fn stage_download(
    target: &std::path::Path,
) -> Result<(tokio::fs::File, tempfile::TempPath), TransferRunError> {
    let parent = target
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_owned();
    let staged = tokio::task::spawn_blocking(move || {
        let mut builder = tempfile::Builder::new();
        builder.prefix(".unfour-download-").suffix(".part");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // The published download has ordinary file-creation permissions, subject to umask.
            builder.permissions(std::fs::Permissions::from_mode(0o666));
        }
        builder.tempfile_in(parent)
    })
    .await
    .map_err(|error| TransferRunError::Failed(error.to_string()))?
    .map_err(|error| TransferRunError::Failed(error.to_string()))?;
    let (file, staging) = staged.into_parts();
    Ok((tokio::fs::File::from_std(file), staging))
}

#[cfg(feature = "ssh-native")]
pub(super) async fn publish_download(
    staging: tempfile::TempPath,
    target: std::path::PathBuf,
    overwrite: bool,
) -> Result<(), TransferRunError> {
    tokio::task::spawn_blocking(move || {
        if overwrite {
            staging.persist(target)
        } else {
            staging.persist_noclobber(target)
        }
    })
    .await
    .map_err(|error| TransferRunError::Failed(error.to_string()))?
    .map_err(|error| TransferRunError::Failed(error.error.to_string()))
}

#[cfg(feature = "ssh-native")]
pub(super) async fn finalize_remote_upload(
    sftp: &SftpSession,
    temp_path: &str,
    target_path: &str,
    transfer_id: &str,
    overwrite: bool,
) -> Result<(), TransferRunError> {
    if !overwrite {
        // SSH_FXP_RENAME in SFTP v3 must fail if newpath exists. Let the
        // server perform the no-replace operation after the upload completes.
        return sftp
            .rename(temp_path, target_path)
            .await
            .map_err(transfer_sftp_error);
    }
    let backup = format!("{target_path}.unfour-backup-{transfer_id}");
    // Opportunistic backup avoids a prior try_exists RTT on overwrite uploads.
    let had_backup = sftp
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
        return Err(transfer_sftp_error(error));
    }
    if had_backup {
        let _ = sftp.remove_file(backup).await;
    }
    Ok(())
}

pub(super) fn normalize_remote_path(path: &str) -> AppResult<String> {
    if path.is_empty() || path.contains('\0') || !path.starts_with('/') {
        return Err(AppError::Validation(
            "remote path must be an absolute POSIX path".to_string(),
        ));
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
    Ok(if components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", components.join("/"))
    })
}

pub(super) fn normalize_mutation_path(path: &str) -> AppResult<String> {
    let path = normalize_remote_path(path)?;
    if path == "/" {
        return Err(AppError::Validation(
            "the remote root cannot be modified".to_string(),
        ));
    }
    Ok(path)
}

#[cfg(feature = "ssh-native")]
pub(super) fn remote_file_name(path: &str) -> String {
    path.rsplit('/')
        .find(|component| !component.is_empty())
        .unwrap_or("/")
        .to_string()
}

#[cfg(feature = "ssh-native")]
pub(super) fn file_kind(kind: russh_sftp::protocol::FileType) -> &'static str {
    if kind.is_dir() {
        "directory"
    } else if kind.is_file() {
        "file"
    } else if kind.is_symlink() {
        "symlink"
    } else {
        "other"
    }
}

#[cfg(feature = "ssh-native")]
pub(super) fn file_kind_rank(kind: &str) -> u8 {
    match kind {
        "directory" => 0,
        "file" => 1,
        "symlink" => 2,
        _ => 3,
    }
}

#[cfg(any(feature = "ssh-native", test))]
pub(super) fn upload_temp_path(remote_path: &str, transfer_id: &str) -> String {
    let parent = remote_path
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or(".");
    format!("{parent}/.unfour-upload-{transfer_id}")
}

#[cfg(feature = "ssh-native")]
pub(super) fn sftp_error(operation: &str, error: impl std::fmt::Display) -> AppError {
    AppError::Config(format!("{operation} failed: {error}"))
}

#[cfg(any(feature = "ssh-native", test))]
pub(super) fn transfer_speed(transferred_bytes: u64, elapsed_seconds: f64) -> u64 {
    (transferred_bytes as f64 / elapsed_seconds.max(0.001)) as u64
}

/// Keep the newest finished transfers and drop the rest by `started_at`.
#[cfg(any(feature = "ssh-native", test))]
pub(super) fn finished_transfer_ids_to_prune(
    mut finished: Vec<(String, String)>,
    max_finished: usize,
) -> Vec<String> {
    if finished.len() <= max_finished {
        return Vec::new();
    }
    finished.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    let remove_count = finished.len() - max_finished;
    finished
        .into_iter()
        .take(remove_count)
        .map(|(id, _)| id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(unix, feature = "ssh-native"))]
    #[tokio::test]
    async fn published_downloads_use_normal_file_creation_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let control = std::fs::File::create(dir.path().join("control")).unwrap();
        let expected = control.metadata().unwrap().permissions().mode() & 0o777;
        let target = dir.path().join("download");
        for overwrite in [false, true] {
            let (file, staging) = stage_download(&target).await.unwrap();
            drop(file);
            publish_download(staging, target.clone(), overwrite)
                .await
                .unwrap();
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                expected
            );
        }
    }

    #[test]
    fn normalizes_remote_paths_without_shell_semantics() {
        assert_eq!(
            normalize_remote_path("/tmp/../home/./用户").unwrap(),
            "/home/用户"
        );
        assert_eq!(normalize_remote_path("/../../").unwrap(), "/");
        assert!(normalize_remote_path("relative/path").is_err());
        assert!(normalize_remote_path("/tmp/\0bad").is_err());
    }

    #[test]
    fn protects_remote_root_from_mutation() {
        assert!(normalize_mutation_path("/").is_err());
        assert_eq!(
            normalize_mutation_path("/tmp/example").unwrap(),
            "/tmp/example"
        );
    }

    #[test]
    fn uploads_use_short_per_transfer_siblings_even_for_the_same_destination() {
        assert_eq!(upload_temp_path("/demo.txt", "a"), "/.unfour-upload-a");
        assert_eq!(
            upload_temp_path("/tmp/demo.txt", "b"),
            "/tmp/.unfour-upload-b"
        );
        assert_ne!(
            upload_temp_path("/tmp/demo.txt", "a"),
            upload_temp_path("/tmp/demo.txt", "b")
        );
    }

    #[test]
    fn calculates_transfer_speed_without_dividing_by_zero() {
        assert_eq!(transfer_speed(1_048_576, 2.0), 524_288);
        assert_eq!(transfer_speed(1_024, 0.0), 1_024_000);
    }

    #[test]
    fn prunes_oldest_finished_transfers_first() {
        let finished = vec![
            ("a".to_string(), "2026-01-01T00:00:00Z".to_string()),
            ("b".to_string(), "2026-01-03T00:00:00Z".to_string()),
            ("c".to_string(), "2026-01-02T00:00:00Z".to_string()),
        ];
        assert_eq!(
            finished_transfer_ids_to_prune(finished, 2),
            vec!["a".to_string()]
        );
        assert!(finished_transfer_ids_to_prune(
            vec![("a".to_string(), "2026-01-01T00:00:00Z".to_string())],
            2
        )
        .is_empty());
    }

    #[cfg(feature = "ssh-native")]
    #[test]
    fn maps_sftp_metadata_types() {
        assert_eq!(file_kind(russh_sftp::protocol::FileType::Dir), "directory");
        assert_eq!(file_kind(russh_sftp::protocol::FileType::File), "file");
        assert_eq!(
            file_kind(russh_sftp::protocol::FileType::Symlink),
            "symlink"
        );
    }

    #[cfg(feature = "ssh-native")]
    #[test]
    fn observes_transfer_cancellation_signal() {
        let (sender, receiver) = tokio::sync::watch::channel(false);
        assert!(ensure_not_cancelled(&receiver).is_ok());
        sender.send(true).unwrap();
        assert!(matches!(
            ensure_not_cancelled(&receiver),
            Err(TransferRunError::Cancelled)
        ));
    }
}
