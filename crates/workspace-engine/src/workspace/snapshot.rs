use sqlx::SqliteConnection;
use unfour_core::domain::{
    DomainEntityKey, DomainEntityType, DomainSnapshot, SnapshotVariableValue, TombstoneSnapshot,
    WorkspaceEnvironmentSnapshot, WorkspaceEnvironmentVariableSnapshot, WorkspaceSnapshot,
    WorkspaceVariableSnapshot,
};
use unfour_core::models::{WorkspaceEnvironmentVariable, WorkspaceVariable};
use unfour_core::{AppError, AppResult};

use super::variables::WorkspaceEnvironmentRow;
use super::{get_workspace_on, WorkspaceService};

impl WorkspaceService {
    pub async fn read_snapshot(&self, key: &DomainEntityKey) -> AppResult<DomainSnapshot> {
        let mut connection = self.db.pool().acquire().await?;
        self.read_snapshot_on(&mut connection, key).await
    }

    /// Reads a workspace-owned domain snapshot on the caller's transaction.
    /// Cloud Sync uses this to make entity enumeration, redaction, and initial
    /// outbox capture one atomic operation.
    pub async fn read_snapshot_on(
        &self,
        connection: &mut SqliteConnection,
        key: &DomainEntityKey,
    ) -> AppResult<DomainSnapshot> {
        read_snapshot_on(connection, key).await
    }
}

pub(crate) async fn read_snapshot_on(
    connection: &mut SqliteConnection,
    key: &DomainEntityKey,
) -> AppResult<DomainSnapshot> {
    match key.entity_type {
        DomainEntityType::Workspace => {
            if key.workspace_id != key.entity_id {
                return Err(AppError::Validation(
                    "workspace snapshot key must use the workspace id as entity id".to_string(),
                ));
            }
            let workspace = get_workspace_on(connection, &key.entity_id, true).await?;
            if let Some(deleted_at) = workspace.deleted_at {
                return Ok(tombstone(key, deleted_at, workspace.revision));
            }
            Ok(DomainSnapshot::Workspace(WorkspaceSnapshot {
                id: workspace.id,
                name: workspace.name,
                environment_type: workspace.environment_type,
                mcp_policy: workspace.mcp_policy,
                created_at: workspace.created_at,
                updated_at: workspace.updated_at,
                revision: workspace.revision,
            }))
        }
        DomainEntityType::WorkspaceVariable => {
            let variable = sqlx::query_as::<_, WorkspaceVariable>(
                r#"
                SELECT id, workspace_id, key, value, is_secret, is_enabled, description,
                       sort_order, created_at, updated_at, deleted_at, revision
                FROM workspace_variables
                WHERE id = ?1 AND workspace_id = ?2
                "#,
            )
            .bind(&key.entity_id)
            .bind(&key.workspace_id)
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| AppError::NotFound("workspace variable".to_string()))?;
            if let Some(deleted_at) = variable.deleted_at {
                return Ok(tombstone(key, deleted_at, variable.revision));
            }
            let is_secret = unfour_core::redaction::is_sensitive_workspace_variable(
                &variable.key,
                &variable.value,
                variable.is_secret,
            );
            let value = snapshot_value(is_secret, variable.value);
            Ok(DomainSnapshot::WorkspaceVariable(
                WorkspaceVariableSnapshot {
                    id: variable.id,
                    workspace_id: variable.workspace_id,
                    key: variable.key,
                    value,
                    is_secret,
                    is_enabled: variable.is_enabled,
                    description: variable.description,
                    sort_order: variable.sort_order,
                    created_at: variable.created_at,
                    updated_at: variable.updated_at,
                    revision: variable.revision,
                },
            ))
        }
        DomainEntityType::WorkspaceEnvironment => {
            let environment = sqlx::query_as::<_, WorkspaceEnvironmentRow>(
                r#"
                SELECT id, workspace_id, name, sort_order, created_at, updated_at,
                       deleted_at, revision
                FROM workspace_environments
                WHERE id = ?1 AND workspace_id = ?2
                "#,
            )
            .bind(&key.entity_id)
            .bind(&key.workspace_id)
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| AppError::NotFound("workspace environment".to_string()))?;
            if let Some(deleted_at) = environment.deleted_at {
                return Ok(tombstone(key, deleted_at, environment.revision));
            }
            Ok(DomainSnapshot::WorkspaceEnvironment(
                WorkspaceEnvironmentSnapshot {
                    id: environment.id,
                    workspace_id: environment.workspace_id,
                    name: environment.name,
                    sort_order: environment.sort_order,
                    created_at: environment.created_at,
                    updated_at: environment.updated_at,
                    revision: environment.revision,
                },
            ))
        }
        DomainEntityType::WorkspaceEnvironmentVariable => {
            let variable = sqlx::query_as::<_, WorkspaceEnvironmentVariable>(
                r#"
                SELECT id, workspace_id, environment_id, key, value, is_secret,
                       is_enabled, description, sort_order, created_at, updated_at,
                       deleted_at, revision
                FROM workspace_environment_variables
                WHERE id = ?1 AND workspace_id = ?2
                "#,
            )
            .bind(&key.entity_id)
            .bind(&key.workspace_id)
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| AppError::NotFound("workspace environment variable".to_string()))?;
            if let Some(deleted_at) = variable.deleted_at {
                let mut tombstone_key = key.clone();
                tombstone_key.parent_entity_id = Some(variable.environment_id);
                return Ok(tombstone(&tombstone_key, deleted_at, variable.revision));
            }
            let is_secret = unfour_core::redaction::is_sensitive_workspace_variable(
                &variable.key,
                &variable.value,
                variable.is_secret,
            );
            let value = snapshot_value(is_secret, variable.value);
            Ok(DomainSnapshot::WorkspaceEnvironmentVariable(
                WorkspaceEnvironmentVariableSnapshot {
                    id: variable.id,
                    workspace_id: variable.workspace_id,
                    environment_id: variable.environment_id,
                    key: variable.key,
                    value,
                    is_secret,
                    is_enabled: variable.is_enabled,
                    description: variable.description,
                    sort_order: variable.sort_order,
                    created_at: variable.created_at,
                    updated_at: variable.updated_at,
                    revision: variable.revision,
                },
            ))
        }
        DomainEntityType::Connection => Err(AppError::Validation(
            "connection snapshots must be read through a connection domain service".to_string(),
        )),
        DomainEntityType::ApiCollection
        | DomainEntityType::ApiFolder
        | DomainEntityType::ApiRequest => Err(AppError::Validation(
            "API snapshots must be read through the API domain service".to_string(),
        )),
        DomainEntityType::SshTask | DomainEntityType::SshTaskStep => Err(AppError::Validation(
            "SSH Task snapshots must be read through the SSH domain service".to_string(),
        )),
    }
}

