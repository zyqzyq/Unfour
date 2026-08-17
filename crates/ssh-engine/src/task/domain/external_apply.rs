use sqlx::{FromRow, SqliteConnection};
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityType, DomainMutation, ExternalApplyPage,
    ExternalApplyReport, ExternalDelete, ExternalSshTaskApply, ExternalSshTaskStepApply,
    ExternalSshTaskStepUpsert, ExternalSshTaskUpsert, MutationOperation, MutationOrigin,
};
use unfour_core::{AppError, AppResult};

use super::{delete_task_steps_on, mutation, validate_workspace_on, SshService};
use crate::ssh::task::template::{
    canonical_step_config, normalized_step_config, restore_device_local_step_config,
    validate_step_config,
};

#[derive(FromRow)]
struct CurrentTask {
    workspace_id: String,
    name: String,
    description: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(FromRow)]
struct CurrentStep {
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
}

impl SshService {
    pub async fn apply_external_task_page_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        page: ExternalApplyPage,
    ) -> AppResult<DomainCommandResult<ExternalApplyReport>> {
        if context.origin != MutationOrigin::External {
            return Err(AppError::Config(
                "external apply requires an External command context".to_string(),
            ));
        }
        let (task_upserts, task_deletes) = split_tasks(page.ssh_tasks);
        let (step_upserts, step_deletes) = split_steps(page.ssh_task_steps);
        let mut mutations = Vec::new();

        for record in task_upserts {
            let workspace_id = record.workspace_id.clone();
            let id = record.id.clone();
            if let Some(revision) = upsert_task(connection, record).await? {
                mutations.push(mutation(
                    context,
                    DomainEntityType::SshTask,
                    MutationOperation::Upsert,
                    &workspace_id,
                    &id,
                    None,
                    revision,
                ));
            }
        }

        for record in step_upserts {
            let workspace_id = record.workspace_id.clone();
            let task_id = record.task_id.clone();
            let id = record.id.clone();
            if let Some(revision) = upsert_step(connection, record).await? {
                mutations.push(mutation(
                    context,
                    DomainEntityType::SshTaskStep,
                    MutationOperation::Upsert,
                    &workspace_id,
                    &id,
                    Some(&task_id),
                    revision,
                ));
            }
        }

        for delete in step_deletes {
            apply_step_delete(connection, context, delete, &mut mutations).await?;
        }
        for delete in task_deletes {
            apply_task_delete(connection, context, delete, &mut mutations).await?;
        }

        let report = ExternalApplyReport {
            applied_count: mutations.len(),
            mutations: mutations.clone(),
            secret_material_outcomes: Vec::new(),
        };
        Ok(DomainCommandResult::new(report, mutations))
    }
}

fn split_tasks(
    changes: Vec<ExternalSshTaskApply>,
) -> (Vec<ExternalSshTaskUpsert>, Vec<ExternalDelete>) {
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();
    for change in changes {
        match change {
            ExternalSshTaskApply::Upsert(record) => upserts.push(record),
            ExternalSshTaskApply::Delete(delete) => deletes.push(delete),
        }
    }
    (upserts, deletes)
}

fn split_steps(
    changes: Vec<ExternalSshTaskStepApply>,
) -> (Vec<ExternalSshTaskStepUpsert>, Vec<ExternalDelete>) {
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();
    for change in changes {
        match change {
            ExternalSshTaskStepApply::Upsert(record) => upserts.push(record),
            ExternalSshTaskStepApply::Delete(delete) => deletes.push(delete),
        }
    }
    (upserts, deletes)
}

