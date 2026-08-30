//! Reconcile remote changes with local intent and persist conflict resolutions.
//! Prepare, Core apply and cursor advancement must share the caller transaction.

use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;

use super::SyncRepository;
use crate::canonical::canonical_snapshot_intent;
use crate::conflict_scope;
use crate::{
    Clock, IdGenerator, RemoteChange, SyncBinding, SyncConflict, SyncConflictDetails,
    SyncEntityType, SyncError, PAYLOAD_SCHEMA_VERSION,
};
use unfour_core::domain::{
    DomainEntityKey, DomainMutation, DomainSnapshot, MutationOperation, MutationOrigin,
};

impl SyncRepository {
    /// Returns true only when the remote row is safe to apply to live Core
    /// tables. Local intent creates a conflict shadow and is never overwritten.
    pub async fn prepare_remote_change_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        change: &RemoteChange,
        now: &str,
    ) -> Result<bool, SyncError> {
        Self::prepare_remote_change_with_aggregate_root_on(connection, binding, change, None, now)
            .await
    }

    pub(crate) async fn prepare_remote_change_with_aggregate_root_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        change: &RemoteChange,
        aggregate_delete_root: Option<(SyncEntityType, &str)>,
        now: &str,
    ) -> Result<bool, SyncError> {
        let attempt_exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM cloud_sync_attempts
               WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND operation_id = ?3)"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&change.operation_id)
        .fetch_one(&mut *connection)
        .await?;
        let head_matches: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM cloud_sync_outbox
               WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND operation_id = ?3)"#,
        )
        .bind(&binding.account_id)
        .bind(&binding.cloud_workspace_id)
        .bind(&change.operation_id)
        .fetch_one(&mut *connection)
        .await?;
        if attempt_exists || head_matches {
            sqlx::query(
                r#"UPDATE cloud_sync_attempts SET status = 'applied', finished_at = ?1,
                     result_server_version = ?2, result_cursor = ?3,
                     lease_owner = NULL, lease_expires_at = NULL, error_code = NULL
                   WHERE account_id = ?4 AND cloud_workspace_id = ?5 AND operation_id = ?6"#,
            )
            .bind(now)
            .bind(change.server_version)
            .bind(change.cursor)
            .bind(&binding.account_id)
            .bind(&binding.cloud_workspace_id)
            .bind(&change.operation_id)
            .execute(&mut *connection)
            .await?;
            sqlx::query("DELETE FROM cloud_sync_outbox WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND operation_id = ?3")
                .bind(&binding.account_id).bind(&binding.cloud_workspace_id).bind(&change.operation_id).execute(&mut *connection).await?;
            sqlx::query(
                r#"UPDATE cloud_sync_outbox SET base_version = MAX(base_version, ?1), updated_at = ?2
                   WHERE account_id = ?3 AND cloud_workspace_id = ?4 AND entity_type = ?5 AND entity_id = ?6"#,
            ).bind(change.server_version).bind(now).bind(&binding.account_id).bind(&binding.cloud_workspace_id)
             .bind(change.entity_type.as_str()).bind(&change.entity_id).execute(&mut *connection).await?;
            Self::record_remote_state_on(connection, binding, change, "synced", now).await?;
            return Ok(false);
        }
        // A children-first aggregate delete must be checked against its root
        // before any child tombstone is applied. The child still gets its own
        // conflict shadow so keep-local can re-push its remote base version.
        let (intent_entity_type, intent_entity_id) =
            aggregate_delete_root.unwrap_or((change.entity_type, &change.entity_id));
        let local_intent = conflict_scope::has_local_intent_on(
            connection,
            binding,
            intent_entity_type,
            intent_entity_id,
            change.operation,
        )
        .await?;
        if local_intent {
            Self::record_remote_state_on(connection, binding, change, "conflict", now).await?;
            sqlx::query(
                "UPDATE cloud_sync_workspace_bindings SET state = 'conflict', last_error = 'remote_local_conflict', updated_at = ?1 WHERE account_id = ?2 AND local_workspace_id = ?3",
            ).bind(now).bind(&binding.account_id).bind(&binding.local_workspace_id).execute(&mut *connection).await?;
            Self::record_diagnostic_on(
                connection,
                &binding.account_id,
                Some(&binding.cloud_workspace_id),
                "conflict",
                "remote_local_conflict",
                Some(change.entity_type.as_str()),
                Some(&change.entity_id),
                now,
            )
            .await?;
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) async fn record_remote_state_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        change: &RemoteChange,
        status: &str,
        now: &str,
    ) -> Result<(), SyncError> {
        let payload_json = change
            .payload
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| SyncError::InvalidData)?;
        sqlx::query(
            r#"INSERT INTO cloud_sync_entity_state (
                 account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                 last_operation_id, sync_status, conflict_remote_payload_json,
                 conflict_remote_operation, conflict_parent_entity_id, conflict_deleted_at,
                 conflict_operation_id, updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                         CASE WHEN ?7 = 'conflict' THEN ?8 ELSE NULL END,
                         CASE WHEN ?7 = 'conflict' THEN ?9 ELSE NULL END,
                         CASE WHEN ?7 = 'conflict' THEN ?10 ELSE NULL END,
                         CASE WHEN ?7 = 'conflict' THEN ?11 ELSE NULL END,
                         CASE WHEN ?7 = 'conflict' THEN ?6 ELSE NULL END, ?12)
               ON CONFLICT(account_id, cloud_workspace_id, entity_type, entity_id) DO UPDATE SET
                 server_version = excluded.server_version, last_operation_id = excluded.last_operation_id,
                 sync_status = excluded.sync_status,
                 conflict_remote_payload_json = excluded.conflict_remote_payload_json,
                 conflict_remote_operation = excluded.conflict_remote_operation,
                 conflict_parent_entity_id = excluded.conflict_parent_entity_id,
                 conflict_deleted_at = excluded.conflict_deleted_at,
                 conflict_operation_id = excluded.conflict_operation_id, updated_at = excluded.updated_at"#,
        ).bind(&binding.account_id).bind(&binding.cloud_workspace_id).bind(change.entity_type.as_str())
         .bind(&change.entity_id).bind(change.server_version).bind(&change.operation_id).bind(status)
         .bind(payload_json).bind(change.operation.as_str()).bind(&change.parent_entity_id)
         .bind(&change.deleted_at).bind(now).execute(&mut *connection).await?;
        Ok(())
    }

    pub async fn record_applied_remote_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        change: &RemoteChange,
        now: &str,
    ) -> Result<(), SyncError> {
        Self::record_remote_state_on(connection, binding, change, "synced", now).await
    }

    pub async fn record_push_conflict(
        &self,
        binding: &SyncBinding,
        details: &SyncConflictDetails,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let change = RemoteChange {
            cursor: binding.last_pulled_cursor,
            operation_id: "server-conflict".into(),
            entity_type: details.entity_type,
            entity_id: details.entity_id.clone(),
            parent_entity_id: details.parent_entity_id.clone(),
            operation: details.operation,
            server_version: details.server_version.max(1),
            payload_schema_version: details
                .payload_schema_version
                .unwrap_or(PAYLOAD_SCHEMA_VERSION),
            payload: details.payload.clone(),
            deleted_at: None,
        };
        let now = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        Self::assert_binding_generation_on(&mut tx, binding).await?;
        Self::record_remote_state_on(&mut tx, binding, &change, "conflict", &now).await?;
        sqlx::query("UPDATE cloud_sync_workspace_bindings SET state = 'conflict', last_error = 'base_version_conflict', updated_at = ?1 WHERE account_id = ?2 AND local_workspace_id = ?3")
            .bind(&now).bind(&binding.account_id).bind(&binding.local_workspace_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn advance_cursor_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        cursor: i64,
        now: &str,
    ) -> Result<(), SyncError> {
        let changed = sqlx::query(
            r#"UPDATE cloud_sync_workspace_bindings SET last_pulled_cursor = ?1,
                 last_success_at = ?2,
                 last_error = CASE WHEN EXISTS (
                   SELECT 1 FROM cloud_sync_outbox
                   WHERE account_id = cloud_sync_workspace_bindings.account_id
                     AND local_workspace_id = cloud_sync_workspace_bindings.local_workspace_id
                     AND status = 'dead'
                 ) THEN COALESCE(last_error, 'cloud_sync_dead_letter_blocked') ELSE NULL END,
                 updated_at = ?2
               WHERE account_id = ?3 AND local_workspace_id = ?4 AND generation = ?5"#,
        )
        .bind(cursor)
        .bind(now)
        .bind(&binding.account_id)
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

    pub async fn conflicts(
        &self,
        account_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<SyncConflict>, SyncError> {
        sqlx::query_as::<_, SyncConflict>(
            r#"SELECT state.account_id, state.cloud_workspace_id, state.entity_type, state.entity_id,
                      state.server_version, state.conflict_remote_payload_json,
                      state.conflict_remote_operation, state.conflict_parent_entity_id,
                      state.conflict_deleted_at, state.conflict_operation_id
               FROM cloud_sync_entity_state AS state JOIN cloud_sync_workspace_bindings AS binding
                 ON binding.account_id = state.account_id AND binding.cloud_workspace_id = state.cloud_workspace_id
               WHERE binding.account_id = ?1 AND binding.local_workspace_id = ?2 AND state.sync_status = 'conflict'
               ORDER BY state.entity_type, state.entity_id"#,
        ).bind(account_id).bind(workspace_id).fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn conflict(
        &self,
        account_id: &str,
        cloud_workspace_id: &str,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<SyncConflict, SyncError> {
        sqlx::query_as::<_, SyncConflict>(
            r#"SELECT account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                      conflict_remote_payload_json, conflict_remote_operation,
                      conflict_parent_entity_id, conflict_deleted_at, conflict_operation_id
               FROM cloud_sync_entity_state WHERE account_id = ?1 AND cloud_workspace_id = ?2
                 AND entity_type = ?3 AND entity_id = ?4 AND sync_status = 'conflict'"#,
        )
        .bind(account_id)
        .bind(cloud_workspace_id)
        .bind(entity_type.as_str())
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SyncError::NotFound)
    }

    pub async fn local_secret_present(
        &self,
        workspace_id: &str,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<Option<bool>, SyncError> {
        let present = match entity_type {
            SyncEntityType::WorkspaceVariable => sqlx::query_as::<_, (bool, bool)>("SELECT is_secret, value <> '' FROM workspace_variables WHERE workspace_id = ?1 AND id = ?2")
                .bind(workspace_id).bind(entity_id).fetch_optional(&self.pool).await?.and_then(|(secret, present)| secret.then_some(present)),
            SyncEntityType::WorkspaceEnvironmentVariable => sqlx::query_as::<_, (bool, bool)>("SELECT is_secret, value <> '' FROM workspace_environment_variables WHERE workspace_id = ?1 AND id = ?2")
                .bind(workspace_id).bind(entity_id).fetch_optional(&self.pool).await?.and_then(|(secret, present)| secret.then_some(present)),
            _ => None,
        };
        Ok(present)
    }

    pub async fn keep_local(
        &self,
        binding: &SyncBinding,
        conflict: &SyncConflict,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<(), SyncError> {
        let now = clock.now();
        let mut tx = self.pool.begin().await?;
        Self::assert_binding_generation_on(&mut tx, binding).await?;
        let conflicts = conflict_scope::conflicts_on(&mut tx, binding, conflict).await?;
        for scoped_conflict in &conflicts {
            let entity_type = SyncEntityType::parse(&scoped_conflict.entity_type)?;
            let revision = Self::entity_revision_on(
                &mut tx,
                &binding.local_workspace_id,
                &scoped_conflict.entity_id,
            )
            .await?;
            let operation = if Self::entity_is_deleted_on(
                &mut tx,
                entity_type,
                &binding.local_workspace_id,
                &scoped_conflict.entity_id,
            )
            .await?
            {
                MutationOperation::Delete
            } else {
                MutationOperation::Upsert
            };
            let mut key = DomainEntityKey::new(
                entity_type.into(),
                &binding.local_workspace_id,
                &scoped_conflict.entity_id,
            );
            key.parent_entity_id
                .clone_from(&scoped_conflict.conflict_parent_entity_id);
            // Protocol v1 treats the Workspace root as a versioned entity, so a
            // root tombstone is recoverable by a new upsert at that tombstone's
            // server version. A future API that deletes the cloud container
            // itself must use an explicit copy/create flow instead.
            let mutation = DomainMutation::new(MutationOrigin::Local, operation, key, revision);
            Self::enqueue_on(
                &mut tx,
                &binding.account_id,
                &binding.local_workspace_id,
                &binding.cloud_workspace_id,
                &mutation,
                ids.next_id(),
                now,
            )
            .await?;
            sqlx::query("UPDATE cloud_sync_outbox SET base_version = ?1 WHERE account_id = ?2 AND cloud_workspace_id = ?3 AND entity_type = ?4 AND entity_id = ?5")
                .bind(scoped_conflict.server_version).bind(&binding.account_id).bind(&binding.cloud_workspace_id)
                .bind(&scoped_conflict.entity_type).bind(&scoped_conflict.entity_id).execute(&mut *tx).await?;
        }
        for scoped_conflict in &conflicts {
            Self::clear_conflict_on(&mut tx, binding, scoped_conflict, false, &now.to_rfc3339())
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn scoped_conflicts(
        &self,
        binding: &SyncBinding,
        conflict: &SyncConflict,
    ) -> Result<Vec<SyncConflict>, SyncError> {
        let mut connection = self.pool.acquire().await?;
        conflict_scope::conflicts_on(&mut connection, binding, conflict).await
    }

    pub async fn keep_local_snapshots(
        &self,
        binding: &SyncBinding,
        conflict: &SyncConflict,
        snapshots: Vec<DomainSnapshot>,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<(), SyncError> {
        let mut intents = Vec::with_capacity(snapshots.len());
        for snapshot in snapshots {
            let snapshot = canonical_snapshot_intent(snapshot)?;
            if snapshot.entity.workspace_id != binding.local_workspace_id {
                return Err(SyncError::InvalidData);
            }
            intents.push(snapshot);
        }
        let now = clock.now();
        let mut tx = self.pool.begin().await?;
        Self::assert_binding_generation_on(&mut tx, binding).await?;
        let conflicts = conflict_scope::conflicts_on(&mut tx, binding, conflict).await?;
        if conflicts.is_empty() {
            return Err(SyncError::InvalidData);
        }
        for scoped_conflict in &conflicts {
            let index = intents.iter().position(|snapshot| {
                snapshot.entity.entity_id == scoped_conflict.entity_id
                    && snapshot.intent.entity_type.as_str() == scoped_conflict.entity_type
            });
            let Some(index) = index else {
                return Err(SyncError::InvalidData);
            };
            let snapshot = intents.swap_remove(index);
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
            sqlx::query(
                "UPDATE cloud_sync_outbox SET base_version = ?1 WHERE account_id = ?2 AND cloud_workspace_id = ?3 AND entity_type = ?4 AND entity_id = ?5",
            )
            .bind(scoped_conflict.server_version)
            .bind(&binding.account_id)
            .bind(&binding.cloud_workspace_id)
            .bind(&scoped_conflict.entity_type)
            .bind(&scoped_conflict.entity_id)
            .execute(&mut *tx)
            .await?;
        }
        if !intents.is_empty() {
            return Err(SyncError::InvalidData);
        }
        for scoped_conflict in &conflicts {
            Self::clear_conflict_on(&mut tx, binding, scoped_conflict, false, &now.to_rfc3339())
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn entity_is_deleted_on(
        connection: &mut SqliteConnection,
        entity_type: SyncEntityType,
        workspace_id: &str,
        entity_id: &str,
    ) -> Result<bool, SyncError> {
        let deleted: Option<String> = match entity_type {
            SyncEntityType::Workspace => sqlx::query_scalar("SELECT deleted_at FROM workspaces WHERE id = ?1").bind(entity_id).fetch_one(&mut *connection).await?,
            SyncEntityType::Connection => sqlx::query_scalar("SELECT deleted_at FROM connections WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
            SyncEntityType::WorkspaceVariable => sqlx::query_scalar("SELECT deleted_at FROM workspace_variables WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
            SyncEntityType::WorkspaceEnvironment => sqlx::query_scalar("SELECT deleted_at FROM workspace_environments WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
            SyncEntityType::WorkspaceEnvironmentVariable => sqlx::query_scalar("SELECT deleted_at FROM workspace_environment_variables WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
            SyncEntityType::ApiCollection => sqlx::query_scalar("SELECT deleted_at FROM api_collections WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
            SyncEntityType::ApiFolder => sqlx::query_scalar("SELECT deleted_at FROM api_collection_folders WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
            SyncEntityType::ApiRequest => sqlx::query_scalar("SELECT deleted_at FROM api_requests WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
            SyncEntityType::SshTask => sqlx::query_scalar("SELECT deleted_at FROM ssh_task WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
            SyncEntityType::SshTaskStep => sqlx::query_scalar("SELECT deleted_at FROM ssh_task_step WHERE id = ?1 AND workspace_id = ?2").bind(entity_id).bind(workspace_id).fetch_one(&mut *connection).await?,
        };
        Ok(deleted.is_some())
    }

    pub async fn clear_conflict_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
        conflict: &SyncConflict,
        delete_intent: bool,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"UPDATE cloud_sync_entity_state SET sync_status = 'synced',
                 conflict_remote_payload_json = NULL, conflict_remote_operation = NULL,
                 conflict_parent_entity_id = NULL, conflict_deleted_at = NULL,
                 conflict_operation_id = NULL, updated_at = ?1
               WHERE account_id = ?2 AND cloud_workspace_id = ?3 AND entity_type = ?4 AND entity_id = ?5"#,
        ).bind(now).bind(&binding.account_id).bind(&binding.cloud_workspace_id)
         .bind(&conflict.entity_type).bind(&conflict.entity_id).execute(&mut *connection).await?;
        if delete_intent {
            sqlx::query("DELETE FROM cloud_sync_outbox WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND entity_type = ?3 AND entity_id = ?4")
                .bind(&binding.account_id).bind(&binding.cloud_workspace_id).bind(&conflict.entity_type).bind(&conflict.entity_id).execute(&mut *connection).await?;
        }
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_entity_state WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND sync_status = 'conflict'")
            .bind(&binding.account_id).bind(&binding.cloud_workspace_id).fetch_one(&mut *connection).await?;
        if remaining == 0 {
            sqlx::query(
                r#"UPDATE cloud_sync_workspace_bindings
                   SET state = CASE WHEN EXISTS (
                         SELECT 1 FROM cloud_sync_outbox
                         WHERE account_id = ?2 AND local_workspace_id = ?3 AND status = 'dead'
                       ) THEN 'error' ELSE 'reconciling' END,
                       last_error = CASE WHEN EXISTS (
                         SELECT 1 FROM cloud_sync_outbox
                         WHERE account_id = ?2 AND local_workspace_id = ?3 AND status = 'dead'
                       ) THEN 'cloud_sync_dead_letter_blocked' ELSE NULL END,
                       updated_at = ?1
                   WHERE account_id = ?2 AND local_workspace_id = ?3"#,
            )
            .bind(now)
            .bind(&binding.account_id)
            .bind(&binding.local_workspace_id)
            .execute(&mut *connection)
            .await?;
        }
        Ok(())
    }
}
