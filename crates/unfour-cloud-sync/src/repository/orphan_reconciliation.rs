//! Self-healing for live local entities that predate (or escaped) outbox
//! capture. This is deliberately a repair pass, not a versioned bootstrap:
//! it has no completion marker and is safe to run before every sync.

use sqlx::SqliteConnection;
use unfour_core::domain::{
    DomainEntityKey, DomainEntityType, DomainMutation, DomainSnapshot, MutationOperation,
    MutationOrigin,
};
use unfour_database_engine::DatabaseService;
use unfour_http_engine::ApiClientService;
use unfour_ssh_engine::SshService;

use super::SyncRepository;
use crate::canonical::{canonical_intent_on, canonical_snapshot_intent};
use crate::{Clock, IdGenerator, SyncBinding, SyncEntityType, SyncError, SyncOperation};

impl SyncRepository {
    /// Re-enqueues live local entities that have neither sync state nor an
    /// outbox row for this exact account/cloud binding. The transaction is
    /// fenced by the binding generation so a sign-out, account switch, or
    /// pause cannot repair into a context that is no longer eligible.
    ///
    /// This intentionally does not write a marker. A marker could turn a
    /// transiently incomplete repair into a permanent omission; the query is
    /// cheap and idempotent enough to run before every normal sync pass.
    #[allow(clippy::too_many_arguments)]
    pub async fn reconcile_missing_local_sync_state(
        &self,
        binding: &SyncBinding,
        api_client: &ApiClientService,
        ssh: &SshService,
        database: &DatabaseService,
        ids: &dyn IdGenerator,
        clock: &dyn Clock,
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

        let keys =
            Self::live_syncable_entity_keys_on(&mut tx, &binding.local_workspace_id, ssh).await?;
        let mut repaired = 0;
        for key in keys {
            let entity_type = SyncEntityType::from(key.entity_type);
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

            let (revision, intent) = if matches!(
                key.entity_type,
                DomainEntityType::Workspace
                    | DomainEntityType::WorkspaceVariable
                    | DomainEntityType::WorkspaceEnvironment
                    | DomainEntityType::WorkspaceEnvironmentVariable
            ) {
                let revision =
                    Self::entity_revision_on(&mut tx, &binding.local_workspace_id, &key.entity_id)
                        .await?;
                let mutation = DomainMutation::new(
                    MutationOrigin::Local,
                    MutationOperation::Upsert,
                    key.clone(),
                    revision,
                );
                (revision, canonical_intent_on(&mut tx, &mutation).await?)
            } else {
                let Some(snapshot) =
                    Self::read_feature_snapshot_on(&mut tx, &key, api_client, ssh, database)
                        .await?
                else {
                    // A live-key enumeration and snapshot read can straddle a
                    // local delete. Its delete mutation, if any, owns that
                    // transition; do not manufacture an upsert for a row that
                    // no longer exists.
                    continue;
                };
                let snapshot = canonical_snapshot_intent(snapshot)?;
                if snapshot.intent.operation != SyncOperation::Upsert {
                    continue;
                }
                if snapshot.entity.workspace_id != binding.local_workspace_id
                    || snapshot.entity.entity_type != key.entity_type
                    || snapshot.entity.entity_id != key.entity_id
                {
                    return Err(SyncError::InvalidData);
                }
                (snapshot.revision, snapshot.intent)
            };

            Self::enqueue_intent_on(
                &mut tx,
                &binding.account_id,
                &binding.local_workspace_id,
                &binding.cloud_workspace_id,
                &key.entity_id,
                revision,
                intent,
                ids.next_id(),
                now,
            )
            .await?;
            repaired += 1;
        }

        tx.commit().await?;
        Ok(repaired)
    }

