use sqlx::{FromRow, SqliteConnection};
use unfour_core::domain::{
    DomainEntityKey, DomainEntityType, DomainSnapshot, SshTaskSnapshot, SshTaskStepSnapshot,
    TombstoneSnapshot,
};
use unfour_core::{AppError, AppResult};

use super::{validate_domain_key, SshService};
use crate::ssh::task::template::canonical_step_config;

#[derive(FromRow)]
struct TaskSnapshotRow {
    id: String,
    workspace_id: String,
    name: String,
    description: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    revision: i64,
}

#[derive(FromRow)]
struct StepSnapshotRow {
    id: String,
    workspace_id: String,
    task_id: String,
    name: String,
    step_type: String,
    position: i64,
    enabled: i64,
    config_version: i64,
    config_json: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    revision: i64,
}

impl SshService {
    pub async fn read_task_domain_snapshot(
        &self,
        key: &DomainEntityKey,
    ) -> AppResult<DomainSnapshot> {
        let mut connection = self.db.pool().acquire().await?;
        self.read_task_domain_snapshot_on(&mut connection, key)
            .await
    }

    pub async fn read_task_domain_snapshot_on(
        &self,
        connection: &mut SqliteConnection,
        key: &DomainEntityKey,
    ) -> AppResult<DomainSnapshot> {
        validate_domain_key(key)?;
        match key.entity_type {
            DomainEntityType::SshTask => {
                let row = task_row(connection, key).await?;
                if let Some(deleted_at) = row.deleted_at {
                    return Ok(tombstone(key.clone(), deleted_at, row.revision));
                }
                Ok(DomainSnapshot::SshTask(SshTaskSnapshot {
                    id: row.id,
                    workspace_id: row.workspace_id,
                    name: row.name,
                    description: row.description,
                    sort_order: row.sort_order,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    revision: row.revision,
                }))
            }
            DomainEntityType::SshTaskStep => {
                let row = step_row(connection, key).await?;
                let mut snapshot_key = key.clone();
                snapshot_key.parent_entity_id = Some(row.task_id.clone());
                if let Some(deleted_at) = row.deleted_at {
                    return Ok(tombstone(snapshot_key, deleted_at, row.revision));
                }
                let config_json: serde_json::Value = serde_json::from_str(&row.config_json)
                    .map_err(|error| {
                        AppError::Config(format!("stored SSH task step config is invalid: {error}"))
                    })?;
                let config_json = canonical_step_config(
                    &row.id,
                    &row.step_type,
                    row.config_version,
                    &config_json,
                )?;
                Ok(DomainSnapshot::SshTaskStep(SshTaskStepSnapshot {
                    id: row.id,
                    workspace_id: row.workspace_id,
                    task_id: row.task_id,
                    name: row.name,
                    step_type: row.step_type,
                    position: row.position,
                    enabled: row.enabled != 0,
                    config_version: row.config_version,
                    config_json,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    revision: row.revision,
                }))
            }
            _ => Err(AppError::Validation(
                "SSH Task snapshot requires an SSH Task domain entity type".to_string(),
            )),
        }
    }
}

async fn task_row(
    connection: &mut SqliteConnection,
    key: &DomainEntityKey,
) -> AppResult<TaskSnapshotRow> {
    sqlx::query_as(
        r#"
        SELECT id, workspace_id, name, description, sort_order, created_at,
               updated_at, deleted_at, revision
        FROM ssh_task WHERE workspace_id = ?1 AND id = ?2
        "#,
    )
    .bind(&key.workspace_id)
    .bind(&key.entity_id)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::NotFound("SSH task".to_string()))
}

async fn step_row(
    connection: &mut SqliteConnection,
    key: &DomainEntityKey,
) -> AppResult<StepSnapshotRow> {
    sqlx::query_as(
        r#"
        SELECT id, workspace_id, task_id, name, step_type, position, enabled,
               config_version, config_json, created_at, updated_at, deleted_at,
               revision
        FROM ssh_task_step WHERE workspace_id = ?1 AND id = ?2
        "#,
    )
    .bind(&key.workspace_id)
    .bind(&key.entity_id)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::NotFound("SSH task step".to_string()))
}

fn tombstone(key: DomainEntityKey, deleted_at: String, revision: i64) -> DomainSnapshot {
    DomainSnapshot::Tombstone(TombstoneSnapshot {
        entity: key,
        deleted_at,
        revision,
    })
}
