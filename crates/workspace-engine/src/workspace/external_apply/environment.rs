use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainEntityType, DomainMutation, ExternalWorkspaceEnvironmentApply,
    ExternalWorkspaceEnvironmentUpsert, MutationOperation,
};
use unfour_core::AppResult;

use super::{delete_existing, doomed_orphan_to_skip, validate_delete};
use crate::workspace::get_workspace_on;
use crate::workspace::variable_executor::{
    entity_mutation, entity_mutation_with_parent, update_active_environment_after_delete_on,
};
use crate::workspace::variables::{
    active_environment_id_on, normalize_environment_name, WorkspaceEnvironmentRow,
};

pub(super) async fn apply_environment(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    change: ExternalWorkspaceEnvironmentApply,
    mutations: &mut Vec<DomainMutation>,
) -> AppResult<()> {
    match change {
        ExternalWorkspaceEnvironmentApply::Upsert(record) => {
            let workspace_id = record.workspace_id.clone();
            let id = record.id.clone();
            if let Some(revision) = upsert_environment(connection, record).await? {
                mutations.push(entity_mutation(
                    context,
                    DomainEntityType::WorkspaceEnvironment,
                    MutationOperation::Upsert,
                    &workspace_id,
                    &id,
                    revision,
                ));
            }
        }
        ExternalWorkspaceEnvironmentApply::Delete(delete) => {
            validate_delete(&delete, DomainEntityType::WorkspaceEnvironment)?;
            let was_active = active_environment_id_on(connection, &delete.entity.workspace_id)
                .await?
                .as_deref()
                == Some(delete.entity.entity_id.as_str());
            if let Some(revision) = delete_existing(
                connection,
                "workspace_environments",
                &delete.entity.workspace_id,
                &delete.entity.entity_id,
                &delete.deleted_at,
            )
            .await?
            {
                mutations.push(entity_mutation(
                    context,
                    DomainEntityType::WorkspaceEnvironment,
                    MutationOperation::Delete,
                    &delete.entity.workspace_id,
                    &delete.entity.entity_id,
                    revision,
                ));
                let children: Vec<(String, i64)> = sqlx::query_as(
                    r#"
                    UPDATE workspace_environment_variables
                    SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
                    WHERE workspace_id = ?2 AND environment_id = ?3 AND deleted_at IS NULL
                    RETURNING id, revision
                    "#,
                )
                .bind(&delete.deleted_at)
                .bind(&delete.entity.workspace_id)
                .bind(&delete.entity.entity_id)
                .fetch_all(&mut *connection)
                .await?;
                for (id, revision) in children {
                    mutations.push(entity_mutation_with_parent(
                        context,
                        DomainEntityType::WorkspaceEnvironmentVariable,
                        MutationOperation::Delete,
                        &delete.entity.workspace_id,
                        &id,
                        &delete.entity.entity_id,
                        revision,
                    ));
                }
                if was_active {
                    update_active_environment_after_delete_on(
                        connection,
                        &delete.entity.workspace_id,
                        &delete.entity.entity_id,
                        &delete.deleted_at,
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}

async fn upsert_environment(
    connection: &mut SqliteConnection,
    record: ExternalWorkspaceEnvironmentUpsert,
) -> AppResult<Option<i64>> {
    if doomed_orphan_to_skip(get_workspace_on(connection, &record.workspace_id, false).await)?
        .is_none()
    {
        return Ok(None);
    }
    let name = normalize_environment_name(record.name)?;
    let current = sqlx::query_as::<_, WorkspaceEnvironmentRow>(
        r#"
        SELECT id, workspace_id, name, sort_order, created_at, updated_at,
               deleted_at, revision
        FROM workspace_environments WHERE id = ?1 AND workspace_id = ?2
        "#,
    )
    .bind(&record.id)
    .bind(&record.workspace_id)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(current) = current {
        if current.deleted_at.is_none()
            && current.name == name
            && current.sort_order == record.sort_order
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok(None);
        }
        return Ok(Some(
            sqlx::query_scalar(
                r#"
                UPDATE workspace_environments
                SET name = ?1, sort_order = ?2, created_at = ?3, updated_at = ?4,
                    deleted_at = NULL, revision = revision + 1
                WHERE id = ?5 AND workspace_id = ?6 RETURNING revision
                "#,
            )
            .bind(name)
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
        INSERT INTO workspace_environments (
          id, workspace_id, name, sort_order, created_at, updated_at, revision
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
        "#,
    )
    .bind(record.id)
    .bind(record.workspace_id)
    .bind(name)
    .bind(record.sort_order)
    .bind(record.created_at)
    .bind(record.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok(Some(1))
}