    async fn live_syncable_entity_keys_on(
        connection: &mut SqliteConnection,
        workspace_id: &str,
        ssh: &SshService,
    ) -> Result<Vec<DomainEntityKey>, SyncError> {
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            r#"WITH RECURSIVE folder_depth(id, depth) AS (
                 SELECT id, 0 FROM api_collection_folders
                 WHERE workspace_id = ?1
                   AND parent_folder_id IS NULL AND deleted_at IS NULL
                 UNION ALL
                 SELECT child.id, parent.depth + 1
                 FROM api_collection_folders AS child
                 JOIN folder_depth AS parent ON parent.id = child.parent_folder_id
                 WHERE child.workspace_id = ?1 AND child.deleted_at IS NULL
               ), entities(entity_type, entity_id, parent_entity_id, depth) AS (
                 SELECT 'workspace', id, NULL, 0
                 FROM workspaces
                 WHERE id = ?1 AND deleted_at IS NULL
                 UNION ALL
                 SELECT 'workspaceVariable', id, NULL, 1
                 FROM workspace_variables
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
                 UNION ALL
                 SELECT 'workspaceEnvironment', id, NULL, 1
                 FROM workspace_environments
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
                 UNION ALL
                 SELECT 'workspaceEnvironmentVariable', id, environment_id, 2
                 FROM workspace_environment_variables
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
                 UNION ALL
                 SELECT 'connection', id, NULL, 1
                 FROM connections
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
                 UNION ALL
                 SELECT 'apiCollection', id, ?1, 1
                 FROM api_collections
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
                 UNION ALL
                 SELECT 'apiFolder', id,
                        COALESCE(parent_folder_id, collection_id),
                        2 + COALESCE(
                          (SELECT depth FROM folder_depth
                           WHERE folder_depth.id = api_collection_folders.id),
                          0
                        )
                 FROM api_collection_folders
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
                 UNION ALL
                 SELECT 'apiRequest', id,
                        COALESCE(parent_folder_id, collection_id),
                        3 + COALESCE(
                          (SELECT depth FROM folder_depth
                           WHERE folder_depth.id = api_requests.parent_folder_id),
                          0
                        )
                 FROM api_requests
                 WHERE workspace_id = ?1 AND deleted_at IS NULL
               )
               SELECT entity_type, entity_id, parent_entity_id
               FROM entities
               ORDER BY depth, entity_type, entity_id"#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        if !rows
            .iter()
            .any(|(entity_type, _, _)| entity_type == "workspace")
        {
            // A deleted workspace has no live syncable entities. In
            // particular, do not ask the SSH service to validate a workspace
            // whose tombstone is already the current local state.
            return Ok(Vec::new());
        }

        let mut keys = rows
            .into_iter()
            .map(|(entity_type, entity_id, parent_entity_id)| {
                let entity_type = SyncEntityType::parse(&entity_type)?;
                let mut key = DomainEntityKey::new(
                    DomainEntityType::from(entity_type),
                    workspace_id,
                    entity_id,
                );
                key.parent_entity_id = parent_entity_id;
                Ok(key)
            })
            .collect::<Result<Vec<_>, SyncError>>()?;

        let ssh_keys = ssh
            .list_task_domain_entities_on(connection, workspace_id)
            .await
            .map_err(|_| SyncError::Core)?;
        keys.extend(ssh_keys);
        Ok(keys)
    }

    async fn read_feature_snapshot_on(
        connection: &mut SqliteConnection,
        key: &DomainEntityKey,
        api_client: &ApiClientService,
        ssh: &SshService,
        database: &DatabaseService,
    ) -> Result<Option<DomainSnapshot>, SyncError> {
        let result = match key.entity_type {
            DomainEntityType::ApiCollection
            | DomainEntityType::ApiFolder
            | DomainEntityType::ApiRequest => {
                api_client.read_domain_snapshot_on(connection, key).await
            }
            DomainEntityType::SshTask | DomainEntityType::SshTaskStep => {
                ssh.read_task_domain_snapshot_on(connection, key).await
            }
            DomainEntityType::Connection => {
                let connection_type: Option<String> = sqlx::query_scalar(
                    "SELECT connection_type FROM connections WHERE workspace_id = ?1 AND id = ?2",
                )
                .bind(&key.workspace_id)
                .bind(&key.entity_id)
                .fetch_optional(&mut *connection)
                .await?;
                match connection_type.as_deref() {
                    Some("ssh") => {
                        ssh.read_connection_domain_snapshot_on(connection, key)
                            .await
                    }
                    Some("database") => {
                        database
                            .read_connection_domain_snapshot_on(connection, key)
                            .await
                    }
                    Some(_) => return Err(SyncError::InvalidData),
                    None => return Ok(None),
                }
            }
            _ => return Err(SyncError::InvalidData),
        };

        match result {
            Ok(snapshot) => Ok(Some(snapshot)),
            Err(error) if error.code() == "NOT_FOUND" => Ok(None),
            Err(_) => Err(SyncError::Core),
        }
    }
}
