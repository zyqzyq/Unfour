use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;
use unfour_core::domain::DomainSnapshot;

use super::SyncRepository;
use crate::canonical::canonical_snapshot_intent;
use crate::{
    Clock, DeadLetterView, IdGenerator, OutboxEntry, SnapshotItem, SyncBinding, SyncEntityType,
    SyncError, PAYLOAD_SCHEMA_VERSION,
};

impl SyncRepository {
    /// Revives dead letters that were parked with `protocol_version_unsupported`.
    /// After an app upgrade the running build speaks a newer protocol, so those
    /// operations are retryable again. Returns the affected local workspaces so
    /// the caller can refresh their binding state.
    pub async fn revive_protocol_dead_letters(
        &self,
        account_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<String>, SyncError> {
        let mut tx = self.pool.begin().await?;
        let workspaces: Vec<String> = sqlx::query_scalar(
            r#"UPDATE cloud_sync_outbox SET status = 'pending', attempt_count = 0,
                 next_attempt_at = NULL, lease_owner = NULL, lease_started_at = NULL,
                 lease_expires_at = NULL, last_error = NULL, updated_at = ?2
               WHERE account_id = ?1 AND status = 'dead'
                 AND last_error = 'protocol_version_unsupported'
                 AND EXISTS (
                   SELECT 1 FROM cloud_sync_workspace_ownership AS owner
                   WHERE owner.local_workspace_id = cloud_sync_outbox.local_workspace_id
                     AND owner.account_id = cloud_sync_outbox.account_id
                     AND owner.cloud_workspace_id = cloud_sync_outbox.cloud_workspace_id
                 )
               RETURNING local_workspace_id"#,
        )
        .bind(account_id)
        .bind(now.to_rfc3339())
        .fetch_all(&mut *tx)
        .await?;
        let mut affected: Vec<String> = workspaces;
        affected.sort();
        affected.dedup();
        for workspace_id in &affected {
            sqlx::query(
                r#"UPDATE cloud_sync_workspace_bindings SET
                     state = CASE
                       WHEN state = 'paused' THEN 'paused'
                       WHEN EXISTS (
                         SELECT 1 FROM cloud_sync_entity_state
                         WHERE account_id = ?1 AND cloud_workspace_id =
                           cloud_sync_workspace_bindings.cloud_workspace_id
                           AND sync_status = 'conflict'
                       ) THEN 'conflict'
                       WHEN EXISTS (
                         SELECT 1 FROM cloud_sync_outbox
                         WHERE account_id = ?1 AND local_workspace_id = ?2
                           AND status = 'dead'
                       ) THEN 'error'
                       WHEN initial_confirmed < initial_total THEN 'uploading'
                       ELSE 'reconciling'
                     END,
                     last_error = CASE
                       WHEN EXISTS (
                         SELECT 1 FROM cloud_sync_outbox
                         WHERE account_id = ?1 AND local_workspace_id = ?2
                           AND status = 'dead'
                       ) THEN last_error
                       ELSE NULL
                     END,
                     updated_at = ?3
                   WHERE account_id = ?1 AND local_workspace_id = ?2
                     AND EXISTS (
                       SELECT 1 FROM cloud_sync_workspace_ownership AS owner
                       WHERE owner.local_workspace_id = cloud_sync_workspace_bindings.local_workspace_id
                         AND owner.account_id = cloud_sync_workspace_bindings.account_id
                         AND owner.cloud_workspace_id = cloud_sync_workspace_bindings.cloud_workspace_id
                     )"#,
            )
            .bind(account_id)
            .bind(workspace_id)
            .bind(now.to_rfc3339())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(affected)
    }

    pub async fn dead_letters(
        &self,
        account_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<DeadLetterView>, SyncError> {
        sqlx::query_as::<_, DeadLetterView>(
            r#"SELECT outbox.operation_id, outbox.entity_type, outbox.entity_id,
                      CASE outbox.entity_type
                        WHEN 'workspace' THEN (
                          SELECT name FROM workspaces WHERE id = outbox.entity_id
                        )
                        WHEN 'connection' THEN (
                          SELECT name FROM connections
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                        WHEN 'workspaceVariable' THEN (
                          SELECT key FROM workspace_variables
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                        WHEN 'workspaceEnvironment' THEN (
                          SELECT name FROM workspace_environments
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                        WHEN 'workspaceEnvironmentVariable' THEN (
                          SELECT key FROM workspace_environment_variables
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                        WHEN 'apiCollection' THEN (
                          SELECT name FROM api_collections
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                        WHEN 'apiFolder' THEN (
                          SELECT name FROM api_collection_folders
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                        WHEN 'apiRequest' THEN (
                          SELECT name FROM api_requests
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                        WHEN 'sshTask' THEN (
                          SELECT name FROM ssh_task
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                        WHEN 'sshTaskStep' THEN (
                          SELECT name FROM ssh_task_step
                          WHERE workspace_id = outbox.local_workspace_id
                            AND id = outbox.entity_id
                        )
                      END AS entity_name,
                      COALESCE(outbox.last_error, 'cloud_sync_permanent_failure') AS error_code
               FROM cloud_sync_outbox AS outbox
               WHERE outbox.account_id = ?1 AND outbox.local_workspace_id = ?2
                 AND outbox.status = 'dead'
                 AND EXISTS (
                   SELECT 1 FROM cloud_sync_workspace_ownership AS owner
                   WHERE owner.local_workspace_id = outbox.local_workspace_id
                     AND owner.account_id = outbox.account_id
                     AND owner.cloud_workspace_id = outbox.cloud_workspace_id
                 )
               ORDER BY outbox.created_at, outbox.entity_type, outbox.entity_id"#,
        )
        .bind(account_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub(crate) async fn dead_letter(
        &self,
        account_id: &str,
        workspace_id: &str,
        operation_id: &str,
    ) -> Result<OutboxEntry, SyncError> {
        Self::dead_letter_on(
            &mut *self.pool.acquire().await?,
            account_id,
            workspace_id,
            operation_id,
        )
        .await
    }

    pub(crate) async fn dead_letter_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        workspace_id: &str,
        operation_id: &str,
    ) -> Result<OutboxEntry, SyncError> {
        sqlx::query_as::<_, OutboxEntry>(
            r#"SELECT account_id, operation_id, local_workspace_id, cloud_workspace_id,
                      entity_type, entity_id, parent_entity_id, operation, base_version,
                      payload_schema_version, canonical_payload_json, deleted_at,
                      content_revision, status, attempt_count, last_error
               FROM cloud_sync_outbox
               WHERE account_id = ?1 AND local_workspace_id = ?2
                 AND operation_id = ?3 AND status = 'dead'
                 AND EXISTS (
                   SELECT 1 FROM cloud_sync_workspace_ownership AS owner
                   WHERE owner.local_workspace_id = cloud_sync_outbox.local_workspace_id
                     AND owner.account_id = cloud_sync_outbox.account_id
                     AND owner.cloud_workspace_id = cloud_sync_outbox.cloud_workspace_id
                 )"#,
        )
        .bind(account_id)
        .bind(workspace_id)
        .bind(operation_id)
        .fetch_optional(&mut *connection)
        .await?
        .ok_or(SyncError::NotFound)
    }

    /// Older clients marked every operation in an atomically rolled-back
    /// batch dead. Those rows have no batch id in the published schema, but
    /// the attempt lease start is shared by one `mark_in_flight` call. Use the
    /// exact start timestamp and error code as conservative legacy metadata to
    /// revive peers when the user repairs one dead entry. New operation-level
    /// failures leave no peers dead, so this path is a no-op for new data.
    async fn revive_legacy_batch_peers_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        entry: &OutboxEntry,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let Some(error_code) = entry.last_error.as_deref() else {
            return Ok(());
        };
        let Some(started_at) = sqlx::query_scalar::<_, String>(
            r#"SELECT started_at FROM cloud_sync_attempts
               WHERE account_id = ?1 AND cloud_workspace_id = ?2
                 AND operation_id = ?3"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&entry.operation_id)
        .fetch_optional(&mut *connection)
        .await?
        else {
            return Ok(());
        };
        sqlx::query(
            r#"UPDATE cloud_sync_outbox SET status = 'pending', attempt_count = 0,
                 next_attempt_at = NULL, lease_owner = NULL,
                 lease_started_at = NULL, lease_expires_at = NULL,
                 last_error = NULL, updated_at = ?1
               WHERE account_id = ?2 AND local_workspace_id = ?3
                 AND cloud_workspace_id = ?4 AND status = 'dead'
                 AND operation_id <> ?5 AND last_error = ?6
                 AND EXISTS (
                   SELECT 1 FROM cloud_sync_attempts AS attempt
                   WHERE attempt.account_id = cloud_sync_outbox.account_id
                     AND attempt.cloud_workspace_id = cloud_sync_outbox.cloud_workspace_id
                     AND attempt.operation_id = cloud_sync_outbox.operation_id
                     AND attempt.started_at = ?7
                     AND attempt.status = 'failed'
                     AND attempt.error_code = ?6
                 )"#,
        )
        .bind(now.to_rfc3339())
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&entry.operation_id)
        .bind(error_code)
        .bind(started_at)
        .execute(&mut *connection)
        .await?;
        Ok(())
    }

    pub async fn retry_dead_letter_current_snapshot(
        &self,
        binding: &SyncBinding,
        operation_id: &str,
        snapshot: DomainSnapshot,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<String, SyncError> {
        let now = clock.now();
        let mut tx = self.pool.begin().await?;
        Self::assert_recovery_binding_on(&mut tx, binding).await?;
        let entry = Self::dead_letter_on(
            &mut tx,
            &binding.account_id,
            &binding.local_workspace_id,
            operation_id,
        )
        .await?;
        let new_operation_id = ids.next_id();
        if new_operation_id.trim().is_empty() {
            return Err(SyncError::InvalidData);
        }
        let operation_id_was_used: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                 SELECT 1 FROM cloud_sync_outbox WHERE operation_id = ?1
               ) OR EXISTS(
                 SELECT 1 FROM cloud_sync_attempts
                 WHERE operation_id = ?1
               )"#,
        )
        .bind(&new_operation_id)
        .fetch_one(&mut *tx)
        .await?;
        if operation_id_was_used {
            return Err(SyncError::InvalidData);
        }
        let snapshot = canonical_snapshot_intent(snapshot)?;
        if snapshot.entity.workspace_id != binding.local_workspace_id
            || snapshot.entity.entity_id != entry.entity_id
            || snapshot.intent.entity_type.as_str() != entry.entity_type
        {
            return Err(SyncError::InvalidData);
        }
        let reliable_base_version: i64 = sqlx::query_scalar(
            r#"SELECT MAX(?1, COALESCE((
                 SELECT server_version FROM cloud_sync_entity_state
                 WHERE account_id = ?2 AND cloud_workspace_id = ?3
                   AND entity_type = ?4 AND entity_id = ?5
               ), 0))"#,
        )
        .bind(entry.base_version)
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&entry.entity_type)
        .bind(&entry.entity_id)
        .fetch_one(&mut *tx)
        .await?;
        // Reviving a dead row is a recovery transition, not a normal local
        // enqueue. Rewrite the same durable head from the freshly read Core
        // snapshot so an old canonical payload (or an old content revision)
        // can never be sent again. Keep the old operation's attempt row as
        // history while assigning a new id to the pending operation.
        Self::revive_legacy_batch_peers_on(&mut tx, binding, &entry, now).await?;
        let changed = sqlx::query(
            r#"UPDATE cloud_sync_outbox SET
                 operation_id = ?1, parent_entity_id = ?2, operation = ?3,
                 base_version = ?4, payload_schema_version = ?5,
                 canonical_payload_json = ?6, deleted_at = ?7,
                 content_revision = ?8, status = 'pending', attempt_count = 0,
                 next_attempt_at = NULL, lease_owner = NULL,
                 lease_started_at = NULL, lease_expires_at = NULL,
                 last_error = NULL, updated_at = ?9
               WHERE account_id = ?10 AND local_workspace_id = ?11
                 AND operation_id = ?12 AND status = 'dead'"#,
        )
        .bind(&new_operation_id)
        .bind(&snapshot.intent.parent_entity_id)
        .bind(snapshot.intent.operation.as_str())
        .bind(reliable_base_version)
        .bind(PAYLOAD_SCHEMA_VERSION)
        .bind(&snapshot.intent.payload_json)
        .bind(&snapshot.intent.deleted_at)
        .bind(snapshot.revision)
        .bind(now.to_rfc3339())
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .bind(&entry.operation_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(SyncError::Storage);
        }
        sqlx::query(
            r#"UPDATE cloud_sync_entity_state SET sync_status = 'synced',
                 conflict_remote_payload_json = NULL, conflict_remote_operation = NULL,
                 conflict_parent_entity_id = NULL, conflict_deleted_at = NULL,
                 conflict_operation_id = NULL, updated_at = ?1
               WHERE account_id = ?2 AND cloud_workspace_id = ?3
                 AND entity_type = ?4 AND entity_id = ?5"#,
        )
        .bind(now.to_rfc3339())
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&entry.entity_type)
        .bind(&entry.entity_id)
        .execute(&mut *tx)
        .await?;
        Self::refresh_binding_after_dead_letter_on(&mut tx, binding, now).await?;
        tx.commit().await?;
        Ok(new_operation_id)
    }

    /// `use_remote` with an absent remote means "delete the local entity".
    /// Refuse when that delete would cascade into local descendants the user
    /// never confirmed losing, or strand other outbox intents whose parent
    /// chain disappears.
    pub(crate) async fn ensure_remote_absence_is_safe_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        entry: &OutboxEntry,
    ) -> Result<(), SyncError> {
        let entity_type = SyncEntityType::parse(&entry.entity_type)?;
        match entity_type {
            SyncEntityType::Workspace => Err(SyncError::SafeReplaceUnavailable),
            SyncEntityType::WorkspaceEnvironment => {
                let dependent_intents: bool = sqlx::query_scalar(
                    r#"SELECT EXISTS(SELECT 1 FROM cloud_sync_outbox
                       WHERE account_id = ?1 AND cloud_workspace_id = ?2
                         AND operation_id <> ?3
                         AND entity_type = 'workspaceEnvironmentVariable'
                         AND parent_entity_id = ?4)"#,
                )
                .bind(&binding.account_id)
                .bind(&binding.cloud_workspace_id)
                .bind(&entry.operation_id)
                .bind(&entry.entity_id)
                .fetch_one(&mut *connection)
                .await?;
                if dependent_intents {
                    return Err(SyncError::SafeReplaceUnavailable);
                }
                Ok(())
            }
            SyncEntityType::ApiCollection | SyncEntityType::ApiFolder => {
                Self::ensure_api_subtree_absence_is_safe_on(connection, binding, entry, entity_type)
                    .await
            }
            SyncEntityType::SshTask => {
                let has_local_children: bool = sqlx::query_scalar(
                    r#"SELECT EXISTS(
                         SELECT 1 FROM ssh_task_step
                         WHERE workspace_id = ?1 AND task_id = ?2 AND deleted_at IS NULL
                       ) OR EXISTS(
                         SELECT 1 FROM cloud_sync_outbox
                         WHERE account_id = ?3 AND cloud_workspace_id = ?4
                           AND operation_id <> ?5 AND entity_type = 'sshTaskStep'
                           AND parent_entity_id = ?2
                       )"#,
                )
                .bind(&binding.local_workspace_id)
                .bind(&entry.entity_id)
                .bind(&binding.account_id)
                .bind(&binding.cloud_workspace_id)
                .bind(&entry.operation_id)
                .fetch_one(&mut *connection)
                .await?;
                if has_local_children {
                    return Err(SyncError::SafeReplaceUnavailable);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn ensure_api_subtree_absence_is_safe_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        entry: &OutboxEntry,
        entity_type: SyncEntityType,
    ) -> Result<(), SyncError> {
        let descendants_sql = match entity_type {
            SyncEntityType::ApiCollection => {
                r#"SELECT id FROM api_collection_folders
                   WHERE workspace_id = ?1 AND collection_id = ?2
                   UNION ALL
                   SELECT id FROM api_requests
                   WHERE workspace_id = ?1 AND collection_id = ?2"#
            }
            SyncEntityType::ApiFolder => {
                r#"WITH RECURSIVE folder_tree(id) AS (
                     SELECT id FROM api_collection_folders
                     WHERE workspace_id = ?1 AND id = ?2
                     UNION
                     SELECT folders.id FROM api_collection_folders AS folders
                     JOIN folder_tree ON folders.parent_folder_id = folder_tree.id
                     WHERE folders.workspace_id = ?1
                   )
                   SELECT id FROM folder_tree WHERE id <> ?2
                   UNION ALL
                   SELECT requests.id FROM api_requests AS requests
                   WHERE requests.workspace_id = ?1
                     AND requests.parent_folder_id IN (SELECT id FROM folder_tree)"#
            }
            _ => return Ok(()),
        };
        let live_descendants_sql = format!(
            r#"SELECT EXISTS(
                 SELECT 1 FROM api_collection_folders
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
                   AND id IN ({descendants_sql})
               ) OR EXISTS(
                 SELECT 1 FROM api_requests
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
                   AND id IN ({descendants_sql})
               )"#
        );
        let live_descendants: bool = sqlx::query_scalar(&live_descendants_sql)
            .bind(&binding.local_workspace_id)
            .bind(&entry.entity_id)
            .fetch_one(&mut *connection)
            .await?;
        if live_descendants {
            return Err(SyncError::SafeReplaceUnavailable);
        }
        let dependent_intents_sql = format!(
            r#"SELECT EXISTS(
                 SELECT 1 FROM cloud_sync_outbox
                 WHERE account_id = ?3 AND cloud_workspace_id = ?4
                   AND operation_id <> ?5
                   AND entity_type IN ('apiFolder', 'apiRequest')
                   AND entity_id IN ({descendants_sql})
               )"#
        );
        let dependent_intents: bool = sqlx::query_scalar(&dependent_intents_sql)
            .bind(&binding.local_workspace_id)
            .bind(&entry.entity_id)
            .bind(&binding.account_id)
            .bind(&binding.cloud_workspace_id)
            .bind(&entry.operation_id)
            .fetch_one(&mut *connection)
            .await?;
        if dependent_intents {
            return Err(SyncError::SafeReplaceUnavailable);
        }
        Ok(())
    }

    pub(crate) async fn assert_recovery_binding_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
    ) -> Result<(), SyncError> {
        Self::assert_workspace_owner_on(connection, binding).await?;
        let current: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM cloud_sync_workspace_bindings
               WHERE account_id = ?1 AND local_workspace_id = ?2
                 AND cloud_workspace_id = ?3 AND generation = ?4
                 AND last_pulled_cursor = ?5)"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .bind(&binding.cloud_workspace_id)
        .bind(binding.generation)
        .bind(binding.last_pulled_cursor)
        .fetch_one(&mut *connection)
        .await?;
        current.then_some(()).ok_or(SyncError::AccountChanged)
    }

