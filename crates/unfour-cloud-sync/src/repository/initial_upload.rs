//! Create a binding and capture its initial upload set in one transaction.
//! Enumeration and domain snapshots must use that same SQLite connection.

use sqlx::SqliteConnection;
use unfour_core::domain::{
    DomainEntityKey, DomainEntityType, DomainMutation, MutationOperation, MutationOrigin,
};
use unfour_http_engine::ApiClientService;
use unfour_ssh_engine::SshService;

use super::SyncRepository;
use crate::canonical::canonical_snapshot_intent;
use crate::{Clock, IdGenerator, SyncEntityType, SyncError};

impl SyncRepository {
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
        Self::ensure_new_binding_owner_available_on(&mut tx, account_id, workspace_id).await?;
        sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_bindings (
                 account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, state, initial_cursor, ssh_task_v3_bootstrap_state,
                 connection_v4_bootstrap_state, api_v2_bootstrap_state,
                 generation, created_at, updated_at
               ) VALUES (?1, ?2, ?3, ?4, 1, 'preparing', ?4, 'completed',
                         'pending', 'completed', ?5, ?6, ?6)"#,
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
    pub(super) async fn live_api_entity_keys_on(
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
}
