//! Bounded safe diagnostics plus persisted retry scheduling metadata.

use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;

use super::SyncRepository;
use crate::{RemoteSyncProblem, SyncDiagnosticEvent, SyncDiagnostics, SyncError, SyncPhase};

const DIAGNOSTIC_HISTORY_LIMIT: i64 = 200;

impl SyncRepository {
    pub async fn diagnostics(
        &self,
        account_id: &str,
        workspace_id: &str,
    ) -> Result<Option<SyncDiagnostics>, SyncError> {
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
        let recent_events = sqlx::query_as::<_, SyncDiagnosticEvent>(
            r#"SELECT source, category, error_code, request_id, http_status, phase,
                      operation_id, operation_index, entity_type, entity_id, occurred_at
               FROM cloud_sync_diagnostics
               WHERE account_id = ?1 AND cloud_workspace_id = ?2
               ORDER BY id DESC LIMIT 20"#,
        )
        .bind(account_id)
        .bind(&binding.cloud_workspace_id)
        .fetch_all(&self.pool)
        .await?;
        let last_remote: Option<(String, Option<String>, Option<i64>, Option<String>)> =
            sqlx::query_as(
                r#"SELECT error_code, request_id, http_status, phase
                   FROM cloud_sync_diagnostics
                   WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND source = 'remote'
                   ORDER BY id DESC LIMIT 1"#,
            )
            .bind(account_id)
            .bind(&binding.cloud_workspace_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(Some(SyncDiagnostics {
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
            last_server_error_code: last_remote.as_ref().map(|value| value.0.clone()),
            last_server_request_id: last_remote.as_ref().and_then(|value| value.1.clone()),
            last_http_status: last_remote.as_ref().and_then(|value| value.2),
            last_sync_phase: last_remote.and_then(|value| value.3),
            recent_events,
        }))
    }

    pub async fn record_remote_problem(
        &self,
        account_id: &str,
        cloud_workspace_id: Option<&str>,
        problem: &RemoteSyncProblem,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let mut tx = self.pool.begin().await?;
        let now = now.to_rfc3339();
        Self::record_diagnostic_context_on(
            &mut tx,
            account_id,
            cloud_workspace_id,
            problem.diagnostic_category(),
            &problem.server_error_code,
            Some("remote"),
            problem.request_id.as_deref(),
            problem.http_status.map(i64::from),
            Some(problem.phase),
            problem.operation_id.as_deref(),
            problem.operation_index,
            problem.entity_type.as_deref(),
            problem.entity_id.as_deref(),
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn record_local_diagnostic(
        &self,
        account_id: &str,
        cloud_workspace_id: Option<&str>,
        category: &str,
        error_code: &str,
        phase: SyncPhase,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let mut tx = self.pool.begin().await?;
        let now = now.to_rfc3339();
        Self::record_diagnostic_context_on(
            &mut tx,
            account_id,
            cloud_workspace_id,
            category,
            error_code,
            Some("local"),
            None,
            None,
            Some(phase),
            None,
            None,
            None,
            None,
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn prepare_manual_retry(
        &self,
        account_id: &str,
        workspace_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"UPDATE cloud_sync_workspace_bindings SET
                 state = CASE WHEN initial_confirmed < initial_total THEN 'uploading' ELSE 'reconciling' END,
                 last_error = NULL, updated_at = ?1
               WHERE account_id = ?2 AND local_workspace_id = ?3 AND state = 'error'
                 AND NOT EXISTS (
                   SELECT 1 FROM cloud_sync_outbox
                   WHERE account_id = ?2 AND local_workspace_id = ?3 AND status = 'dead'
                 )
                 AND NOT EXISTS (
                   SELECT 1 FROM cloud_sync_entity_state AS entity
                   JOIN cloud_sync_workspace_bindings AS binding
                     ON binding.account_id = entity.account_id
                    AND binding.cloud_workspace_id = entity.cloud_workspace_id
                   WHERE binding.account_id = ?2 AND binding.local_workspace_id = ?3
                     AND entity.sync_status = 'conflict'
                 )"#,
        )
        .bind(now.to_rfc3339())
        .bind(account_id)
        .bind(workspace_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn next_scheduled_retry(
        &self,
    ) -> Result<Option<(String, String, i64, String)>, SyncError> {
        sqlx::query_as(
            r#"SELECT outbox.account_id, outbox.local_workspace_id,
                      context.generation, MIN(outbox.next_attempt_at)
               FROM cloud_sync_outbox AS outbox
               JOIN cloud_sync_runtime_context AS context
                 ON context.singleton = 1 AND context.active_account_id = outbox.account_id
               JOIN cloud_sync_workspace_bindings AS binding
                 ON binding.account_id = outbox.account_id
                AND binding.local_workspace_id = outbox.local_workspace_id
                AND binding.cloud_workspace_id = outbox.cloud_workspace_id
               JOIN cloud_sync_account_settings AS settings
                 ON settings.account_id = outbox.account_id AND settings.sync_enabled = 1
               JOIN cloud_sync_workspace_ownership AS owner
                 ON owner.local_workspace_id = binding.local_workspace_id
                AND owner.account_id = binding.account_id
                AND owner.cloud_workspace_id = binding.cloud_workspace_id
               WHERE outbox.status IN ('pending', 'uncertain')
                 AND outbox.next_attempt_at IS NOT NULL
                 AND binding.sync_enabled = 1 AND binding.state <> 'paused'
               GROUP BY outbox.account_id, outbox.local_workspace_id, context.generation
               ORDER BY MIN(outbox.next_attempt_at), outbox.local_workspace_id
               LIMIT 1"#,
        )
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
        Self::record_diagnostic_context_on(
            connection,
            account_id,
            cloud_workspace_id,
            category,
            error_code,
            Some("domain"),
            None,
            None,
            None,
            None,
            None,
            entity_type,
            entity_id,
            now,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_diagnostic_context_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        cloud_workspace_id: Option<&str>,
        category: &str,
        error_code: &str,
        source: Option<&str>,
        request_id: Option<&str>,
        http_status: Option<i64>,
        phase: Option<SyncPhase>,
        operation_id: Option<&str>,
        operation_index: Option<i64>,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"INSERT INTO cloud_sync_diagnostics (
                 account_id, cloud_workspace_id, category, error_code,
                 source, request_id, http_status, phase, operation_id, operation_index,
                 entity_type, entity_id, occurred_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
        )
        .bind(account_id)
        .bind(cloud_workspace_id)
        .bind(category)
        .bind(error_code)
        .bind(source)
        .bind(request_id)
        .bind(http_status)
        .bind(phase.map(SyncPhase::as_str))
        .bind(operation_id)
        .bind(operation_index)
        .bind(entity_type)
        .bind(entity_id)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        sqlx::query(
            "DELETE FROM cloud_sync_diagnostics WHERE account_id = ?1 AND id NOT IN (SELECT id FROM cloud_sync_diagnostics WHERE account_id = ?1 ORDER BY id DESC LIMIT ?2)",
        )
        .bind(account_id)
        .bind(DIAGNOSTIC_HISTORY_LIMIT)
        .execute(&mut *connection)
        .await?;
        Ok(())
    }
}