async fn upsert_task(
    connection: &mut SqliteConnection,
    record: ExternalSshTaskUpsert,
) -> AppResult<Option<i64>> {
    validate_external_record(
        &record.id,
        &record.workspace_id,
        &record.created_at,
        &record.updated_at,
    )?;
    if record.sort_order < 0 {
        return Err(AppError::Validation(
            "external SSH task sort order cannot be negative".to_string(),
        ));
    }
    validate_workspace_on(connection, &record.workspace_id).await?;
    let name = record.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation(
            "external SSH task name cannot be empty".to_string(),
        ));
    }
    let current = sqlx::query_as::<_, CurrentTask>(
        r#"
        SELECT workspace_id, name, description, sort_order, created_at,
               updated_at, deleted_at
        FROM ssh_task WHERE id = ?1
        "#,
    )
    .bind(&record.id)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(current) = current {
        if current.workspace_id != record.workspace_id {
            return Err(AppError::Validation(
                "external SSH task workspace ownership mismatch".to_string(),
            ));
        }
        if current.deleted_at.is_none()
            && current.name == name
            && current.description == record.description
            && current.sort_order == record.sort_order
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok(None);
        }
        return Ok(Some(
            sqlx::query_scalar(
                r#"
                UPDATE ssh_task
                SET name = ?1, description = ?2, sort_order = ?3,
                    created_at = ?4, updated_at = ?5, deleted_at = NULL,
                    revision = revision + 1
                WHERE id = ?6 AND workspace_id = ?7
                RETURNING revision
                "#,
            )
            .bind(name)
            .bind(record.description)
            .bind(record.sort_order)
            .bind(record.created_at)
            .bind(record.updated_at)
            .bind(record.id)
            .bind(record.workspace_id)
            .fetch_one(&mut *connection)
            .await?,
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO ssh_task (
          id, workspace_id, name, description, sort_order, created_at,
          updated_at, deleted_at, revision
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 1)
        "#,
    )
    .bind(record.id)
    .bind(record.workspace_id)
    .bind(name)
    .bind(record.description)
    .bind(record.sort_order)
    .bind(record.created_at)
    .bind(record.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok(Some(1))
}

async fn upsert_step(
    connection: &mut SqliteConnection,
    record: ExternalSshTaskStepUpsert,
) -> AppResult<Option<i64>> {
    validate_external_record(
        &record.id,
        &record.workspace_id,
        &record.created_at,
        &record.updated_at,
    )?;
    if record.task_id.trim().is_empty() || record.position < 0 {
        return Err(AppError::Validation(
            "external SSH task step requires a parent and non-negative position".to_string(),
        ));
    }
    validate_step_config(
        &record.step_type,
        record.config_version,
        &record.config_json,
    )?;
    let canonical_config = canonical_step_config(
        &record.id,
        &record.step_type,
        record.config_version,
        &record.config_json,
    )?;
    if canonical_config != record.config_json {
        return Err(AppError::Validation(
            "external SSH task step config must not contain device-local paths".to_string(),
        ));
    }
    let name = record.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation(
            "external SSH task step name cannot be empty".to_string(),
        ));
    }
    let parent: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT workspace_id, deleted_at FROM ssh_task WHERE id = ?1")
            .bind(&record.task_id)
            .fetch_optional(&mut *connection)
            .await?;
    let Some((parent_workspace_id, parent_deleted_at)) = parent else {
        return Ok(None);
    };
    if parent_workspace_id != record.workspace_id {
        return Err(AppError::Validation(
            "external SSH task step parent workspace mismatch".to_string(),
        ));
    }
    if parent_deleted_at.is_some() {
        return Ok(None);
    }

    let current = sqlx::query_as::<_, CurrentStep>(
        r#"
        SELECT workspace_id, task_id, name, step_type, position, enabled,
               config_version, config_json, created_at, updated_at, deleted_at
        FROM ssh_task_step WHERE id = ?1
        "#,
    )
    .bind(&record.id)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(current) = current {
        if current.workspace_id != record.workspace_id {
            return Err(AppError::Validation(
                "external SSH task step workspace ownership mismatch".to_string(),
            ));
        }
        let was_active = current.deleted_at.is_none();
        if was_active && current.task_id != record.task_id {
            return Err(AppError::Validation(
                "external SSH task step cannot change its task parent".to_string(),
            ));
        }
        let stored_config: serde_json::Value =
            serde_json::from_str(&current.config_json).map_err(|error| {
                AppError::Config(format!("stored SSH task step config is invalid: {error}"))
            })?;
        let current_config =
            normalized_step_config(&current.step_type, current.config_version, &stored_config)?;
        let restored_config = restore_device_local_step_config(
            &record.id,
            &record.step_type,
            record.config_version,
            &canonical_config,
            &current.step_type,
            current.config_version,
            &stored_config,
        )?;
        if was_active
            && current.task_id == record.task_id
            && current.name == name
            && current.step_type == record.step_type
            && current.position == record.position
            && (current.enabled != 0) == record.enabled
            && current.config_version == record.config_version
            && current_config == restored_config
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok(None);
        }
        let config_json = serde_json::to_string(&restored_config)?;
        return Ok(Some(
            sqlx::query_scalar(
                r#"
                UPDATE ssh_task_step
                SET task_id = ?1, name = ?2, step_type = ?3, position = ?4,
                    enabled = ?5, config_version = ?6, config_json = ?7,
                    created_at = ?8, updated_at = ?9, deleted_at = NULL,
                    revision = revision + 1
                WHERE id = ?10 AND workspace_id = ?11
                RETURNING revision
                "#,
            )
            .bind(record.task_id)
            .bind(name)
            .bind(record.step_type)
            .bind(record.position)
            .bind(if record.enabled { 1_i64 } else { 0_i64 })
            .bind(record.config_version)
            .bind(config_json)
            .bind(record.created_at)
            .bind(record.updated_at)
            .bind(record.id)
            .bind(record.workspace_id)
            .fetch_one(&mut *connection)
            .await?,
        ));
    }
    let config_json = serde_json::to_string(&canonical_config)?;
    sqlx::query(
        r#"
        INSERT INTO ssh_task_step (
          id, workspace_id, task_id, name, step_type, position, enabled,
          config_version, config_json, created_at, updated_at, deleted_at,
          revision
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, 1)
        "#,
    )
    .bind(record.id)
    .bind(record.workspace_id)
    .bind(record.task_id)
    .bind(name)
    .bind(record.step_type)
    .bind(record.position)
    .bind(if record.enabled { 1_i64 } else { 0_i64 })
    .bind(record.config_version)
    .bind(config_json)
    .bind(record.created_at)
    .bind(record.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok(Some(1))
}

