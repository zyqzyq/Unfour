//! Persist network attempt outcomes, leases, retries and initial-upload acknowledgements.
//! A successful push updates entity versions, never the pull cursor.

use chrono::{DateTime, Duration, Utc};
use sqlx::SqliteConnection;

use super::SyncRepository;
use crate::{Clock, OutboxEntry, PushResult, PushResultStatus, SyncBinding, SyncError};

const LEASE_DURATION: Duration = Duration::seconds(45);

impl SyncRepository {
    pub async fn recover_expired_leases(
        &self,
        account_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let now = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"UPDATE cloud_sync_attempts SET status = 'uncertain', lease_owner = NULL,
                 lease_expires_at = NULL, error_code = 'lease_expired'
               WHERE account_id = ?1 AND status = 'in_flight' AND lease_expires_at <= ?2"#,
        )
        .bind(account_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"UPDATE cloud_sync_outbox SET status = 'pending', lease_owner = NULL,
                 lease_started_at = NULL, lease_expires_at = NULL, next_attempt_at = ?2,
                 last_error = 'lease_expired', updated_at = ?2
               WHERE account_id = ?1 AND status = 'in_flight' AND lease_expires_at <= ?2"#,
        )
        .bind(account_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_in_flight(
        &self,
        entries: &[OutboxEntry],
        lease_owner: &str,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let started = now.to_rfc3339();
        let expires = (now + LEASE_DURATION).to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for entry in entries {
            sqlx::query(
                r#"INSERT INTO cloud_sync_attempts (
                     account_id, cloud_workspace_id, operation_id, entity_type, entity_id,
                     base_version, status, lease_owner, started_at, lease_expires_at
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in_flight', ?7, ?8, ?9)
                   ON CONFLICT(account_id, cloud_workspace_id, operation_id) DO UPDATE SET
                     status = 'in_flight', lease_owner = excluded.lease_owner,
                     started_at = excluded.started_at, lease_expires_at = excluded.lease_expires_at,
                     finished_at = NULL, error_code = NULL"#,
            )
            .bind(&entry.account_id)
            .bind(&entry.cloud_workspace_id)
            .bind(&entry.operation_id)
            .bind(&entry.entity_type)
            .bind(&entry.entity_id)
            .bind(entry.base_version)
            .bind(lease_owner)
            .bind(&started)
            .bind(&expires)
            .execute(&mut *tx)
            .await?;
            let changed = sqlx::query(
                r#"UPDATE cloud_sync_outbox SET status = 'in_flight', lease_owner = ?1,
                     lease_started_at = ?2, lease_expires_at = ?3, updated_at = ?2
                   WHERE account_id = ?4 AND operation_id = ?5 AND status IN ('pending', 'uncertain')"#,
            ).bind(lease_owner).bind(&started).bind(&expires).bind(&entry.account_id)
             .bind(&entry.operation_id).execute(&mut *tx).await?.rows_affected();
            if changed != 1 {
                return Err(SyncError::Storage);
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_uncertain(
        &self,
        entries: &[OutboxEntry],
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let next = (now + Duration::seconds(5)).to_rfc3339();
        let now = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for entry in entries {
            sqlx::query(
                "UPDATE cloud_sync_attempts SET status = 'uncertain', lease_owner = NULL, lease_expires_at = NULL, error_code = 'result_unknown' WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND operation_id = ?3",
            ).bind(&entry.account_id).bind(&entry.cloud_workspace_id).bind(&entry.operation_id).execute(&mut *tx).await?;
            sqlx::query(
                r#"UPDATE cloud_sync_outbox SET status = 'uncertain', attempt_count = attempt_count + 1,
                     next_attempt_at = ?1, lease_owner = NULL, lease_started_at = NULL,
                     lease_expires_at = NULL, last_error = 'result_unknown', updated_at = ?2
                   WHERE account_id = ?3 AND operation_id = ?4 AND status = 'in_flight'"#,
            ).bind(&next).bind(&now).bind(&entry.account_id).bind(&entry.operation_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_not_sent(
        &self,
        entries: &[OutboxEntry],
        error_code: &str,
        retryable: bool,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let now_text = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for entry in entries {
            let exponent = entry.attempt_count.clamp(0, 8) as u32;
            let jitter_ms = entry.operation_id.bytes().fold(0_u64, |value, byte| {
                value.wrapping_mul(31).wrapping_add(byte as u64)
            }) % 1_000;
            let next = now
                + Duration::seconds((1_i64 << exponent).min(300))
                + Duration::milliseconds(jitter_ms as i64);
            sqlx::query(
                "UPDATE cloud_sync_attempts SET status = 'failed', finished_at = ?1, lease_owner = NULL, lease_expires_at = NULL, error_code = ?2 WHERE account_id = ?3 AND cloud_workspace_id = ?4 AND operation_id = ?5",
            ).bind(&now_text).bind(error_code).bind(&entry.account_id).bind(&entry.cloud_workspace_id).bind(&entry.operation_id).execute(&mut *tx).await?;
            sqlx::query(
                r#"UPDATE cloud_sync_outbox SET status = ?1, attempt_count = attempt_count + 1,
                     next_attempt_at = ?2, lease_owner = NULL, lease_started_at = NULL,
                     lease_expires_at = NULL, last_error = ?3, updated_at = ?4
                   WHERE account_id = ?5 AND operation_id = ?6"#,
            )
            .bind(if retryable { "pending" } else { "dead" })
            .bind(retryable.then(|| next.to_rfc3339()))
            .bind(error_code)
            .bind(&now_text)
            .bind(&entry.account_id)
            .bind(&entry.operation_id)
            .execute(&mut *tx)
            .await?;
            Self::record_diagnostic_on(
                &mut tx,
                &entry.account_id,
                Some(&entry.cloud_workspace_id),
                if retryable {
                    "retryable"
                } else {
                    "dead_letter"
                },
                error_code,
                Some(&entry.entity_type),
                Some(&entry.entity_id),
                &now_text,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// A permanent operation-level batch failure is still an atomic server
    /// rollback. Persist the one operation the server identified as dead and
    /// return every other operation to a clean pending state in one local
    /// transaction. This intentionally does not reuse `mark_not_sent` for the
    /// whole batch: doing so recreates one dead letter per rolled-back row.
    pub async fn mark_batch_permanent_failure(
        &self,
        entries: &[OutboxEntry],
        failed_operation_id: &str,
        error_code: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, SyncError> {
        if !entries
            .iter()
            .any(|entry| entry.operation_id == failed_operation_id)
        {
            return Ok(false);
        }
        let now_text = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for entry in entries {
            let failed = entry.operation_id == failed_operation_id;
            let persisted_error = if failed {
                error_code
            } else {
                "batch_rolled_back"
            };
            sqlx::query(
                r#"UPDATE cloud_sync_attempts SET status = 'failed', finished_at = ?1,
                     lease_owner = NULL, lease_expires_at = NULL, error_code = ?2
                   WHERE account_id = ?3 AND cloud_workspace_id = ?4 AND operation_id = ?5"#,
            )
            .bind(&now_text)
            .bind(persisted_error)
            .bind(&entry.account_id)
            .bind(&entry.cloud_workspace_id)
            .bind(&entry.operation_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"UPDATE cloud_sync_outbox SET status = ?1,
                     attempt_count = attempt_count + 1,
                     next_attempt_at = NULL, lease_owner = NULL,
                     lease_started_at = NULL, lease_expires_at = NULL,
                     last_error = ?2, updated_at = ?3
                   WHERE account_id = ?4 AND operation_id = ?5"#,
            )
            .bind(if failed { "dead" } else { "pending" })
            .bind(failed.then_some(error_code))
            .bind(&now_text)
            .bind(&entry.account_id)
            .bind(&entry.operation_id)
            .execute(&mut *tx)
            .await?;
            Self::record_diagnostic_on(
                &mut tx,
                &entry.account_id,
                Some(&entry.cloud_workspace_id),
                if failed { "dead_letter" } else { "retryable" },
                persisted_error,
                Some(&entry.entity_type),
                Some(&entry.entity_id),
                &now_text,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    pub async fn apply_push_results(
        &self,
        binding: &SyncBinding,
        entries: &[OutboxEntry],
        results: &[PushResult],
        clock: &dyn Clock,
    ) -> Result<(), SyncError> {
        if entries.len() != results.len() {
            return Err(SyncError::InvalidData);
        }
        let now = clock.now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        Self::assert_binding_generation_on(&mut tx, binding).await?;
        for entry in entries {
            let result = results
                .iter()
                .find(|result| result.operation_id == entry.operation_id)
                .ok_or(SyncError::InvalidData)?;
            let attempt_status = match result.status {
                PushResultStatus::Applied => "applied",
                PushResultStatus::NoOp => "no_op",
            };
            sqlx::query(
                r#"UPDATE cloud_sync_attempts SET status = ?1, finished_at = ?2,
                     result_server_version = ?3, result_cursor = ?4,
                     lease_owner = NULL, lease_expires_at = NULL, error_code = NULL
                   WHERE account_id = ?5 AND cloud_workspace_id = ?6 AND operation_id = ?7"#,
            )
            .bind(attempt_status)
            .bind(&now)
            .bind(result.server_version)
            .bind(result.cursor)
            .bind(&entry.account_id)
            .bind(&entry.cloud_workspace_id)
            .bind(&entry.operation_id)
            .execute(&mut *tx)
            .await?;
            Self::record_success_on(&mut tx, entry, result.server_version, &now).await?;
        }
        sqlx::query(
            r#"UPDATE cloud_sync_workspace_bindings SET
                 initial_confirmed = MIN(initial_total, initial_confirmed + ?1),
                 initialization_checkpoint = ?2,
                 updated_at = ?3
               WHERE account_id = ?4 AND local_workspace_id = ?5"#,
        )
        .bind(entries.len() as i64)
        .bind(entries.last().map(|entry| entry.operation_id.as_str()))
        .bind(&now)
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn record_success_on(
        connection: &mut SqliteConnection,
        entry: &OutboxEntry,
        server_version: i64,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"INSERT INTO cloud_sync_entity_state (
                 account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                 last_operation_id, sync_status, updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'synced', ?7)
               ON CONFLICT(account_id, cloud_workspace_id, entity_type, entity_id) DO UPDATE SET
                 server_version = MAX(server_version, excluded.server_version),
                 last_operation_id = excluded.last_operation_id,
                 sync_status = CASE WHEN sync_status = 'conflict' THEN 'conflict' ELSE 'synced' END,
                 updated_at = excluded.updated_at"#,
        )
        .bind(&entry.account_id)
        .bind(&entry.cloud_workspace_id)
        .bind(&entry.entity_type)
        .bind(&entry.entity_id)
        .bind(server_version)
        .bind(&entry.operation_id)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        sqlx::query("DELETE FROM cloud_sync_outbox WHERE account_id = ?1 AND operation_id = ?2")
            .bind(&entry.account_id)
            .bind(&entry.operation_id)
            .execute(&mut *connection)
            .await?;
        sqlx::query(
            r#"UPDATE cloud_sync_outbox SET base_version = MAX(base_version, ?1), updated_at = ?2
               WHERE account_id = ?3 AND cloud_workspace_id = ?4 AND entity_type = ?5 AND entity_id = ?6"#,
        ).bind(server_version).bind(now).bind(&entry.account_id).bind(&entry.cloud_workspace_id)
         .bind(&entry.entity_type).bind(&entry.entity_id).execute(&mut *connection).await?;
        Ok(())
    }
}
