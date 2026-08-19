use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainEntityKey, DomainEntityType, DomainMutation,
    ExternalWorkspaceEnvironmentVariableApply, ExternalWorkspaceEnvironmentVariableUpsert,
    ExternalWorkspaceVariableApply, ExternalWorkspaceVariableUpsert, MutationOperation,
    SecretMaterialOutcome,
};
use unfour_core::models::{WorkspaceEnvironmentVariable, WorkspaceVariable};
use unfour_core::{AppError, AppResult};

use super::{
    delete_existing, doomed_orphan_to_skip, external_value, normalized_key, validate_delete,
};
use crate::workspace::get_workspace_on;
use crate::workspace::variable_executor::{entity_mutation, entity_mutation_with_parent};
use crate::workspace::variables::{get_environment_on, normalize_description};

pub(super) async fn apply_workspace_variable(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    change: ExternalWorkspaceVariableApply,
    mutations: &mut Vec<DomainMutation>,
    secret_material_outcomes: &mut Vec<SecretMaterialOutcome>,
) -> AppResult<()> {
    match change {
        ExternalWorkspaceVariableApply::Upsert(record) => {
            let workspace_id = record.workspace_id.clone();
            let id = record.id.clone();
            let (revision, secret_status) = upsert_workspace_variable(connection, record).await?;
            if let Some(status) = secret_status {
                secret_material_outcomes.push(SecretMaterialOutcome {
                    entity: DomainEntityKey::new(
                        DomainEntityType::WorkspaceVariable,
                        &workspace_id,
                        &id,
                    ),
                    status,
                });
            }
            if let Some(revision) = revision {
                mutations.push(entity_mutation(
                    context,
                    DomainEntityType::WorkspaceVariable,
                    MutationOperation::Upsert,
                    &workspace_id,
                    &id,
                    revision,
                ));
            }
        }
        ExternalWorkspaceVariableApply::Delete(delete) => {
            validate_delete(&delete, DomainEntityType::WorkspaceVariable)?;
            if let Some(revision) = delete_existing(
                connection,
                "workspace_variables",
                &delete.entity.workspace_id,
                &delete.entity.entity_id,
                &delete.deleted_at,
            )
            .await?
            {
                mutations.push(entity_mutation(
                    context,
                    DomainEntityType::WorkspaceVariable,
                    MutationOperation::Delete,
                    &delete.entity.workspace_id,
                    &delete.entity.entity_id,
                    revision,
                ));
            }
        }
    }
    Ok(())
}

