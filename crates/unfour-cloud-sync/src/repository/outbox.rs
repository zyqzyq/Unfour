//! Durable local intent: capture/coalesce, materialize and select eligible heads.
//! Network attempt state lives in `attempts`; selection retains dependency ordering.

use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;

use super::SyncRepository;
use crate::canonical::{canonical_intent_on, canonical_snapshot_intent, CanonicalIntent};
use crate::{Clock, IdGenerator, OutboxEntry, SyncError, PAYLOAD_SCHEMA_VERSION};
use unfour_core::domain::{DomainMutation, DomainSnapshot};

impl SyncRepository {
    pub async fn enqueue_mutations_on(
        connection: &mut SqliteConnection,
        mutations: &[DomainMutation],
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<Vec<String>, SyncError> {
        let now = clock.now();
        let mut workspaces = Vec::new();
        for mutation in mutations {
            let Some(owner) =
                Self::resolve_cloud_sync_owner_on(connection, &mutation.entity.workspace_id)
                    .await?
            else {
                continue;
            };
            let network_enabled: bool = sqlx::query_scalar(
                r#"SELECT EXISTS (
                     SELECT 1
                     FROM cloud_sync_workspace_bindings AS binding
                     LEFT JOIN cloud_sync_account_settings AS settings
                       ON settings.account_id = binding.account_id
                     WHERE binding.account_id = ?1
                       AND binding.local_workspace_id = ?2
                       AND binding.cloud_workspace_id = ?3
                       AND binding.sync_enabled = 1
                       AND binding.state <> 'paused'
                       AND COALESCE(settings.sync_enabled, 0) = 1
                       AND EXISTS (
                         SELECT 1 FROM cloud_sync_runtime_context AS runtime
                         WHERE runtime.singleton = 1
                           AND runtime.active_account_id = binding.account_id
                       )
                   )"#,
            )
            .bind(&owner.account_id)
            .bind(&mutation.entity.workspace_id)
            .bind(&owner.cloud_workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            Self::enqueue_on(
                connection,
                &owner.account_id,
                &mutation.entity.workspace_id,
                &owner.cloud_workspace_id,
                mutation,
                ids.next_id(),
                now,
            )
            .await?;
            if network_enabled && !workspaces.contains(&mutation.entity.workspace_id) {
                workspaces.push(mutation.entity.workspace_id.clone());
            }
        }
        Ok(workspaces)
    }

    pub(super) async fn enqueue_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        workspace_id: &str,
        cloud_workspace_id: &str,
        mutation: &DomainMutation,
        operation_id: String,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let intent = canonical_intent_on(connection, mutation).await?;
        Self::enqueue_intent_on(
            connection,
            account_id,
            workspace_id,
            cloud_workspace_id,
            &mutation.entity.entity_id,
            mutation.revision,
            intent,
            operation_id,
            now,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn enqueue_intent_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        workspace_id: &str,
        cloud_workspace_id: &str,
        entity_id: &str,
        content_revision: i64,
        intent: CanonicalIntent,
        operation_id: String,
        now: DateTime<Utc>,
    ) -> Result<(), SyncError> {
        let base_version = sqlx::query_scalar::<_, i64>(
            r#"SELECT server_version FROM cloud_sync_entity_state
               WHERE account_id = ?1 AND cloud_workspace_id = ?2 AND entity_type = ?3 AND entity_id = ?4"#,
        ).bind(account_id).bind(cloud_workspace_id).bind(intent.entity_type.as_str())
         .bind(entity_id).fetch_optional(&mut *connection).await?.unwrap_or(0);
        let now = now.to_rfc3339();
        sqlx::query(
            r#"INSERT INTO cloud_sync_outbox (
                 account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
                 operation_id, parent_entity_id, operation, base_version,
                 payload_schema_version, canonical_payload_json, deleted_at, content_revision,
                 status, created_at, updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'pending', ?14, ?14)
               ON CONFLICT(account_id, cloud_workspace_id, entity_type, entity_id) DO UPDATE SET
                 operation_id = excluded.operation_id,
                 parent_entity_id = excluded.parent_entity_id,
                 operation = excluded.operation,
                 base_version = MAX(cloud_sync_outbox.base_version, excluded.base_version),
                 canonical_payload_json = excluded.canonical_payload_json,
                 deleted_at = excluded.deleted_at,
                 content_revision = excluded.content_revision,
                 status = 'pending', attempt_count = 0, next_attempt_at = NULL,
                 lease_owner = NULL, lease_started_at = NULL, lease_expires_at = NULL,
                 last_error = NULL, updated_at = excluded.updated_at
               WHERE excluded.content_revision >= cloud_sync_outbox.content_revision"#,
        )
        .bind(account_id).bind(workspace_id).bind(cloud_workspace_id).bind(intent.entity_type.as_str())
        .bind(entity_id).bind(operation_id).bind(intent.parent_entity_id)
        .bind(intent.operation.as_str()).bind(base_version).bind(PAYLOAD_SCHEMA_VERSION)
        .bind(intent.payload_json).bind(intent.deleted_at).bind(content_revision).bind(now)
        .execute(&mut *connection).await?;
        Ok(())
    }