fn snapshot_value(is_secret: bool, value: String) -> SnapshotVariableValue {
    if is_secret {
        SnapshotVariableValue::SecretRedacted
    } else {
        SnapshotVariableValue::Plain(value)
    }
}

fn tombstone(key: &DomainEntityKey, deleted_at: String, revision: i64) -> DomainSnapshot {
    DomainSnapshot::Tombstone(TombstoneSnapshot {
        entity: key.clone(),
        deleted_at,
        revision,
    })
}

impl WorkspaceService {
    pub async fn export_workspace_snapshots_on(
        &self,
        connection: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<DomainSnapshot>> {
        let mut records = Vec::new();
        records.push(
            self.read_snapshot_on(
                connection,
                &DomainEntityKey::new(DomainEntityType::Workspace, workspace, workspace),
            )
            .await?,
        );
        for id in sqlx::query_scalar::<_, String>("SELECT id FROM workspace_variables WHERE workspace_id = ? AND deleted_at IS NULL ORDER BY id").bind(workspace).fetch_all(&mut *connection).await? { records.push(self.read_snapshot_on(connection, &DomainEntityKey::new(DomainEntityType::WorkspaceVariable, workspace, id)).await?); }
        for id in sqlx::query_scalar::<_, String>("SELECT id FROM workspace_environments WHERE workspace_id = ? AND deleted_at IS NULL ORDER BY id").bind(workspace).fetch_all(&mut *connection).await? { records.push(self.read_snapshot_on(connection, &DomainEntityKey::new(DomainEntityType::WorkspaceEnvironment, workspace, id)).await?); }
        for id in sqlx::query_scalar::<_, String>("SELECT id FROM workspace_environment_variables WHERE workspace_id = ? AND deleted_at IS NULL ORDER BY id").bind(workspace).fetch_all(&mut *connection).await? { records.push(self.read_snapshot_on(connection, &DomainEntityKey::new(DomainEntityType::WorkspaceEnvironmentVariable, workspace, id)).await?); }
        Ok(records)
    }
}
impl WorkspaceService {
    /// Resolve portable import names on the same transaction as creation.
    pub async fn bundle_name_on(
        &self,
        connection: &mut SqliteConnection,
        name: &str,
    ) -> AppResult<String> {
        let name = super::normalize_name(name.to_string())?;
        let names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM workspaces WHERE deleted_at IS NULL")
                .fetch_all(connection)
                .await?;
        let used = |candidate: &str| {
            names
                .iter()
                .any(|n| n.to_lowercase() == candidate.to_lowercase())
        };
        if !used(&name) {
            return Ok(name);
        }
        // Leave room within the 80-character workspace name limit for the copy suffix.
        let base: String = name.chars().take(60).collect();
        for i in 1..=100_000 {
            let candidate = format!("{base} (Copy {i})");
            if !used(&candidate) {
                return Ok(candidate);
            }
        }
        Err(AppError::Validation(
            "WORKSPACE_BUNDLE_NAME_UNAVAILABLE".into(),
        ))
    }
}

impl WorkspaceService {
    pub async fn read_workspace_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
    ) -> AppResult<unfour_core::models::Workspace> {
        super::get_workspace_on(connection, workspace_id, false).await
    }
}
