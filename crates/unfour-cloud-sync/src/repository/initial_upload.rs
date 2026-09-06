//! Create a binding and capture its initial upload set in one transaction.
//! Enumeration and domain snapshots must use that same SQLite connection.

use super::SyncRepository;
use crate::canonical::canonical_snapshot_intent;
use crate::entity_registry::live_syncable_entity_keys_on;
use crate::{Clock, IdGenerator, SyncEntityAdapters, SyncError, SyncOperation};

impl SyncRepository {
    /// Creates the binding and snapshots every registered live entity into the
    /// initial outbox in one transaction.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_binding_with_initial_outbox(
        &self,
        account_id: &str,
        account_generation: u64,
        workspace_id: &str,
        cloud_workspace_id: &str,
        cursor: i64,
        adapters: &SyncEntityAdapters<'_>,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<(), SyncError> {
        let now = clock.now();
        let now_text = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        Self::ensure_new_binding_owner_available_on(&mut tx, account_id, workspace_id).await?;
        sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_bindings (
                 account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, state, initial_cursor, generation, created_at, updated_at
               ) VALUES (?1, ?2, ?3, ?4, 1, 'preparing', ?4, ?5, ?6, ?6)"#,
        )
        .bind(account_id)
        .bind(workspace_id)
        .bind(cloud_workspace_id)
        .bind(cursor)
        .bind(account_generation as i64)
        .bind(&now_text)
        .execute(&mut *tx)
        .await?;
        Self::insert_workspace_owner_on(
            &mut tx,
            account_id,
            workspace_id,
            cloud_workspace_id,
            &now_text,
        )
        .await?;

        let keys = live_syncable_entity_keys_on(&mut tx, workspace_id).await?;
        let mut initial_total = 0;
        for key in &keys {
            let Some(snapshot) = adapters.read_snapshot_on(&mut tx, key).await? else {
                continue;
            };
            let snapshot = canonical_snapshot_intent(snapshot)?;
            if snapshot.entity.workspace_id != workspace_id
                || snapshot.entity.entity_type != key.entity_type
                || snapshot.entity.entity_id != key.entity_id
                || snapshot.intent.operation != SyncOperation::Upsert
            {
                return Err(SyncError::InvalidData);
            }
            Self::enqueue_intent_on(
                &mut tx,
                account_id,
                workspace_id,
                cloud_workspace_id,
                &snapshot.entity.entity_id,
                snapshot.revision,
                snapshot.intent,
                ids.next_id(),
                now,
            )
            .await?;
            initial_total += 1;
        }
        sqlx::query(
            "UPDATE cloud_sync_workspace_bindings SET state = 'uploading', initial_total = ?1, updated_at = ?2 WHERE account_id = ?3 AND local_workspace_id = ?4",
        ).bind(initial_total).bind(&now_text).bind(account_id).bind(workspace_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