async fn apply_step_delete(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    delete: ExternalDelete,
    mutations: &mut Vec<DomainMutation>,
) -> AppResult<()> {
    validate_delete(&delete, DomainEntityType::SshTaskStep)?;
    let current: Option<(String, String, Option<String>)> =
        sqlx::query_as("SELECT workspace_id, task_id, deleted_at FROM ssh_task_step WHERE id = ?1")
            .bind(&delete.entity.entity_id)
            .fetch_optional(&mut *connection)
            .await?;
    let Some((workspace_id, task_id, deleted_at)) = current else {
        return Ok(());
    };
    if workspace_id != delete.entity.workspace_id {
        return Err(AppError::Validation(
            "external SSH task step workspace ownership mismatch".to_string(),
        ));
    }
    if deleted_at.is_some() {
        return Ok(());
    }
    if delete
        .entity
        .parent_entity_id
        .as_deref()
        .is_some_and(|provided| provided != task_id)
    {
        return Err(AppError::Validation(
            "external SSH task step delete parent mismatch".to_string(),
        ));
    }
    let revision: i64 = sqlx::query_scalar(
        r#"
        UPDATE ssh_task_step
        SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
        WHERE id = ?2 AND workspace_id = ?3 AND deleted_at IS NULL
        RETURNING revision
        "#,
    )
    .bind(&delete.deleted_at)
    .bind(&delete.entity.entity_id)
    .bind(&delete.entity.workspace_id)
    .fetch_one(&mut *connection)
    .await?;
    mutations.push(mutation(
        context,
        DomainEntityType::SshTaskStep,
        MutationOperation::Delete,
        &delete.entity.workspace_id,
        &delete.entity.entity_id,
        Some(&task_id),
        revision,
    ));
    Ok(())
}

async fn apply_task_delete(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    delete: ExternalDelete,
    mutations: &mut Vec<DomainMutation>,
) -> AppResult<()> {
    validate_delete(&delete, DomainEntityType::SshTask)?;
    let current: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT workspace_id, deleted_at FROM ssh_task WHERE id = ?1")
            .bind(&delete.entity.entity_id)
            .fetch_optional(&mut *connection)
            .await?;
    let Some((workspace_id, deleted_at)) = current else {
        return Ok(());
    };
    if workspace_id != delete.entity.workspace_id {
        return Err(AppError::Validation(
            "external SSH task workspace ownership mismatch".to_string(),
        ));
    }
    if deleted_at.is_some() {
        return Ok(());
    }
    mutations.extend(
        delete_task_steps_on(
            connection,
            context,
            &delete.entity.workspace_id,
            &delete.entity.entity_id,
            &delete.deleted_at,
        )
        .await?,
    );
    let revision: i64 = sqlx::query_scalar(
        r#"
        UPDATE ssh_task
        SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
        WHERE id = ?2 AND workspace_id = ?3 AND deleted_at IS NULL
        RETURNING revision
        "#,
    )
    .bind(&delete.deleted_at)
    .bind(&delete.entity.entity_id)
    .bind(&delete.entity.workspace_id)
    .fetch_one(&mut *connection)
    .await?;
    mutations.push(mutation(
        context,
        DomainEntityType::SshTask,
        MutationOperation::Delete,
        &delete.entity.workspace_id,
        &delete.entity.entity_id,
        None,
        revision,
    ));
    Ok(())
}

fn validate_external_record(
    id: &str,
    workspace_id: &str,
    created_at: &str,
    updated_at: &str,
) -> AppResult<()> {
    if [id, workspace_id, created_at, updated_at]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(AppError::Validation(
            "external SSH task upsert requires ids and timestamps".to_string(),
        ));
    }
    Ok(())
}

fn validate_delete(delete: &ExternalDelete, expected: DomainEntityType) -> AppResult<()> {
    if delete.entity.entity_type != expected {
        return Err(AppError::Validation(
            "external delete entity type does not match its apply collection".to_string(),
        ));
    }
    if delete.entity.workspace_id.trim().is_empty()
        || delete.entity.entity_id.trim().is_empty()
        || delete.deleted_at.trim().is_empty()
    {
        return Err(AppError::Validation(
            "external delete requires ids and deleted_at".to_string(),
        ));
    }
    Ok(())
}