async fn upsert_workspace_variable(
    connection: &mut SqliteConnection,
    record: ExternalWorkspaceVariableUpsert,
) -> AppResult<(
    Option<i64>,
    Option<unfour_core::domain::SecretMaterialStatus>,
)> {
    if doomed_orphan_to_skip(get_workspace_on(connection, &record.workspace_id, false).await)?
        .is_none()
    {
        return Ok((None, None));
    }
    let key = normalized_key(&record.key)?;
    let current = sqlx::query_as::<_, WorkspaceVariable>(
        r#"
        SELECT id, workspace_id, key, value, is_secret, is_enabled, description,
               sort_order, created_at, updated_at, deleted_at, revision
        FROM workspace_variables WHERE id = ?1 AND workspace_id = ?2
        "#,
    )
    .bind(&record.id)
    .bind(&record.workspace_id)
    .fetch_optional(&mut *connection)
    .await?;
    let (value, secret_status) = external_value(
        record.is_secret,
        &record.value,
        current.as_ref().map(|v| &v.value),
    )?;
    let description = normalize_description(record.description);
    if let Some(current) = current {
        if current.deleted_at.is_none()
            && current.key == key
            && current.value == value
            && current.is_secret == record.is_secret
            && current.is_enabled == record.is_enabled
            && current.description == description
            && current.sort_order == record.sort_order
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok((None, secret_status));
        }
        return Ok((
            Some(
                sqlx::query_scalar(
                    r#"
                UPDATE workspace_variables
                SET key = ?1, value = ?2, is_secret = ?3, is_enabled = ?4,
                    description = ?5, sort_order = ?6, created_at = ?7,
                    updated_at = ?8, deleted_at = NULL, revision = revision + 1
                WHERE id = ?9 AND workspace_id = ?10 RETURNING revision
                "#,
                )
                .bind(key)
                .bind(value)
                .bind(record.is_secret)
                .bind(record.is_enabled)
                .bind(description)
                .bind(record.sort_order)
                .bind(record.created_at)
                .bind(record.updated_at)
                .bind(record.id)
                .bind(record.workspace_id)
                .fetch_one(&mut *connection)
                .await?,
            ),
            secret_status,
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO workspace_variables (
          id, workspace_id, key, value, is_secret, is_enabled, description,
          sort_order, created_at, updated_at, revision
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1)
        "#,
    )
    .bind(record.id)
    .bind(record.workspace_id)
    .bind(key)
    .bind(value)
    .bind(record.is_secret)
    .bind(record.is_enabled)
    .bind(description)
    .bind(record.sort_order)
    .bind(record.created_at)
    .bind(record.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok((Some(1), secret_status))
}

pub(super) async fn apply_environment_variable(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    change: ExternalWorkspaceEnvironmentVariableApply,
    mutations: &mut Vec<DomainMutation>,
    secret_material_outcomes: &mut Vec<SecretMaterialOutcome>,
) -> AppResult<()> {
    match change {
        ExternalWorkspaceEnvironmentVariableApply::Upsert(record) => {
            let workspace_id = record.workspace_id.clone();
            let environment_id = record.environment_id.clone();
            let id = record.id.clone();
            let (revision, secret_status) = upsert_environment_variable(connection, record).await?;
            if let Some(status) = secret_status {
                secret_material_outcomes.push(SecretMaterialOutcome {
                    entity: DomainEntityKey::new(
                        DomainEntityType::WorkspaceEnvironmentVariable,
                        &workspace_id,
                        &id,
                    )
                    .with_parent_entity_id(&environment_id),
                    status,
                });
            }
            if let Some(revision) = revision {
                mutations.push(entity_mutation_with_parent(
                    context,
                    DomainEntityType::WorkspaceEnvironmentVariable,
                    MutationOperation::Upsert,
                    &workspace_id,
                    &id,
                    &environment_id,
                    revision,
                ));
            }
        }
        ExternalWorkspaceEnvironmentVariableApply::Delete(delete) => {
            validate_delete(&delete, DomainEntityType::WorkspaceEnvironmentVariable)?;
            let parent_entity_id: Option<String> = sqlx::query_scalar(
                "SELECT environment_id FROM workspace_environment_variables WHERE id = ?1 AND workspace_id = ?2",
            )
            .bind(&delete.entity.entity_id)
            .bind(&delete.entity.workspace_id)
            .fetch_optional(&mut *connection)
            .await?;
            if let Some(revision) = delete_existing(
                connection,
                "workspace_environment_variables",
                &delete.entity.workspace_id,
                &delete.entity.entity_id,
                &delete.deleted_at,
            )
            .await?
            {
                let parent_entity_id = parent_entity_id.ok_or_else(|| {
                    AppError::Config(
                        "environment variable delete lost its parent environment".to_string(),
                    )
                })?;
                if delete
                    .entity
                    .parent_entity_id
                    .as_deref()
                    .is_some_and(|provided| provided != parent_entity_id.as_str())
                {
                    return Err(AppError::Validation(
                        "environment variable delete parent entity does not match local metadata"
                            .to_string(),
                    ));
                }
                mutations.push(entity_mutation_with_parent(
                    context,
                    DomainEntityType::WorkspaceEnvironmentVariable,
                    MutationOperation::Delete,
                    &delete.entity.workspace_id,
                    &delete.entity.entity_id,
                    &parent_entity_id,
                    revision,
                ));
            }
        }
    }
    Ok(())
}

async fn upsert_environment_variable(
    connection: &mut SqliteConnection,
    record: ExternalWorkspaceEnvironmentVariableUpsert,
) -> AppResult<(
    Option<i64>,
    Option<unfour_core::domain::SecretMaterialStatus>,
)> {
    if doomed_orphan_to_skip(
        get_environment_on(
            connection,
            &record.workspace_id,
            &record.environment_id,
            false,
        )
        .await,
    )?
    .is_none()
    {
        return Ok((None, None));
    }
    let key = normalized_key(&record.key)?;
    let current = sqlx::query_as::<_, WorkspaceEnvironmentVariable>(
        r#"
        SELECT id, workspace_id, environment_id, key, value, is_secret,
               is_enabled, description, sort_order, created_at, updated_at,
               deleted_at, revision
        FROM workspace_environment_variables WHERE id = ?1 AND workspace_id = ?2
        "#,
    )
    .bind(&record.id)
    .bind(&record.workspace_id)
    .fetch_optional(&mut *connection)
    .await?;
    let (value, secret_status) = external_value(
        record.is_secret,
        &record.value,
        current.as_ref().map(|v| &v.value),
    )?;
    let description = normalize_description(record.description);
    if let Some(current) = current {
        if current.deleted_at.is_none()
            && current.environment_id == record.environment_id
            && current.key == key
            && current.value == value
            && current.is_secret == record.is_secret
            && current.is_enabled == record.is_enabled
            && current.description == description
            && current.sort_order == record.sort_order
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok((None, secret_status));
        }
        return Ok((
            Some(
                sqlx::query_scalar(
                    r#"
                UPDATE workspace_environment_variables
                SET environment_id = ?1, key = ?2, value = ?3, is_secret = ?4,
                    is_enabled = ?5, description = ?6, sort_order = ?7,
                    created_at = ?8, updated_at = ?9, deleted_at = NULL,
                    revision = revision + 1
                WHERE id = ?10 AND workspace_id = ?11 RETURNING revision
                "#,
                )
                .bind(record.environment_id)
                .bind(key)
                .bind(value)
                .bind(record.is_secret)
                .bind(record.is_enabled)
                .bind(description)
                .bind(record.sort_order)
                .bind(record.created_at)
                .bind(record.updated_at)
                .bind(record.id)
                .bind(record.workspace_id)
                .fetch_one(&mut *connection)
                .await?,
            ),
            secret_status,
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO workspace_environment_variables (
          id, workspace_id, environment_id, key, value, is_secret, is_enabled,
          description, sort_order, created_at, updated_at, revision
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1)
        "#,
    )
    .bind(record.id)
    .bind(record.workspace_id)
    .bind(record.environment_id)
    .bind(key)
    .bind(value)
    .bind(record.is_secret)
    .bind(record.is_enabled)
    .bind(description)
    .bind(record.sort_order)
    .bind(record.created_at)
    .bind(record.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok((Some(1), secret_status))
}
