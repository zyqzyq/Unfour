//! Single source of truth for the workspace entities supported by Cloud Sync.
//!
//! The descriptor is intentionally data-oriented. Entity-specific business
//! snapshot logic remains in the owning domain service; this module only
//! selects that adapter and centralizes the metadata used by sync orchestration.

use sqlx::SqliteConnection;
use unfour_core::domain::{DomainEntityKey, DomainEntityType, DomainSnapshot};
use unfour_database_engine::DatabaseService;
use unfour_http_engine::ApiClientService;
use unfour_ssh_engine::SshService;
use unfour_workspace_engine::WorkspaceService;

use crate::{SyncEntityType, SyncError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentDependency {
    None,
    Workspace,
    Entity(&'static [SyncEntityType]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotProvider {
    Workspace,
    Connection,
    ApiClient,
    SshTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPolicy {
    NeverReplaceRemoteAbsence,
    Leaf,
    EnvironmentChildren,
    ApiSubtree,
    SshTaskChildren,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalEntityStorage {
    pub table: &'static str,
    pub workspace_column: Option<&'static str>,
    pub parent_expression: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncEntityDescriptor {
    pub entity_type: SyncEntityType,
    pub domain_entity_type: DomainEntityType,
    pub wire_name: &'static str,
    /// Current canonical payload schema understood and emitted for this entity.
    /// Entity schemas evolve independently without changing Protocol 5.
    pub payload_schema_version: i64,
    pub workspace_scoped: bool,
    pub parent_dependency: ParentDependency,
    pub initial_upload: bool,
    pub supports_tombstone: bool,
    pub snapshot_provider: SnapshotProvider,
    pub recovery_policy: RecoveryPolicy,
    pub topology_rank: i64,
    pub local_storage: LocalEntityStorage,
}

const WORKSPACE_ENVIRONMENT_PARENT: &[SyncEntityType] = &[SyncEntityType::WorkspaceEnvironment];
const API_FOLDER_PARENTS: &[SyncEntityType] =
    &[SyncEntityType::ApiCollection, SyncEntityType::ApiFolder];
const SSH_TASK_PARENT: &[SyncEntityType] = &[SyncEntityType::SshTask];

/// The only complete list of entity types supported by Cloud Sync.
pub const SYNC_ENTITY_REGISTRY: &[SyncEntityDescriptor] = &[
    descriptor(
        SyncEntityType::Workspace,
        DomainEntityType::Workspace,
        "workspace",
        1,
        false,
        ParentDependency::None,
        SnapshotProvider::Workspace,
        RecoveryPolicy::NeverReplaceRemoteAbsence,
        0,
        "workspaces",
        None,
        "NULL",
    ),
    descriptor(
        SyncEntityType::Connection,
        DomainEntityType::Connection,
        "connection",
        1,
        true,
        ParentDependency::None,
        SnapshotProvider::Connection,
        RecoveryPolicy::Leaf,
        1,
        "connections",
        Some("workspace_id"),
        "NULL",
    ),
    descriptor(
        SyncEntityType::WorkspaceVariable,
        DomainEntityType::WorkspaceVariable,
        "workspaceVariable",
        1,
        true,
        ParentDependency::Workspace,
        SnapshotProvider::Workspace,
        RecoveryPolicy::Leaf,
        1,
        "workspace_variables",
        Some("workspace_id"),
        "NULL",
    ),
    descriptor(
        SyncEntityType::WorkspaceEnvironment,
        DomainEntityType::WorkspaceEnvironment,
        "workspaceEnvironment",
        1,
        true,
        ParentDependency::Workspace,
        SnapshotProvider::Workspace,
        RecoveryPolicy::EnvironmentChildren,
        1,
        "workspace_environments",
        Some("workspace_id"),
        "NULL",
    ),
    descriptor(
        SyncEntityType::WorkspaceEnvironmentVariable,
        DomainEntityType::WorkspaceEnvironmentVariable,
        "workspaceEnvironmentVariable",
        1,
        true,
        ParentDependency::Entity(WORKSPACE_ENVIRONMENT_PARENT),
        SnapshotProvider::Workspace,
        RecoveryPolicy::Leaf,
        2,
        "workspace_environment_variables",
        Some("workspace_id"),
        "environment_id",
    ),
    descriptor(
        SyncEntityType::ApiCollection,
        DomainEntityType::ApiCollection,
        "apiCollection",
        1,
        true,
        ParentDependency::Workspace,
        SnapshotProvider::ApiClient,
        RecoveryPolicy::ApiSubtree,
        1,
        "api_collections",
        Some("workspace_id"),
        "NULL",
    ),
    descriptor(
        SyncEntityType::ApiFolder,
        DomainEntityType::ApiFolder,
        "apiFolder",
        1,
        true,
        ParentDependency::Entity(API_FOLDER_PARENTS),
        SnapshotProvider::ApiClient,
        RecoveryPolicy::ApiSubtree,
        2,
        "api_collection_folders",
        Some("workspace_id"),
        "COALESCE(parent_folder_id, collection_id)",
    ),
    descriptor(
        SyncEntityType::ApiRequest,
        DomainEntityType::ApiRequest,
        "apiRequest",
        1,
        true,
        ParentDependency::Entity(API_FOLDER_PARENTS),
        SnapshotProvider::ApiClient,
        RecoveryPolicy::Leaf,
        3,
        "api_requests",
        Some("workspace_id"),
        "COALESCE(parent_folder_id, collection_id)",
    ),
    descriptor(
        SyncEntityType::SshTask,
        DomainEntityType::SshTask,
        "sshTask",
        1,
        true,
        ParentDependency::None,
        SnapshotProvider::SshTask,
        RecoveryPolicy::SshTaskChildren,
        1,
        "ssh_task",
        Some("workspace_id"),
        "NULL",
    ),
    descriptor(
        SyncEntityType::SshTaskStep,
        DomainEntityType::SshTaskStep,
        "sshTaskStep",
        1,
        true,
        ParentDependency::Entity(SSH_TASK_PARENT),
        SnapshotProvider::SshTask,
        RecoveryPolicy::Leaf,
        2,
        "ssh_task_step",
        Some("workspace_id"),
        "task_id",
    ),
];

#[allow(clippy::too_many_arguments)]
const fn descriptor(
    entity_type: SyncEntityType,
    domain_entity_type: DomainEntityType,
    wire_name: &'static str,
    payload_schema_version: i64,
    workspace_scoped: bool,
    parent_dependency: ParentDependency,
    snapshot_provider: SnapshotProvider,
    recovery_policy: RecoveryPolicy,
    topology_rank: i64,
    table: &'static str,
    workspace_column: Option<&'static str>,
    parent_expression: &'static str,
) -> SyncEntityDescriptor {
    SyncEntityDescriptor {
        entity_type,
        domain_entity_type,
        wire_name,
        payload_schema_version,
        workspace_scoped,
        parent_dependency,
        initial_upload: true,
        supports_tombstone: true,
        snapshot_provider,
        recovery_policy,
        topology_rank,
        local_storage: LocalEntityStorage {
            table,
            workspace_column,
            parent_expression,
        },
    }
}

pub fn sync_entity_descriptor(entity_type: SyncEntityType) -> &'static SyncEntityDescriptor {
    SYNC_ENTITY_REGISTRY
        .iter()
        .find(|descriptor| descriptor.entity_type == entity_type)
        .expect("every SyncEntityType must be registered")
}

pub fn sync_entity_descriptor_for_domain(
    entity_type: DomainEntityType,
) -> &'static SyncEntityDescriptor {
    SYNC_ENTITY_REGISTRY
        .iter()
        .find(|descriptor| descriptor.domain_entity_type == entity_type)
        .expect("every syncable DomainEntityType must be registered")
}

pub struct SyncEntityAdapters<'a> {
    pub workspace: &'a WorkspaceService,
    pub api_client: &'a ApiClientService,
    pub ssh: &'a SshService,
    pub database: &'a DatabaseService,
}

impl<'a> SyncEntityAdapters<'a> {
    pub fn new(
        workspace: &'a WorkspaceService,
        api_client: &'a ApiClientService,
        ssh: &'a SshService,
        database: &'a DatabaseService,
    ) -> Self {
        Self {
            workspace,
            api_client,
            ssh,
            database,
        }
    }

    pub(crate) async fn read_snapshot_on(
        &self,
        connection: &mut SqliteConnection,
        key: &DomainEntityKey,
    ) -> Result<Option<DomainSnapshot>, SyncError> {
        let descriptor = sync_entity_descriptor_for_domain(key.entity_type);
        let result = match descriptor.snapshot_provider {
            SnapshotProvider::Workspace => self.workspace.read_snapshot_on(connection, key).await,
            SnapshotProvider::ApiClient => {
                self.api_client
                    .read_domain_snapshot_on(connection, key)
                    .await
            }
            SnapshotProvider::SshTask => {
                self.ssh.read_task_domain_snapshot_on(connection, key).await
            }
            SnapshotProvider::Connection => {
                let connection_type: Option<String> = sqlx::query_scalar(
                    "SELECT connection_type FROM connections WHERE workspace_id = ?1 AND id = ?2",
                )
                .bind(&key.workspace_id)
                .bind(&key.entity_id)
                .fetch_optional(&mut *connection)
                .await?;
                match connection_type.as_deref() {
                    Some("ssh") => {
                        self.ssh
                            .read_connection_domain_snapshot_on(connection, key)
                            .await
                    }
                    Some("database") => {
                        self.database
                            .read_connection_domain_snapshot_on(connection, key)
                            .await
                    }
                    Some(_) => return Err(SyncError::InvalidData),
                    None => return Ok(None),
                }
            }
        };
        match result {
            Ok(snapshot) => Ok(Some(snapshot)),
            Err(error) if error.code() == "NOT_FOUND" => Ok(None),
            Err(_) => Err(SyncError::Core),
        }
    }
}

pub(crate) async fn live_syncable_entity_keys_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
) -> Result<Vec<DomainEntityKey>, SyncError> {
    let mut keys = Vec::new();
    for descriptor in SYNC_ENTITY_REGISTRY
        .iter()
        .filter(|descriptor| descriptor.initial_upload)
    {
        let storage = descriptor.local_storage;
        let predicate = storage.workspace_column.unwrap_or("id");
        let sql = format!(
            "SELECT id, {} FROM {} WHERE {} = ?1 AND deleted_at IS NULL ORDER BY id",
            storage.parent_expression, storage.table, predicate
        );
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(&sql)
            .bind(workspace_id)
            .fetch_all(&mut *connection)
            .await?;
        if descriptor.entity_type == SyncEntityType::Workspace && rows.is_empty() {
            return Ok(Vec::new());
        }
        keys.extend(rows.into_iter().map(|(entity_id, parent_entity_id)| {
            let mut key =
                DomainEntityKey::new(descriptor.domain_entity_type, workspace_id, entity_id);
            key.parent_entity_id = parent_entity_id;
            key
        }));
    }
    keys.sort_by_key(|key| {
        let descriptor = sync_entity_descriptor_for_domain(key.entity_type);
        (
            descriptor.topology_rank,
            descriptor.wire_name,
            key.entity_id.clone(),
        )
    });
    Ok(keys)
}

pub(crate) async fn local_revision_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    entity_type: SyncEntityType,
    entity_id: &str,
) -> Result<i64, SyncError> {
    let storage = sync_entity_descriptor(entity_type).local_storage;
    let (sql, workspace) = match storage.workspace_column {
        Some(column) => (
            format!(
                "SELECT revision FROM {} WHERE id = ?1 AND {} = ?2",
                storage.table, column
            ),
            Some(workspace_id),
        ),
        None => (
            format!("SELECT revision FROM {} WHERE id = ?1", storage.table),
            None,
        ),
    };
    let query = sqlx::query_scalar::<_, i64>(&sql).bind(entity_id);
    match workspace {
        Some(workspace_id) => query.bind(workspace_id).fetch_one(&mut *connection).await,
        None => query.fetch_one(&mut *connection).await,
    }
    .map_err(Into::into)
}

