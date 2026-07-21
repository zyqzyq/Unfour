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
pub(super) async fn replace_local_file(
    part: &std::path::Path,
    target: &std::path::Path,
    transfer_id: &str,
    overwrite: bool,
) -> Result<(), TransferRunError> {
    let backup = local_backup_path(target, transfer_id);
    let had_target = target.exists();
    if had_target {
        if !overwrite {
            return Err(TransferRunError::Failed(
                "local target already exists".to_string(),
            ));
        }
        tokio::fs::rename(target, &backup)
            .await
            .map_err(|error| TransferRunError::Failed(error.to_string()))?;
    }
    if let Err(error) = tokio::fs::rename(part, target).await {
        if had_target {
            let _ = tokio::fs::rename(&backup, target).await;
        }
        return Err(TransferRunError::Failed(error.to_string()));
    }
    if had_target {
        let _ = tokio::fs::remove_file(backup).await;
    }
    Ok(())
}

#[cfg(feature = "ssh-native")]
pub(super) async fn finalize_remote_upload(
    sftp: &SftpSession,
    temp_path: &str,
    target_path: &str,
    transfer_id: &str,
    target_exists: bool,
) -> Result<(), TransferRunError> {
    let backup = format!("{target_path}.unfour-backup-{transfer_id}");
    if target_exists {
        sftp.rename(target_path.to_string(), backup.clone())
            .await
            .map_err(transfer_sftp_error)?;
    }
    if let Err(error) = sftp
        .rename(temp_path.to_string(), target_path.to_string())
        .await
    {
        if target_exists {
            let _ = sftp.rename(backup.clone(), target_path.to_string()).await;
        }
        return Err(transfer_sftp_error(error));
    }
    if target_exists {
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
pub(super) fn upload_temp_path(remote_path: &str) -> String {
    format!("{remote_path}.unfour-uploading")
}

#[cfg(any(feature = "ssh-native", test))]
pub(super) fn download_part_path(target: &std::path::Path) -> std::path::PathBuf {
    let mut value = target.as_os_str().to_os_string();
    value.push(".part");
    value.into()
}

#[cfg(feature = "ssh-native")]
fn local_backup_path(target: &std::path::Path, transfer_id: &str) -> std::path::PathBuf {
    let mut value = target.as_os_str().to_os_string();
    value.push(format!(".unfour-backup-{transfer_id}"));
    value.into()
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
    finished.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
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
    fn transfer_temporary_names_are_visibly_incomplete() {
        assert_eq!(
            upload_temp_path("/tmp/demo.txt"),
            "/tmp/demo.txt.unfour-uploading"
        );
        assert!(download_part_path(std::path::Path::new("demo.txt"))
            .to_string_lossy()
            .ends_with("demo.txt.part"));
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
