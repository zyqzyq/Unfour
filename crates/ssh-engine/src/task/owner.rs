use std::fs::{OpenOptions, TryLockError};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
#[cfg(any(feature = "ssh-native", test))]
use std::{fs::File, io};

use sqlx::SqliteConnection;
use unfour_core::AppResult;

use super::SshService;

pub(super) const INTERRUPTED: &str = "SSH_TASK_RUN_INTERRUPTED";

/// Hold a dedicated file lock before publishing the running row and until
/// SQLite acknowledges the final status. A separate file keeps logs readable
/// during execution on Windows, where exclusive file locks also deny reads.
#[cfg(any(feature = "ssh-native", test))]
pub(super) struct RunOwner {
    file: Option<File>,
    path: PathBuf,
}

#[cfg(any(feature = "ssh-native", test))]
impl RunOwner {
    pub(super) fn acquire(path: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(path.parent().expect("run owner directory"))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        if let Err(error) = file.try_lock() {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(io::Error::from(error).into());
        }
        Ok(Self {
            file: Some(file),
            path,
        })
    }
}

#[cfg(any(feature = "ssh-native", test))]
impl Drop for RunOwner {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

impl SshService {
    pub(super) fn task_owner_file(&self, id: &str) -> AppResult<PathBuf> {
        let id = uuid::Uuid::parse_str(id)
            .map_err(|_| unfour_core::AppError::Config("Invalid SSH task run ID".into()))?;
        let options = self.db.pool().connect_options();
        let database = options.get_filename();
        // A separate log directory can be configured per adapter. Processes
        // opening the same durable database must still probe the same locks.
        let directory = if database == Path::new(":memory:") {
            self.task_log_dir.as_ref().clone()
        } else {
            let mut dir = std::fs::canonicalize(database)?.into_os_string();
            dir.push(".ssh-task-owners");
            dir.into()
        };
        Ok(directory.join(format!("{id}.owner")))
    }

    pub(super) async fn recover_task_runs_on(
        &self,
        db: &mut SqliteConnection,
        workspace_id: &str,
        task_id: Option<&str>,
    ) -> AppResult<()> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM ssh_task_run WHERE workspace_id = ? AND status = 'running'
             AND owner_lock = 1 AND (? IS NULL OR task_id = ?)",
        )
        .bind(workspace_id)
        .bind(task_id)
        .bind(task_id)
        .fetch_all(&mut *db)
        .await?;
        for id in rows {
            let path = self.task_owner_file(&id)?;
            let owner = match OpenOptions::new().read(true).write(true).open(&path) {
                Ok(file) => match file.try_lock() {
                    Ok(()) => Some(file),
                    Err(TryLockError::WouldBlock) => continue,
                    Err(TryLockError::Error(error)) => return Err(error.into()),
                },
                Err(error) if error.kind() == ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            // A concurrent successful completion can precede our lock probe.
            sqlx::query("UPDATE ssh_task_run SET status = 'failed', error_message = ?, finished_at = ?
                         WHERE workspace_id = ? AND id = ? AND status = 'running' AND owner_lock = 1")
                .bind(INTERRUPTED)
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(workspace_id).bind(&id)
                .execute(&mut *db).await?;
            if let Some(file) = owner {
                drop(file);
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(())
    }
}
