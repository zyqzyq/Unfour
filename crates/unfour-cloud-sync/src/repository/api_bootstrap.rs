//! One-time API entity backfill for bindings created before API sync support.

use unfour_core::domain::DomainEntityType;
use unfour_http_engine::ApiClientService;

use super::SyncRepository;
use crate::canonical::canonical_snapshot_intent;
use crate::{Clock, IdGenerator, SyncBinding, SyncEntityType, SyncError, SyncOperation};

impl SyncRepository {
    /// Captures live API entities that predate the API sync protocol marker.
    /// The marker and all outbox rows commit together, so a retry after a
    /// crash cannot permanently skip the backfill.
    pub async fn bootstrap_api_v2(
        &self,
        binding: &SyncBinding,
        api_client: &ApiClientService,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<bool, SyncError> {
        let now = clock.now();
        let now_text = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;

        let state: String = sqlx::query_scalar(
            "SELECT api_v2_bootstrap_state FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND local_workspace_id = ?2 AND cloud_workspace_id = ?3",
        )
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .bind(&binding.cloud_workspace_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(SyncError::NotFound)?;
        if state == "completed" {
            tx.commit().await?;
            return Ok(false);
        }
        if state != "pending" {
            return Err(SyncError::InvalidData);
        }

        // The no-op update acquires SQLite's write lock before enumeration,
        // matching the SSH bootstrap's claim protocol.
        let claimed = sqlx::query(
            r#"UPDATE cloud_sync_workspace_bindings
               SET api_v2_bootstrap_state = 'pending', updated_at = ?1
               WHERE account_id = ?2 AND local_workspace_id = ?3
                 AND cloud_workspace_id = ?4 AND generation = ?5
                 AND api_v2_bootstrap_state = 'pending'
                 AND sync_enabled = 1 AND state <> 'paused'
                 AND EXISTS (
                   SELECT 1 FROM cloud_sync_account_settings
                   WHERE account_id = ?2 AND sync_enabled = 1
                 )"#,
        )
        .bind(&now_text)
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .bind(&binding.cloud_workspace_id)
        .bind(binding.generation)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if claimed == 0 {
            let completed: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(
                     SELECT 1 FROM cloud_sync_workspace_bindings
                     WHERE account_id = ?1 AND local_workspace_id = ?2
                       AND cloud_workspace_id = ?3 AND generation = ?4
                       AND api_v2_bootstrap_state = 'completed'
                       AND sync_enabled = 1 AND state <> 'paused'
                       AND EXISTS (
                         SELECT 1 FROM cloud_sync_account_settings
                         WHERE account_id = ?1 AND sync_enabled = 1
                       )
                   )"#,
            )
            .bind(&binding.account_id)
            .bind(&binding.local_workspace_id)
            .bind(&binding.cloud_workspace_id)
            .bind(binding.generation)
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            return completed.then_some(false).ok_or(SyncError::AccountChanged);
        }

        let keys = Self::live_api_entity_keys_on(&mut tx, &binding.local_workspace_id).await?;
        for key in &keys {
            // Existing remote state or any durable intent already participated
            // in API sync after the feature was introduced. Re-enqueue only
            // entities with no sync history, preserving pending/conflict/dead
            // letter history.
            let known: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(
                     SELECT 1 FROM cloud_sync_entity_state
                     WHERE account_id = ?1 AND cloud_workspace_id = ?2
                       AND entity_type = ?3 AND entity_id = ?4
                   ) OR EXISTS(
                     SELECT 1 FROM cloud_sync_outbox
                     WHERE account_id = ?1 AND cloud_workspace_id = ?2
                       AND entity_type = ?3 AND entity_id = ?4
                   )"#,
            )
            .bind(&binding.account_id)
            .bind(&binding.cloud_workspace_id)
            .bind(SyncEntityType::from(key.entity_type).as_str())
            .bind(&key.entity_id)
            .fetch_one(&mut *tx)
            .await?;
            if known {
                continue;
            }

            let snapshot = api_client
                .read_domain_snapshot_on(&mut tx, key)
                .await
                .map_err(|_| SyncError::Core)?;
            let snapshot = canonical_snapshot_intent(snapshot)?;
            if snapshot.entity.workspace_id != binding.local_workspace_id
                || snapshot.intent.operation != SyncOperation::Upsert
                || !matches!(
                    snapshot.entity.entity_type,
                    DomainEntityType::ApiCollection
                        | DomainEntityType::ApiFolder
                        | DomainEntityType::ApiRequest
                )
            {
                return Err(SyncError::InvalidData);
            }
            Self::enqueue_intent_on(
                &mut tx,
                &binding.account_id,
                &binding.local_workspace_id,
                &binding.cloud_workspace_id,
                &snapshot.entity.entity_id,
                snapshot.revision,
                snapshot.intent,
                ids.next_id(),
                now,
            )
            .await?;
        }

        let completed = sqlx::query(
            r#"UPDATE cloud_sync_workspace_bindings
               SET api_v2_bootstrap_state = 'completed', updated_at = ?1
               WHERE account_id = ?2 AND local_workspace_id = ?3
                 AND cloud_workspace_id = ?4 AND generation = ?5
                 AND api_v2_bootstrap_state = 'pending'"#,
        )
        .bind(&now_text)
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .bind(&binding.cloud_workspace_id)
        .bind(binding.generation)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if completed != 1 {
            return Err(SyncError::AccountChanged);
        }
        tx.commit().await?;
        Ok(true)
    }
}
