use super::super::*;
use super::convert::*;
use sqlx::Row;
use std::path::PathBuf;

#[cfg(feature = "ssh-native")]
const TASK_RETENTION_DAYS: i64 = 30;
#[cfg(feature = "ssh-native")]
const MAX_RUNS_PER_TASK: usize = 100;
const MAX_TASK_LOG_READ_BYTES: usize = 2 * 1024 * 1024;

impl SshService {
    pub async fn list_task_runs(
        &self,
        workspace_id: String,
        task_id: String,
    ) -> AppResult<Vec<SshTaskRun>> {
        self.get_task(&workspace_id, &task_id).await?;
        let rows = sqlx::query_as::<_, StoredRun>(
            r#"
            SELECT id, workspace_id, task_id, connection_id, status, started_at,
                   finished_at, error_message, log_path
            FROM ssh_task_run
            WHERE workspace_id = ?1 AND task_id = ?2
            ORDER BY started_at DESC, id DESC
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(run_from_row).collect())
    }

    /// Read the on-disk task run log for history replay in the UI.
    /// Path must stay under the configured task log directory.
    pub async fn read_task_run_log(
        &self,
        workspace_id: String,
        run_id: String,
    ) -> AppResult<String> {
        validate_workspace_id(&workspace_id)?;
        if run_id.trim().is_empty() {
            return Err(AppError::Validation("run id is required".to_string()));
        }
        let run = sqlx::query_as::<_, StoredRun>(
            r#"
            SELECT id, workspace_id, task_id, connection_id, status, started_at,
                   finished_at, error_message, log_path
            FROM ssh_task_run WHERE workspace_id = ?1 AND id = ?2
            "#,
        )
        .bind(&workspace_id)
        .bind(&run_id)
        .fetch_optional(self.db.pool())
        .await?
        .map(run_from_row)
        .ok_or_else(|| AppError::NotFound("SSH task run".to_string()))?;

        let path = PathBuf::from(&run.log_path);
        if !safe_task_log_path(&path, Some(self.task_log_dir.as_path())) {
            return Err(AppError::Validation(
                "SSH task log path is outside the allowed directory".to_string(),
            ));
        }
        if !path.exists() {
            return Ok(String::new());
        }
        let bytes = std::fs::read(&path)
            .map_err(|error| AppError::Config(format!("failed to read SSH task log: {error}")))?;
        let capped = if bytes.len() > MAX_TASK_LOG_READ_BYTES {
            &bytes[..MAX_TASK_LOG_READ_BYTES]
        } else {
            &bytes[..]
        };
        let mut text = String::from_utf8_lossy(capped).into_owned();
        if bytes.len() > MAX_TASK_LOG_READ_BYTES {
            text.push_str("\n[log truncated for display]\n");
        }
        Ok(text)
    }

    pub async fn clear_task_runs(
        &self,
        input: SshTaskCleanupInput,
    ) -> AppResult<SshTaskCleanupResult> {
        validate_workspace_id(&input.workspace_id)?;
        if let Some(task_id) = input.task_id.as_deref() {
            self.get_task(&input.workspace_id, task_id).await?;
        }
        let log_paths = self
            .task_log_paths(&input.workspace_id, input.task_id.as_deref())
            .await?;
        let result = if let Some(task_id) = input.task_id.as_deref() {
            sqlx::query(
                "DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2 AND status <> 'running'",
            )
                .bind(&input.workspace_id)
                .bind(task_id)
                .execute(self.db.pool())
                .await?
        } else {
            sqlx::query("DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND status <> 'running'")
                .bind(&input.workspace_id)
                .execute(self.db.pool())
                .await?
        };
        Ok(SshTaskCleanupResult {
            deleted_runs: result.rows_affected() as usize,
            deleted_logs: remove_task_logs(log_paths, &self.task_log_dir),
        })
    }

