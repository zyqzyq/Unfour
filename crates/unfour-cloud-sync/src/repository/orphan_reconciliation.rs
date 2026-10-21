//! Idempotent capture of local entities missing sync state and Outbox intent.
//! Flow activation also captures pre-existing definitions before remote apply.

use super::SyncRepository;
use crate::canonical::canonical_snapshot_intent;
use crate::entity_registry::{live_syncable_entity_keys_on, sync_entity_descriptor};
use crate::{
    Clock, IdGenerator, SyncBinding, SyncEntityAdapters, SyncEntityType, SyncError, SyncOperation,
};
use unfour_core::domain::DomainEntityKey;

impl SyncRepository {
    /// Re-enqueues live local entities that have neither sync state nor an
    /// outbox row for this exact account/cloud binding. The transaction is
    /// generation fenced and idempotent.
    #[allow(clippy::too_many_arguments)]
    pub async fn reconcile_missing_local_sync_state(
        &self,
        binding: &SyncBinding,
        adapters: &SyncEntityAdapters<'_>,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<usize, SyncError> {
        self.reconcile_missing_local_sync_state_for(binding, adapters, ids, clock, false)
            .await
    }

    /// Older clients never captured Flow mutations. Preserve those documents
    /// and tombstones before Pull/replay can replace them or delete a workspace.
    /// Other entities still repair after replay to preserve retained fields.
    pub(crate) async fn capture_untracked_flow_definitions(
        &self,
        binding: &SyncBinding,
        adapters: &SyncEntityAdapters<'_>,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<usize, SyncError> {
        self.reconcile_missing_local_sync_state_for(binding, adapters, ids, clock, true)
            .await
    }

    async fn reconcile_missing_local_sync_state_for(
        &self,
        binding: &SyncBinding,
        adapters: &SyncEntityAdapters<'_>,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
        only_flow: bool,
    ) -> Result<usize, SyncError> {
        let now = clock.now();
        let mut tx = self.pool.begin().await?;
        let owner = Self::resolve_cloud_sync_owner_on(&mut tx, &binding.local_workspace_id)
            .await?
            .ok_or(SyncError::WorkspaceOwnershipInvariant)?;
        if owner.account_id != binding.account_id
            || owner.cloud_workspace_id != binding.cloud_workspace_id
        {
            return Ok(0);
        }
        Self::assert_binding_generation_on(&mut tx, binding).await?;

        let keys = if only_flow {
            let descriptor = sync_entity_descriptor(SyncEntityType::Flow);
            let storage = descriptor.local_storage;
            let ids: Vec<String> = sqlx::query_scalar(&format!(
                "SELECT id FROM {} WHERE {} = ? ORDER BY id",
                storage.table,
                storage.workspace_column.ok_or(SyncError::InvalidData)?
            ))
            .bind(&binding.local_workspace_id)
            .fetch_all(&mut *tx)
            .await?;
            ids.into_iter()
                .map(|id| {
                    DomainEntityKey::new(
                        descriptor.domain_entity_type,
                        &binding.local_workspace_id,
                        id,
                    )
                })
                .collect()
        } else {
            live_syncable_entity_keys_on(&mut tx, &binding.local_workspace_id).await?
        };
        let mut repaired = 0;
        for key in keys {
            let entity_type = SyncEntityType::try_from(key.entity_type)?;
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
            .bind(entity_type.as_str())
            .bind(&key.entity_id)
            .fetch_one(&mut *tx)
            .await?;
            if known {
                continue;
            }

            let Some(snapshot) = adapters.read_snapshot_on(&mut tx, &key).await? else {
                continue;
            };
            let snapshot = canonical_snapshot_intent(snapshot)?;
            if !only_flow && snapshot.intent.operation != SyncOperation::Upsert {
                continue;
            }
            if snapshot.entity.workspace_id != binding.local_workspace_id
                || snapshot.entity.entity_type != key.entity_type
                || snapshot.entity.entity_id != key.entity_id
            {
                return Err(SyncError::InvalidData);
            }

            Self::enqueue_intent_on(
                &mut tx,
                &binding.account_id,
                &binding.local_workspace_id,
                &binding.cloud_workspace_id,
                &key.entity_id,
                snapshot.revision,
                snapshot.intent,
                ids.next_id(),
                now,
            )
            .await?;
            repaired += 1;
        }

        tx.commit().await?;
        Ok(repaired)
    }
}
