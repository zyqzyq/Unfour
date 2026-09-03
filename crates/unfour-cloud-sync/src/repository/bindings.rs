//! Account activation, workspace binding lifecycle, generation fencing and diagnostics.
//! No network or Core writes; caller-owned transactions remain caller-owned.

use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;

use super::SyncRepository;
use crate::{SyncBinding, SyncError, SyncStatus};

const DIAGNOSTIC_HISTORY_LIMIT: i64 = 200;

impl SyncRepository {
    const BINDING_COLUMNS: &'static str = r#"
        account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
        sync_enabled, state, initial_cursor, initial_total, initial_confirmed, initialization_checkpoint,
        ssh_task_v3_bootstrap_state, connection_v4_bootstrap_state, generation,
        last_success_at, last_error, consecutive_failure_count
    "#;

    pub async fn binding(
        &self,
        account_id: &str,
        workspace_id: &str,
    ) -> Result<Option<SyncBinding>, SyncError> {
        let sql = format!("SELECT {} FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND local_workspace_id = ?2", Self::BINDING_COLUMNS);
        sqlx::query_as::<_, SyncBinding>(&sql)
            .bind(account_id)
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn binding_by_cloud(
        &self,
        account_id: &str,
        cloud_workspace_id: &str,
    ) -> Result<Option<SyncBinding>, SyncError> {
        let sql = format!("SELECT {} FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND cloud_workspace_id = ?2", Self::BINDING_COLUMNS);
        sqlx::query_as::<_, SyncBinding>(&sql)
            .bind(account_id)
            .bind(cloud_workspace_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn enabled_bindings(&self, account_id: &str) -> Result<Vec<SyncBinding>, SyncError> {
        let sql = format!("SELECT {} FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND sync_enabled = 1 AND state <> 'paused' AND EXISTS (SELECT 1 FROM cloud_sync_account_settings WHERE account_id = ?1 AND sync_enabled = 1) ORDER BY created_at, local_workspace_id", Self::BINDING_COLUMNS);
        sqlx::query_as::<_, SyncBinding>(&sql)
            .bind(account_id)
            .fetch_all(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn pause_account(
        &self,
        account_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        sqlx::query(
            "UPDATE cloud_sync_workspace_bindings SET sync_enabled = 0, state = 'paused', generation = generation + 1, updated_at = ?1 WHERE account_id = ?2",
        )
        .bind(now.to_rfc3339()).bind(account_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn activate_account(
        &self,
        account_id: &str,
        generation: u64,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let now = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT OR IGNORE INTO cloud_sync_account_settings (account_id, sync_enabled, updated_at) VALUES (?1, 0, ?2)",
        )
        .bind(account_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        let previous = sqlx::query_scalar::<_, Option<String>>(
            "SELECT active_account_id FROM cloud_sync_runtime_context WHERE singleton = 1",
        )
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        if previous.as_deref() != Some(account_id) {
            sqlx::query(
                "UPDATE cloud_sync_workspace_bindings SET sync_enabled = 0, state = 'paused', generation = generation + 1, updated_at = ?1 WHERE account_id <> ?2 AND sync_enabled = 1",
            )
            .bind(&now)
            .bind(account_id)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            r#"INSERT INTO cloud_sync_runtime_context (singleton, active_account_id, generation, updated_at)
               VALUES (1, ?1, ?2, ?3)
               ON CONFLICT(singleton) DO UPDATE SET
                 active_account_id = excluded.active_account_id,
                 generation = excluded.generation,
                 updated_at = excluded.updated_at"#,
        )
        .bind(account_id)
        .bind(generation as i64)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn global_sync_enabled(&self, account_id: &str) -> Result<bool, SyncError> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT sync_enabled FROM cloud_sync_account_settings WHERE account_id = ?1",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(true))
    }

    pub async fn set_global_sync_enabled(
        &self,
        account_id: &str,
        enabled: bool,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let now = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO cloud_sync_account_settings (account_id, sync_enabled, updated_at)
               VALUES (?1, ?2, ?3)
               ON CONFLICT(account_id) DO UPDATE SET
                 sync_enabled = excluded.sync_enabled,
                 updated_at = excluded.updated_at"#,
        )
        .bind(account_id)
        .bind(enabled)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE cloud_sync_workspace_bindings SET generation = generation + 1, updated_at = ?1 WHERE account_id = ?2 AND sync_enabled = 1",
        )
        .bind(&now)
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn deactivate_active_account(&self, now: DateTime<Utc>) -> Result<(), SyncError> {
        let now = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"UPDATE cloud_sync_workspace_bindings
               SET sync_enabled = 0, state = 'paused', generation = generation + 1, updated_at = ?1
               WHERE account_id = (
                 SELECT active_account_id FROM cloud_sync_runtime_context WHERE singleton = 1
               )"#,
        )
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE cloud_sync_runtime_context SET active_account_id = NULL, generation = generation + 1, updated_at = ?1 WHERE singleton = 1",
        )
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn claim_generation(
        &self,
        account_id: &str,
        workspace_id: &str,
        generation: u64,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let changed = sqlx::query(
            "UPDATE cloud_sync_workspace_bindings SET generation = ?1, updated_at = ?2 WHERE account_id = ?3 AND local_workspace_id = ?4 AND sync_enabled = 1 AND state <> 'paused' AND EXISTS (SELECT 1 FROM cloud_sync_account_settings WHERE account_id = ?3 AND sync_enabled = 1)",
        )
        .bind(generation as i64)
        .bind(now.to_rfc3339())
        .bind(account_id)
        .bind(workspace_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if changed == 0 {
            return Err(SyncError::NotFound);
        }
        Ok(())
    }

    pub async fn set_enabled(
        &self,
        account_id: &str,
        workspace_id: &str,
        enabled: bool,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let state = if enabled { "error" } else { "paused" };
        let changed = sqlx::query(
            "UPDATE cloud_sync_workspace_bindings SET sync_enabled = ?1, state = CASE WHEN ?1 THEN CASE WHEN initial_confirmed >= initial_total THEN 'reconciling' ELSE 'uploading' END ELSE ?2 END, generation = generation + 1, updated_at = ?3 WHERE account_id = ?4 AND local_workspace_id = ?5",
        )
        .bind(enabled).bind(state).bind(now.to_rfc3339()).bind(account_id).bind(workspace_id)
        .execute(&self.pool).await?.rows_affected();
        if changed == 0 {
            return Err(SyncError::NotFound);
        }
        Ok(())
    }

    pub async fn api_v2_bootstrap_completed(
        &self,
        account_id: &str,
        workspace_id: &str,
    ) -> Result<bool, SyncError> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT api_v2_bootstrap_state = 'completed' FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND local_workspace_id = ?2",
        )
        .bind(account_id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(false))
    }

    pub async fn status(
        &self,
        account_id: &str,
        workspace_id: &str,
        running: bool,
    ) -> Result<SyncStatus, SyncError> {
        let binding = self.binding(account_id, workspace_id).await?;
        let counts: (i64, i64, i64, i64) = sqlx::query_as(
            r#"SELECT
                 COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN status = 'uncertain' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN status = 'in_flight' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN status = 'dead' THEN 1 ELSE 0 END), 0)
               FROM cloud_sync_outbox
               WHERE account_id = ?1 AND local_workspace_id = ?2"#,
        )
        .bind(account_id)
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;
        let conflict_count = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM cloud_sync_entity_state AS state
               JOIN cloud_sync_workspace_bindings AS binding
                 ON binding.account_id = state.account_id AND binding.cloud_workspace_id = state.cloud_workspace_id
               WHERE binding.account_id = ?1 AND binding.local_workspace_id = ?2 AND state.sync_status = 'conflict'"#,
        ).bind(account_id).bind(workspace_id).fetch_one(&self.pool).await?;
        Ok(SyncStatus {
            binding,
            pending_count: counts.0,
            uncertain_count: counts.1,
            in_flight_count: counts.2,
            dead_count: counts.3,
            dead_letters: self.dead_letters(account_id, workspace_id).await?,
            conflict_count,
            running,
        })
    }

    pub async fn diagnostics(
        &self,
        account_id: &str,
        workspace_id: &str,
    ) -> Result<Option<crate::SyncDiagnostics>, SyncError> {
        let Some(binding) = self.binding(account_id, workspace_id).await? else {
            return Ok(None);
        };
        let (pending_outbox_count, dead_outbox_count, next_retry_at): (i64, i64, Option<String>) =
            sqlx::query_as(
                r#"SELECT COUNT(*),
                      COALESCE(SUM(CASE WHEN status = 'dead' THEN 1 ELSE 0 END), 0),
                      MIN(CASE WHEN status <> 'dead' THEN next_attempt_at END)
               FROM cloud_sync_outbox
               WHERE account_id = ?1 AND local_workspace_id = ?2"#,
            )
            .bind(account_id)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        let last_push_at = sqlx::query_scalar::<_, Option<String>>(
            r#"SELECT MAX(finished_at) FROM cloud_sync_attempts
               WHERE account_id = ?1 AND cloud_workspace_id = ?2
                 AND status IN ('applied', 'no_op')"#,
        )
        .bind(account_id)
        .bind(&binding.cloud_workspace_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(Some(crate::SyncDiagnostics {
            local_workspace_id: binding.local_workspace_id,
            remote_workspace_id: binding.cloud_workspace_id,
            last_push_at,
            last_pull_at: binding.last_success_at,
            pending_outbox_count,
            dead_outbox_count,
            dead_letters: self.dead_letters(account_id, workspace_id).await?,
            pull_cursor: binding.last_pulled_cursor,
            last_error_code: binding.last_error,
            consecutive_failure_count: binding.consecutive_failure_count,
            next_retry_at,
        }))
    }

    pub(crate) async fn assert_binding_generation_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
    ) -> Result<(), SyncError> {
        let generation: Option<i64> = sqlx::query_scalar(
            "SELECT generation FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND local_workspace_id = ?2 AND sync_enabled = 1 AND state <> 'paused' AND EXISTS (SELECT 1 FROM cloud_sync_account_settings WHERE account_id = ?1 AND sync_enabled = 1)",
        )
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .fetch_optional(&mut *connection)
        .await?;
        (generation == Some(binding.generation))
            .then_some(())
            .ok_or(SyncError::AccountChanged)
    }

    pub async fn set_binding_state(
        &self,
        binding: &SyncBinding,
        state: &str,
        error: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let changed = sqlx::query(
            "UPDATE cloud_sync_workspace_bindings SET state = ?1, last_error = ?2, consecutive_failure_count = CASE WHEN ?1 = 'active' AND ?2 IS NULL THEN 0 ELSE consecutive_failure_count END, updated_at = ?3 WHERE account_id = ?4 AND local_workspace_id = ?5 AND generation = ?6",
        ).bind(state).bind(error).bind(now.to_rfc3339()).bind(&binding.account_id)
         .bind(&binding.local_workspace_id).bind(binding.generation).execute(&self.pool).await?.rows_affected();
        if changed != 1 {
            return Err(SyncError::AccountChanged);
        }
        Ok(())
    }

    pub async fn record_error(
        &self,
        account_id: &str,
        workspace_id: &str,
        generation: u64,
        code: &str,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        sqlx::query(
            "UPDATE cloud_sync_workspace_bindings SET state = CASE WHEN state = 'conflict' THEN state ELSE 'error' END, last_error = ?1, consecutive_failure_count = consecutive_failure_count + 1, updated_at = ?2 WHERE account_id = ?3 AND local_workspace_id = ?4 AND generation = ?5",
        ).bind(code).bind(now.to_rfc3339()).bind(account_id).bind(workspace_id).bind(generation as i64).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn local_workspace_name(
        &self,
        workspace_id: &str,
    ) -> Result<Option<String>, SyncError> {
        sqlx::query_scalar("SELECT name FROM workspaces WHERE id = ?1")
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub(super) async fn record_diagnostic_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        cloud_workspace_id: Option<&str>,
        category: &str,
        error_code: &str,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            "INSERT INTO cloud_sync_diagnostics (account_id, cloud_workspace_id, category, error_code, entity_type, entity_id, occurred_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        ).bind(account_id).bind(cloud_workspace_id).bind(category).bind(error_code).bind(entity_type).bind(entity_id).bind(now).execute(&mut *connection).await?;
        sqlx::query(
            "DELETE FROM cloud_sync_diagnostics WHERE account_id = ?1 AND id NOT IN (SELECT id FROM cloud_sync_diagnostics WHERE account_id = ?1 ORDER BY id DESC LIMIT ?2)",
        ).bind(account_id).bind(DIAGNOSTIC_HISTORY_LIMIT).execute(&mut *connection).await?;
        Ok(())
    }
}