    #[cfg(any(feature = "ssh-native", test))]
    pub(in super::super) async fn record_task_connection_use(
        &self,
        workspace_id: &str,
        task_id: &str,
        connection_id: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO ssh_task_local_binding (
              task_id, workspace_id, default_connection_id, last_used_connection_id,
              created_at, updated_at
            ) VALUES (?1, ?2, NULL, ?3, ?4, ?4)
            ON CONFLICT(task_id) DO UPDATE SET
              last_used_connection_id = excluded.last_used_connection_id,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(task_id)
        .bind(workspace_id)
        .bind(connection_id)
        .bind(&now)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    #[cfg(feature = "ssh-native")]
    pub(in super::super) async fn insert_task_run(&self, run: &SshTaskRun) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, connection_id, status, started_at,
              finished_at, error_message, log_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&run.id)
        .bind(&run.workspace_id)
        .bind(&run.task_id)
        .bind(run.connection_id.as_deref())
        .bind(&run.status)
        .bind(&run.started_at)
        .bind(run.finished_at.as_deref())
        .bind(run.error_message.as_deref())
        .bind(&run.log_path)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    #[cfg(feature = "ssh-native")]
    pub(in super::super) async fn finish_task_run(
        &self,
        workspace_id: &str,
        run_id: &str,
        status: &str,
        error_message: Option<&str>,
    ) -> AppResult<SshTaskRun> {
        let finished_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE ssh_task_run
            SET status = ?1, finished_at = ?2, error_message = ?3
            WHERE workspace_id = ?4 AND id = ?5
            "#,
        )
        .bind(status)
        .bind(&finished_at)
        .bind(error_message)
        .bind(workspace_id)
        .bind(run_id)
        .execute(self.db.pool())
        .await?;
        self.get_task_run(workspace_id, run_id).await
    }

    #[cfg(feature = "ssh-native")]
    pub(in super::super) async fn cleanup_task_retention(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> AppResult<SshTaskCleanupResult> {
        let runs = self
            .list_task_runs(workspace_id.to_string(), task_id.to_string())
            .await?;
        let cutoff = Utc::now() - chrono::Duration::days(TASK_RETENTION_DAYS);
        let mut completed_seen = 0_usize;
        let mut remove = Vec::new();
        for run in runs {
            if run.status == "running" {
                continue;
            }
            let older_than_cutoff = chrono::DateTime::parse_from_rfc3339(&run.started_at)
                .map(|started| started.with_timezone(&Utc) < cutoff)
                .unwrap_or(false);
            if older_than_cutoff || completed_seen >= MAX_RUNS_PER_TASK {
                remove.push(run);
            }
            completed_seen += 1;
        }
        let mut deleted_logs = 0;
        for run in &remove {
            sqlx::query("DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND id = ?2")
                .bind(workspace_id)
                .bind(&run.id)
                .execute(self.db.pool())
                .await?;
            deleted_logs +=
                remove_task_logs(vec![PathBuf::from(&run.log_path)], &self.task_log_dir);
        }
        Ok(SshTaskCleanupResult {
            deleted_runs: remove.len(),
            deleted_logs,
        })
    }

    #[cfg(feature = "ssh-native")]
    pub(in super::super) async fn get_task_run(
        &self,
        workspace_id: &str,
        run_id: &str,
    ) -> AppResult<SshTaskRun> {
        sqlx::query_as::<_, StoredRun>(
            r#"
            SELECT id, workspace_id, task_id, connection_id, status, started_at,
                   finished_at, error_message, log_path
            FROM ssh_task_run WHERE workspace_id = ?1 AND id = ?2
            "#,
        )
        .bind(workspace_id)
        .bind(run_id)
        .fetch_optional(self.db.pool())
        .await?
        .map(run_from_row)
        .ok_or_else(|| AppError::NotFound("SSH task run".to_string()))
    }

    pub(super) async fn task_log_paths(
        &self,
        workspace_id: &str,
        task_id: Option<&str>,
    ) -> AppResult<Vec<PathBuf>> {
        let rows = if let Some(task_id) = task_id {
            sqlx::query(
                "SELECT log_path FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2 AND status <> 'running'",
            )
            .bind(workspace_id)
            .bind(task_id)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query(
                "SELECT log_path FROM ssh_task_run WHERE workspace_id = ?1 AND status <> 'running'",
            )
            .bind(workspace_id)
            .fetch_all(self.db.pool())
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|row| PathBuf::from(row.get::<String, _>("log_path")))
            .collect())
    }

    #[cfg(feature = "ssh-native")]
    pub(in super::super) fn task_log_path(&self, run_id: &str) -> AppResult<PathBuf> {
        std::fs::create_dir_all(&*self.task_log_dir).map_err(|error| {
            AppError::Config(format!("failed to create task log directory: {error}"))
        })?;
        Ok(self.task_log_dir.join(format!("{run_id}.log")))
    }
}
