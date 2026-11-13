use super::super::*;
use super::convert::*;
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
        let mut db = self.db.pool().acquire().await?;
        self.recover_task_runs_on(&mut db, &workspace_id, Some(&task_id))
            .await?;
        let rows = sqlx::query_as::<_, StoredRun>(
            r#"
            SELECT id, workspace_id, task_id, connection_id, status, started_at,
                   finished_at, error_message, log_path, context_json, owner_lock
            FROM ssh_task_run
            WHERE workspace_id = ?1 AND task_id = ?2
            ORDER BY started_at DESC, id DESC
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_all(&mut *db)
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
                   finished_at, error_message, log_path, context_json, owner_lock
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
        let mut db = self.db.pool().acquire().await?;
        self.recover_task_runs_on(&mut db, &input.workspace_id, input.task_id.as_deref())
            .await?;
        let log_paths: Vec<String> = sqlx::query_scalar(
            "DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND (?2 IS NULL OR task_id = ?2)
             AND status <> 'running' RETURNING log_path",
        )
        .bind(&input.workspace_id)
        .bind(input.task_id.as_deref())
        .fetch_all(&mut *db)
        .await?;
        Ok(SshTaskCleanupResult {
            warnings: Vec::new(),
            deleted_runs: log_paths.len(),
            deleted_logs: remove_task_logs(
                log_paths.into_iter().map(PathBuf::from).collect(),
                &self.task_log_dir,
            ),
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

    #[cfg(any(feature = "ssh-native", test))]
    pub(in super::super) async fn insert_task_run(&self, run: &SshTaskRun) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, connection_id, status, started_at,
              finished_at, error_message, log_path, context_json, owner_lock
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1)
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
        .bind(
            run.context
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
        )
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    #[cfg(any(feature = "ssh-native", test))]
    pub(in super::super) async fn finish_task_run(
        &self,
        run: &SshTaskRun,
        outcome: &TaskRunOutcome,
        finished_at: &str,
    ) -> AppResult<bool> {
        let updated = sqlx::query(
            r#"
            UPDATE ssh_task_run
            SET status = ?1, finished_at = ?2, error_message = ?3
            WHERE workspace_id = ?4 AND id = ?5 AND status = 'running'
            "#,
        )
        .bind(&outcome.status)
        .bind(finished_at)
        .bind(outcome.error.as_deref())
        .bind(&run.workspace_id)
        .bind(&run.id)
        .execute(self.db.pool())
        .await?;
        Ok(updated.rows_affected() == 1)
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
            warnings: Vec::new(),
            deleted_runs: remove.len(),
            deleted_logs,
        })
    }

    pub(in super::super) async fn get_task_run(
        &self,
        workspace_id: &str,
        run_id: &str,
    ) -> AppResult<SshTaskRun> {
        let mut db = self.db.pool().acquire().await?;
        self.recover_task_runs_on(&mut db, workspace_id, None)
            .await?;
        sqlx::query_as::<_, StoredRun>(
            r#"
            SELECT id, workspace_id, task_id, connection_id, status, started_at,
                   finished_at, error_message, log_path, context_json, owner_lock
            FROM ssh_task_run WHERE workspace_id = ?1 AND id = ?2
            "#,
        )
        .bind(workspace_id)
        .bind(run_id)
        .fetch_optional(&mut *db)
        .await?
        .map(run_from_row)
        .ok_or_else(|| AppError::NotFound("SSH task run".to_string()))
    }

    /// Explicitly settle an ownerless run after the user confirms its old
    /// process exited. Modern runs, including ones owned by MCP, are excluded.
    pub async fn interrupt_legacy_task_run(
        &self,
        workspace_id: &str,
        run_id: &str,
    ) -> AppResult<SshTaskRun> {
        validate_workspace_id(workspace_id)?;
        sqlx::query(
            "UPDATE ssh_task_run SET status = 'failed', error_message = ?, finished_at = ?
                     WHERE workspace_id = ? AND id = ? AND owner_lock = 0 AND status = 'running'",
        )
        .bind(owner::INTERRUPTED)
        .bind(Utc::now().to_rfc3339())
        .bind(workspace_id)
        .bind(run_id)
        .execute(self.db.pool())
        .await?;
        let run = self.get_task_run(workspace_id, run_id).await?;
        if run.status != "failed" || run.error_message.as_deref() != Some(owner::INTERRUPTED) {
            return Err(AppError::RowConflict("SSH_TASK_RUN_NOT_REPAIRABLE".into()));
        }
        Ok(run)
    }

    #[cfg(feature = "ssh-native")]
    pub(in super::super) fn task_log_path(&self, run_id: &str) -> AppResult<PathBuf> {
        std::fs::create_dir_all(&*self.task_log_dir).map_err(|error| {
            AppError::Config(format!("failed to create task log directory: {error}"))
        })?;
        Ok(self.task_log_dir.join(format!("{run_id}.log")))
    }
}
