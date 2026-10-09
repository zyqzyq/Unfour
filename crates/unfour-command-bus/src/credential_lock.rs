use std::fs::{File, OpenOptions};

use unfour_core::{AppError, AppResult};

use crate::CommandBus;

impl CommandBus {
    /// The same database path gives desktop and MCP the same OS lock. Hold it
    /// before journaling until publication or rollback finishes. Unlike a PID
    /// or time lease, the OS releases this lock only when the owner drops/exits.
    pub(crate) async fn credential_stage_guard(&self) -> AppResult<Option<File>> {
        let path = self
            .db
            .pool()
            .connect_options()
            .get_filename()
            .to_path_buf();
        if path == std::path::Path::new(":memory:") {
            return Ok(None);
        }
        let path = std::fs::canonicalize(path)?;
        let mut lock_path = path.into_os_string();
        lock_path.push(".credentials.lock");
        tokio::task::spawn_blocking(move || {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(lock_path)?;
            file.lock()?;
            Ok(Some(file))
        })
        .await
        .map_err(|_| AppError::Config("CREDENTIAL_LIFECYCLE_LOCK_UNAVAILABLE".into()))?
    }
}