    /// Fills a deferred Core-domain outbox row from a canonical snapshot before
    /// its first network attempt. Once materialized, uncertain retries reuse
    /// the immutable payload already stored under the same operation id.
    pub async fn materialize_outbox_entry(
        &self,
        entry: &OutboxEntry,
        snapshot: DomainSnapshot,
        now: DateTime<Utc>,
    ) -> Result<Option<OutboxEntry>, SyncError> {
        let snapshot = canonical_snapshot_intent(snapshot)?;
        if snapshot.entity.workspace_id != entry.local_workspace_id
            || snapshot.entity.entity_id != entry.entity_id
            || snapshot.intent.entity_type.as_str() != entry.entity_type
            || snapshot.intent.operation.as_str() != entry.operation
            || snapshot.revision != entry.content_revision
        {
            return Ok(None);
        }
        let changed = sqlx::query(
            r#"UPDATE cloud_sync_outbox SET parent_entity_id = ?1,
                 canonical_payload_json = ?2, deleted_at = ?3, updated_at = ?4
               WHERE account_id = ?5 AND local_workspace_id = ?6
                 AND operation_id = ?7 AND content_revision = ?8
                 AND status IN ('pending', 'uncertain')"#,
        )
        .bind(&snapshot.intent.parent_entity_id)
        .bind(&snapshot.intent.payload_json)
        .bind(&snapshot.intent.deleted_at)
        .bind(now.to_rfc3339())
        .bind(&entry.account_id)
        .bind(&entry.local_workspace_id)
        .bind(&entry.operation_id)
        .bind(entry.content_revision)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if changed != 1 {
            return Ok(None);
        }
        let mut materialized = entry.clone();
        materialized.parent_entity_id = snapshot.intent.parent_entity_id;
        materialized.canonical_payload_json = snapshot.intent.payload_json;
        materialized.deleted_at = snapshot.intent.deleted_at;
        Ok(Some(materialized))
    }

