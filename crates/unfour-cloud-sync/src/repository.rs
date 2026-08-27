use chrono::{DateTime, Duration, Utc};
use sqlx::{SqliteConnection, SqlitePool};
use unfour_core::domain::{
    DomainEntityKey, DomainEntityType, DomainMutation, DomainSnapshot, MutationOperation,
    MutationOrigin,
};
use unfour_http_engine::ApiClientService;
use unfour_ssh_engine::SshService;

use crate::canonical::{canonical_intent_on, canonical_snapshot_intent, CanonicalIntent};
use crate::conflict_scope;
use crate::{
    Clock, IdGenerator, OutboxEntry, PushResult, PushResultStatus, RemoteChange, SnapshotItem,
    SyncBinding, SyncConflict, SyncConflictDetails, SyncEntityType, SyncError, SyncStatus,
    PAYLOAD_SCHEMA_VERSION,
};

mod bootstrap;
mod recovery;

const LEASE_DURATION: Duration = Duration::seconds(45);
const DIAGNOSTIC_HISTORY_LIMIT: i64 = 200;

#[derive(Clone)]
pub struct SyncRepository {
    pool: SqlitePool,
}

impl SyncRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

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

    pub async fn create_binding_with_initial_outbox(
        &self,
        account_id: &str,
        account_generation: u64,
        workspace_id: &str,
        cloud_workspace_id: &str,
        cursor: i64,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<(), SyncError> {
        self.create_binding_with_initial_outbox_and_domain_entities(
            account_id,
            account_generation,
            workspace_id,
            cloud_workspace_id,
            cursor,
            None,
            None,
            ids,
            clock,
        )
        .await
    }

    /// Creates the binding and enqueues the initial upload set in ONE
    /// transaction. API and SSH Task entities are enumerated and snapshotted
    /// on the same transaction connection.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_binding_with_initial_outbox_and_domain_entities(
        &self,
        account_id: &str,
        account_generation: u64,
        workspace_id: &str,
        cloud_workspace_id: &str,
        cursor: i64,
        api_client: Option<&ApiClientService>,
        ssh: Option<&SshService>,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<(), SyncError> {
        let now = clock.now();
        let now_text = now.to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_bindings (
                 account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, state, initial_cursor, ssh_task_v3_bootstrap_state,
                 connection_v4_bootstrap_state, generation, created_at, updated_at
               ) VALUES (?1, ?2, ?3, ?4, 1, 'preparing', ?4, 'completed',
                         'pending', ?5, ?6, ?6)"#,
        )
        .bind(account_id)
        .bind(workspace_id)
        .bind(cloud_workspace_id)
        .bind(cursor)
        .bind(account_generation as i64)
        .bind(&now_text)
        .execute(&mut *tx)
        .await?;

        let keys = Self::live_entity_keys_on(&mut tx, workspace_id).await?;
        for key in &keys {
            let revision = Self::entity_revision_on(&mut tx, workspace_id, &key.entity_id).await?;
            let mutation = DomainMutation::new(
                MutationOrigin::Local,
                MutationOperation::Upsert,
                key.clone(),
                revision,
            );
            Self::enqueue_on(
                &mut tx,
                account_id,
                workspace_id,
                cloud_workspace_id,
                &mutation,
                ids.next_id(),
                now,
            )
            .await?;
        }
        let mut snapshot_count = 0;
        if let Some(api_client) = api_client {
            let api_keys = Self::live_api_entity_keys_on(&mut tx, workspace_id).await?;
            snapshot_count = api_keys.len();
            for key in &api_keys {
                let snapshot = api_client
                    .read_domain_snapshot_on(&mut tx, key)
                    .await
                    .map_err(|_| SyncError::Core)?;
                let snapshot = canonical_snapshot_intent(snapshot)?;
                if snapshot.entity.workspace_id != workspace_id
                    || snapshot.intent.operation != crate::SyncOperation::Upsert
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
            }
        }
        if let Some(ssh) = ssh {
            let ssh_keys = ssh
                .list_task_domain_entities_on(&mut tx, workspace_id)
                .await
                .map_err(|_| SyncError::Core)?;
            for key in &ssh_keys {
                if key.workspace_id != workspace_id
                    || !matches!(
                        key.entity_type,
                        DomainEntityType::SshTask | DomainEntityType::SshTaskStep
                    )
                {
                    return Err(SyncError::InvalidData);
                }
                let snapshot = ssh
                    .read_task_domain_snapshot_on(&mut tx, key)
                    .await
                    .map_err(|_| SyncError::Core)?;
                let snapshot = canonical_snapshot_intent(snapshot)?;
                if snapshot.entity.workspace_id != workspace_id {
                    return Err(SyncError::InvalidData);
                }
                if snapshot.intent.operation == crate::SyncOperation::Delete {
                    continue;
                }
                snapshot_count += 1;
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
            }
        }
        let initial_total = keys.len() + snapshot_count;
        sqlx::query(
            "UPDATE cloud_sync_workspace_bindings SET state = 'uploading', initial_total = ?1, updated_at = ?2 WHERE account_id = ?3 AND local_workspace_id = ?4",
        ).bind(initial_total as i64).bind(&now_text).bind(account_id).bind(workspace_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Live API entities in stable parent-before-child order (collections,
    /// folders, requests), read on the caller's transaction connection.
    async fn live_api_entity_keys_on(
        connection: &mut SqliteConnection,
        workspace_id: &str,
    ) -> Result<Vec<DomainEntityKey>, SyncError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            r#"SELECT 'apiCollection' AS kind, id FROM api_collections
               WHERE workspace_id = ?1 AND deleted_at IS NULL
               UNION ALL
               SELECT 'apiFolder', id FROM api_collection_folders
               WHERE workspace_id = ?1 AND deleted_at IS NULL
               UNION ALL
               SELECT 'apiRequest', id FROM api_requests
               WHERE workspace_id = ?1 AND deleted_at IS NULL
               ORDER BY 1, 2"#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        rows.into_iter()
            .map(|(kind, id)| {
                let entity_type = SyncEntityType::parse(&kind)?;
                Ok(DomainEntityKey::new(
                    DomainEntityType::from(entity_type),
                    workspace_id,
                    id,
                ))
            })
            .collect()
    }

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
                   WHERE account_id = ?1 AND local_workspace_id = ?2"#,
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

    async fn entity_revision_on(
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

    pub async fn enqueue_mutations_on(
        connection: &mut SqliteConnection,
        mutations: &[DomainMutation],
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
    ) -> Result<Vec<String>, SyncError> {
        let now = clock.now();
        let mut workspaces = Vec::new();
        for mutation in mutations {
            let bindings: Vec<(String, String, bool)> = sqlx::query_as(
                r#"SELECT binding.account_id, binding.cloud_workspace_id,
                          binding.sync_enabled = 1
                            AND binding.state <> 'paused'
                            AND COALESCE(settings.sync_enabled, 0)
                   FROM cloud_sync_workspace_bindings AS binding
                   JOIN cloud_sync_runtime_context AS runtime
                     ON runtime.singleton = 1
                    AND runtime.active_account_id = binding.account_id
                   LEFT JOIN cloud_sync_account_settings AS settings
                     ON settings.account_id = binding.account_id
                   WHERE binding.local_workspace_id = ?1"#,
            )
            .bind(&mutation.entity.workspace_id)
            .fetch_all(&mut *connection)
            .await?;
            let mut should_trigger = false;
            for (account_id, cloud_workspace_id, network_enabled) in bindings {
                Self::enqueue_on(
                    connection,
                    &account_id,
                    &mutation.entity.workspace_id,
                    &cloud_workspace_id,
                    mutation,
                    ids.next_id(),
                    now,
                )
                .await?;
                should_trigger |= network_enabled;
            }
            if should_trigger && !workspaces.contains(&mutation.entity.workspace_id) {
                workspaces.push(mutation.entity.workspace_id.clone());
            }
        }
        Ok(workspaces)
    }

    async fn enqueue_on(
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
    async fn enqueue_intent_on(
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

    async fn live_entity_keys_on(
        connection: &mut SqliteConnection,
        workspace_id: &str,
    ) -> Result<Vec<DomainEntityKey>, SyncError> {
        let mut keys = vec![DomainEntityKey::new(
            DomainEntityType::Workspace,
            workspace_id,
            workspace_id,
        )];
        let variables: Vec<(String,)> = sqlx::query_as("SELECT id FROM workspace_variables WHERE workspace_id = ?1 AND deleted_at IS NULL ORDER BY id")
            .bind(workspace_id).fetch_all(&mut *connection).await?;
        keys.extend(variables.into_iter().map(|(id,)| {
            DomainEntityKey::new(DomainEntityType::WorkspaceVariable, workspace_id, id)
        }));
        let environments: Vec<(String,)> = sqlx::query_as("SELECT id FROM workspace_environments WHERE workspace_id = ?1 AND deleted_at IS NULL ORDER BY id")
            .bind(workspace_id).fetch_all(&mut *connection).await?;
        keys.extend(environments.into_iter().map(|(id,)| {
            DomainEntityKey::new(DomainEntityType::WorkspaceEnvironment, workspace_id, id)
        }));
        let environment_variables: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, environment_id FROM workspace_environment_variables WHERE workspace_id = ?1 AND deleted_at IS NULL ORDER BY environment_id, id",
        ).bind(workspace_id).fetch_all(&mut *connection).await?;
        keys.extend(environment_variables.into_iter().map(|(id, parent)| {
            DomainEntityKey::new(
                DomainEntityType::WorkspaceEnvironmentVariable,
                workspace_id,
                id,
            )
            .with_parent_entity_id(parent)
        }));
        Ok(keys)
    }

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

    async fn record_remote_state_on(
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

    pub async fn local_workspace_exists_on(
        connection: &mut SqliteConnection,
        workspace_id: &str,
    ) -> Result<bool, SyncError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?1)")
            .bind(workspace_id)
            .fetch_one(&mut *connection)
            .await
            .map_err(Into::into)
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

    pub async fn active_workspace_name_exists_on(
        connection: &mut SqliteConnection,
        name: &str,
    ) -> Result<bool, SyncError> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE name = ?1 COLLATE NOCASE AND deleted_at IS NULL)",
        )
        .bind(name)
        .fetch_one(&mut *connection)
        .await
        .map_err(Into::into)
    }

    pub async fn stage_snapshot_page(
        &self,
        stage_id: &str,
        account_id: &str,
        cloud_workspace_id: &str,
        page_cursor: i64,
        items: &[SnapshotItem],
        now: &str,
    ) -> Result<(), SyncError> {
        let mut tx = self.pool.begin().await?;
        for item in items {
            let payload =
                serde_json::to_string(&item.payload).map_err(|_| SyncError::InvalidData)?;
            sqlx::query(
                r#"INSERT INTO cloud_sync_snapshot_staging (
                     stage_id, account_id, cloud_workspace_id, at_cursor, entity_type,
                     entity_id, parent_entity_id, server_version, payload_schema_version,
                     payload_json, topology_rank, created_at
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
            )
            .bind(stage_id)
            .bind(account_id)
            .bind(cloud_workspace_id)
            .bind(page_cursor)
            .bind(item.entity_type.as_str())
            .bind(&item.entity_id)
            .bind(&item.parent_entity_id)
            .bind(item.server_version)
            .bind(item.payload_schema_version)
            .bind(payload)
            .bind(item.entity_type.topology_rank())
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn staged_snapshot_chunk(
        &self,
        stage_id: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<(String, String, Option<String>, i64, i64, String)>, SyncError> {
        sqlx::query_as(
            r#"SELECT entity_type, entity_id, parent_entity_id, server_version,
                      payload_schema_version, payload_json
               FROM cloud_sync_snapshot_staging WHERE stage_id = ?1
               ORDER BY topology_rank, entity_type, entity_id LIMIT ?2 OFFSET ?3"#,
        )
        .bind(stage_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn clear_snapshot_stage(&self, stage_id: &str) -> Result<(), SyncError> {
        sqlx::query("DELETE FROM cloud_sync_snapshot_staging WHERE stage_id = ?1")
            .bind(stage_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_download_binding_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        account_generation: u64,
        workspace_id: &str,
        cloud_workspace_id: &str,
        cursor: i64,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_bindings (
                 account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, state, initial_cursor, initial_total, initial_confirmed,
                 ssh_task_v3_bootstrap_state, connection_v4_bootstrap_state,
                 generation, last_success_at, created_at, updated_at
               ) VALUES (?1, ?2, ?3, ?4, 1, 'reconciling', ?4, 0, 0,
                         'completed', 'completed', ?5, ?6, ?6, ?6)"#,
        )
        .bind(account_id)
        .bind(workspace_id)
        .bind(cloud_workspace_id)
        .bind(cursor)
        .bind(account_generation as i64)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        Ok(())
    }

    pub async fn record_snapshot_state_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        cloud_workspace_id: &str,
        item: &SnapshotItem,
        now: &str,
    ) -> Result<(), SyncError> {
        sqlx::query(
            r#"INSERT INTO cloud_sync_entity_state (
                 account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                 sync_status, updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?5, 'synced', ?6)
               ON CONFLICT(account_id, cloud_workspace_id, entity_type, entity_id) DO UPDATE SET
                 server_version = excluded.server_version, sync_status = 'synced', updated_at = excluded.updated_at"#,
        ).bind(account_id).bind(cloud_workspace_id).bind(item.entity_type.as_str())
         .bind(&item.entity_id).bind(item.server_version).bind(now).execute(&mut *connection).await?;
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

    async fn record_diagnostic_on(
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