pub(crate) async fn local_is_deleted_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    entity_type: SyncEntityType,
    entity_id: &str,
) -> Result<bool, SyncError> {
    let storage = sync_entity_descriptor(entity_type).local_storage;
    let (sql, workspace) = match storage.workspace_column {
        Some(column) => (
            format!(
                "SELECT deleted_at FROM {} WHERE id = ?1 AND {} = ?2",
                storage.table, column
            ),
            Some(workspace_id),
        ),
        None => (
            format!("SELECT deleted_at FROM {} WHERE id = ?1", storage.table),
            None,
        ),
    };
    let query = sqlx::query_scalar::<_, Option<String>>(&sql).bind(entity_id);
    let deleted = match workspace {
        Some(workspace_id) => query.bind(workspace_id).fetch_one(&mut *connection).await,
        None => query.fetch_one(&mut *connection).await,
    }?;
    Ok(deleted.is_some())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn registry_metadata_is_complete_and_valid() {
        let types = SYNC_ENTITY_REGISTRY
            .iter()
            .map(|descriptor| descriptor.entity_type)
            .collect::<HashSet<_>>();
        let wire_names = SYNC_ENTITY_REGISTRY
            .iter()
            .map(|descriptor| descriptor.wire_name)
            .collect::<HashSet<_>>();
        assert_eq!(types.len(), SYNC_ENTITY_REGISTRY.len());
        assert_eq!(wire_names.len(), SYNC_ENTITY_REGISTRY.len());
        assert_eq!(
            types,
            DomainEntityType::ALL
                .into_iter()
                .map(SyncEntityType::from)
                .collect::<HashSet<_>>()
        );
        assert!(SYNC_ENTITY_REGISTRY.iter().all(|descriptor| {
            descriptor.payload_schema_version >= 1
                && descriptor.initial_upload
                && descriptor.supports_tombstone
        }));
        for descriptor in SYNC_ENTITY_REGISTRY {
            assert_eq!(
                serde_json::to_value(descriptor.entity_type).unwrap(),
                descriptor.wire_name
            );
            assert_eq!(
                serde_json::from_value::<SyncEntityType>(descriptor.wire_name.into()).unwrap(),
                descriptor.entity_type
            );
        }
        assert!(serde_json::from_str::<SyncEntityType>(r#""futureEntity""#).is_err());
        for descriptor in SYNC_ENTITY_REGISTRY {
            if let ParentDependency::Entity(parents) = descriptor.parent_dependency {
                assert!(!parents.is_empty());
                assert!(parents.iter().all(|parent| types.contains(parent)));
            }
        }

        let api_request = *sync_entity_descriptor(SyncEntityType::ApiRequest);
        let api_request_v2 = SyncEntityDescriptor {
            payload_schema_version: 2,
            ..api_request
        };
        assert_eq!(api_request_v2.payload_schema_version, 2);
        assert_eq!(
            sync_entity_descriptor(SyncEntityType::Workspace).payload_schema_version,
            1
        );
        assert_eq!(
            sync_entity_descriptor(SyncEntityType::SshTask).payload_schema_version,
            1
        );
    }
}