    pub async fn due_outbox(
        &self,
        account_id: &str,
        cloud_workspace_id: &str,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<OutboxEntry>, SyncError> {
        sqlx::query_as::<_, OutboxEntry>(
            r#"WITH RECURSIVE folder_depth(id, depth) AS (
                 SELECT id, 0 FROM api_collection_folders WHERE parent_folder_id IS NULL
                 UNION
                 SELECT folders.id, depth.depth + 1
                 FROM api_collection_folders AS folders
                 JOIN folder_depth AS depth ON folders.parent_folder_id = depth.id
               )
               , dead_parents(entity_type, entity_id, local_workspace_id) AS (
                 SELECT entity_type, entity_id, local_workspace_id
                 FROM cloud_sync_outbox
                 WHERE account_id = ?1 AND cloud_workspace_id = ?2
                   AND status = 'dead'
               )
               , dead_folder_tree(dead_id, local_workspace_id, descendant_id) AS (
                 SELECT entity_id, local_workspace_id, entity_id
                 FROM dead_parents
                 WHERE entity_type = 'apiFolder'
                 UNION
                 SELECT tree.dead_id, tree.local_workspace_id, folders.id
                 FROM dead_folder_tree AS tree
                 JOIN api_collection_folders AS folders
                   ON folders.workspace_id = tree.local_workspace_id
                  AND folders.parent_folder_id = tree.descendant_id
               )
               SELECT outbox.account_id, outbox.operation_id, outbox.local_workspace_id,
                      outbox.cloud_workspace_id, outbox.entity_type, outbox.entity_id,
                      outbox.parent_entity_id, outbox.operation, outbox.base_version,
                      outbox.payload_schema_version, outbox.canonical_payload_json,
                      outbox.deleted_at, outbox.content_revision, outbox.status,
                      outbox.attempt_count, outbox.last_error
               FROM cloud_sync_outbox AS outbox
               LEFT JOIN folder_depth ON folder_depth.id = outbox.entity_id
               WHERE outbox.account_id = ?1 AND outbox.cloud_workspace_id = ?2
                 AND outbox.status IN ('pending', 'uncertain')
                 AND (outbox.next_attempt_at IS NULL OR outbox.next_attempt_at <= ?3)
                 AND EXISTS (
                   SELECT 1 FROM cloud_sync_workspace_ownership AS owner
                   WHERE owner.local_workspace_id = outbox.local_workspace_id
                     AND owner.account_id = outbox.account_id
                     AND owner.cloud_workspace_id = outbox.cloud_workspace_id
                 )
                 AND NOT EXISTS (
                   SELECT 1 FROM cloud_sync_entity_state AS state
                   WHERE state.account_id = outbox.account_id
                     AND state.cloud_workspace_id = outbox.cloud_workspace_id
                     AND state.sync_status = 'conflict'
                     AND (
                       (state.entity_type = outbox.entity_type AND state.entity_id = outbox.entity_id)
                       OR (
                         state.conflict_remote_operation = 'delete'
                         AND outbox.entity_type IN ('apiFolder', 'apiRequest', 'sshTaskStep')
                         AND (
                           (
                             state.entity_type = 'apiCollection'
                             AND (
                               EXISTS (
                                 SELECT 1 FROM api_collection_folders AS folders
                                 WHERE folders.id = outbox.entity_id
                                   AND folders.collection_id = state.entity_id
                                   AND folders.workspace_id = outbox.local_workspace_id
                               )
                               OR EXISTS (
                                 SELECT 1 FROM api_requests AS requests
                                 WHERE requests.id = outbox.entity_id
                                   AND requests.collection_id = state.entity_id
                                   AND requests.workspace_id = outbox.local_workspace_id
                               )
                             )
                           )
                           OR (
                             state.entity_type = 'apiFolder'
                             AND EXISTS (
                               WITH RECURSIVE folder_tree(id) AS (
                                 SELECT id FROM api_collection_folders
                                 WHERE id = state.entity_id
                                   AND workspace_id = outbox.local_workspace_id
                                 UNION
                                 SELECT folders.id FROM api_collection_folders AS folders
                                 JOIN folder_tree AS tree ON folders.parent_folder_id = tree.id
                                 WHERE folders.workspace_id = outbox.local_workspace_id
                               )
                               SELECT 1 FROM folder_tree
                               WHERE folder_tree.id = outbox.entity_id
                               UNION ALL
                               SELECT 1 FROM api_requests AS requests
                               WHERE requests.id = outbox.entity_id
                                 AND requests.parent_folder_id IN (SELECT id FROM folder_tree)
                             )
                           )
                           OR (
                             state.entity_type = 'sshTask'
                             AND outbox.entity_type = 'sshTaskStep'
                             AND outbox.parent_entity_id = state.entity_id
                           )
                         )
                       )
                     )
                 )
                 AND NOT EXISTS (
                   SELECT 1 FROM dead_parents AS dead
                   WHERE (
                     dead.entity_type = 'workspace'
                     AND outbox.entity_type <> 'workspace'
                   )
                   OR (
                     dead.entity_type = 'workspaceEnvironment'
                     AND outbox.entity_type = 'workspaceEnvironmentVariable'
                     AND outbox.parent_entity_id = dead.entity_id
                   )
                   OR (
                     dead.entity_type = 'apiCollection'
                     AND outbox.entity_type IN ('apiFolder', 'apiRequest')
                     AND (
                       outbox.parent_entity_id = dead.entity_id
                       OR (
                         outbox.entity_type = 'apiFolder'
                         AND EXISTS (
                           SELECT 1 FROM api_collection_folders AS folders
                           WHERE folders.workspace_id = outbox.local_workspace_id
                             AND folders.id = outbox.entity_id
                             AND folders.collection_id = dead.entity_id
                         )
                       )
                       OR (
                         outbox.entity_type = 'apiRequest'
                         AND EXISTS (
                           SELECT 1 FROM api_requests AS requests
                           WHERE requests.workspace_id = outbox.local_workspace_id
                             AND requests.id = outbox.entity_id
                             AND requests.collection_id = dead.entity_id
                         )
                       )
                     )
                   )
                   OR (
                     dead.entity_type = 'apiFolder'
                     AND outbox.entity_type IN ('apiFolder', 'apiRequest')
                     AND (
                       outbox.parent_entity_id = dead.entity_id
                       OR (
                         outbox.entity_type = 'apiFolder'
                         AND EXISTS (
                           SELECT 1 FROM dead_folder_tree AS tree
                           WHERE tree.dead_id = dead.entity_id
                             AND tree.local_workspace_id = outbox.local_workspace_id
                             AND tree.descendant_id = outbox.entity_id
                         )
                       )
                       OR (
                         outbox.entity_type = 'apiRequest'
                         AND EXISTS (
                           SELECT 1 FROM dead_folder_tree AS tree
                           WHERE tree.dead_id = dead.entity_id
                             AND tree.local_workspace_id = outbox.local_workspace_id
                             AND tree.descendant_id = outbox.parent_entity_id
                         )
                       )
                     )
                   )
                   OR (
                     dead.entity_type = 'sshTask'
                     AND outbox.entity_type = 'sshTaskStep'
                     AND outbox.parent_entity_id = dead.entity_id
                   )
                 )
               ORDER BY CASE WHEN outbox.operation = 'delete' THEN 3 - CASE outbox.entity_type
                                WHEN 'workspace' THEN 0
                                WHEN 'workspaceEnvironmentVariable' THEN 2
                                WHEN 'apiFolder' THEN 2
                                WHEN 'apiRequest' THEN 3
                                WHEN 'sshTaskStep' THEN 2
                                ELSE 1 END
                             ELSE CASE outbox.entity_type
                                WHEN 'workspace' THEN 0
                                WHEN 'workspaceEnvironmentVariable' THEN 2
                                WHEN 'apiFolder' THEN 2
                                WHEN 'apiRequest' THEN 3
                                WHEN 'sshTaskStep' THEN 2
                                ELSE 1 END END,
                        CASE
                          WHEN outbox.entity_type = 'apiFolder' AND outbox.operation = 'upsert'
                            THEN COALESCE(folder_depth.depth, 0)
                          WHEN outbox.entity_type = 'apiFolder' AND outbox.operation = 'delete'
                            THEN -COALESCE(folder_depth.depth, 0)
                          ELSE 0
                        END,
                        outbox.created_at, outbox.entity_id LIMIT ?4"#,
        ).bind(account_id).bind(cloud_workspace_id).bind(now.to_rfc3339()).bind(limit)
         .fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub(super) async fn entity_revision_on(
        connection: &mut SqliteConnection,
        workspace_id: &str,
        entity_id: &str,
    ) -> Result<i64, SyncError> {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT revision FROM workspaces WHERE id = ?1 AND id = ?2
               UNION ALL SELECT revision FROM workspace_variables WHERE id = ?2 AND workspace_id = ?1
               UNION ALL SELECT revision FROM workspace_environments WHERE id = ?2 AND workspace_id = ?1
               UNION ALL SELECT revision FROM workspace_environment_variables WHERE id = ?2 AND workspace_id = ?1
               UNION ALL SELECT revision FROM api_collections WHERE id = ?2 AND workspace_id = ?1
               UNION ALL SELECT revision FROM api_collection_folders WHERE id = ?2 AND workspace_id = ?1
               UNION ALL SELECT revision FROM api_requests WHERE id = ?2 AND workspace_id = ?1
               UNION ALL SELECT revision FROM connections WHERE id = ?2 AND workspace_id = ?1
               UNION ALL SELECT revision FROM ssh_task WHERE id = ?2 AND workspace_id = ?1
               UNION ALL SELECT revision FROM ssh_task_step WHERE id = ?2 AND workspace_id = ?1 LIMIT 1"#,
        ).bind(workspace_id).bind(entity_id).fetch_one(&mut *connection).await.map_err(Into::into)
    }
}