    pub(crate) async fn finish_remote_dead_letter_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        entry: &OutboxEntry,
        remote: Option<&SnapshotItem>,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        if let Some(item) = remote {
            if item.entity_type.as_str() != entry.entity_type || item.entity_id != entry.entity_id {
                return Err(SyncError::InvalidData);
            }
            Self::record_snapshot_state_on(
                connection,
                &binding.account_id,
                &binding.cloud_workspace_id,
                item,
                &now.to_rfc3339(),
            )
            .await?;
            sqlx::query(
                r#"UPDATE cloud_sync_entity_state SET
                     conflict_remote_payload_json = NULL,
                     conflict_remote_operation = NULL,
                     conflict_parent_entity_id = NULL,
                     conflict_deleted_at = NULL,
                     conflict_operation_id = NULL
                   WHERE account_id = ?1 AND cloud_workspace_id = ?2
                     AND entity_type = ?3 AND entity_id = ?4"#,
            )
            .bind(&binding.account_id)
            .bind(&binding.cloud_workspace_id)
            .bind(&entry.entity_type)
            .bind(&entry.entity_id)
            .execute(&mut *connection)
            .await?;
        } else {
            sqlx::query(
                r#"DELETE FROM cloud_sync_entity_state
                   WHERE account_id = ?1 AND cloud_workspace_id = ?2
                     AND entity_type = ?3 AND entity_id = ?4"#,
            )
            .bind(&binding.account_id)
            .bind(&binding.cloud_workspace_id)
            .bind(&entry.entity_type)
            .bind(&entry.entity_id)
            .execute(&mut *connection)
            .await?;
        }
        Self::revive_legacy_batch_peers_on(connection, binding, entry, now).await?;
        let changed = sqlx::query(
            r#"DELETE FROM cloud_sync_outbox
               WHERE account_id = ?1 AND local_workspace_id = ?2
                 AND operation_id = ?3 AND status = 'dead'"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.local_workspace_id)
        .bind(&entry.operation_id)
        .execute(&mut *connection)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(SyncError::Storage);
        }
        Self::refresh_binding_after_dead_letter_on(connection, binding, now).await
    }

    async fn refresh_binding_after_dead_letter_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let changed = sqlx::query(
            r#"UPDATE cloud_sync_workspace_bindings SET
                 state = CASE
                   WHEN sync_enabled = 0 OR NOT EXISTS (
                     SELECT 1 FROM cloud_sync_account_settings
                     WHERE account_id = ?2 AND sync_enabled = 1
                   ) THEN 'paused'
                   WHEN EXISTS (
                     SELECT 1 FROM cloud_sync_entity_state
                     WHERE account_id = ?2 AND cloud_workspace_id = ?3
                       AND sync_status = 'conflict'
                   ) THEN 'conflict'
                   WHEN EXISTS (
                     SELECT 1 FROM cloud_sync_outbox
                     WHERE account_id = ?2 AND local_workspace_id = ?4
                       AND status = 'dead'
                   ) THEN 'error'
                   WHEN initial_confirmed < initial_total THEN 'uploading'
                   ELSE 'reconciling'
                 END,
                 last_error = CASE
                   WHEN EXISTS (
                     SELECT 1 FROM cloud_sync_entity_state
                     WHERE account_id = ?2 AND cloud_workspace_id = ?3
                       AND sync_status = 'conflict'
                   ) THEN 'remote_local_conflict'
                   WHEN EXISTS (
                     SELECT 1 FROM cloud_sync_outbox
                     WHERE account_id = ?2 AND local_workspace_id = ?4
                       AND status = 'dead'
                   ) THEN 'cloud_sync_dead_letter_blocked'
                   ELSE NULL
                 END,
                 updated_at = ?1
               WHERE account_id = ?2 AND local_workspace_id = ?4
                 AND generation = ?5"#,
        )
        .bind(now.to_rfc3339())
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&binding.local_workspace_id)
        .bind(binding.generation)
        .execute(&mut *connection)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(SyncError::AccountChanged);
        }
        Ok(())
    }
}
