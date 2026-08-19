use chrono::Utc;
use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainEntityType, DomainMutation, ExternalWorkspaceApply,
    ExternalWorkspaceUpsert, MutationOperation,
};
use unfour_core::models::Workspace;
use unfour_core::{AppError, AppResult};

use super::{delete_existing, validate_delete};
use crate::workspace::delete_cascade::cascade_delete_workspace_children_on;
use crate::workspace::{
    get_workspace_on, insert_workspace_companions, normalize_environment_type,
    normalize_mcp_policy, normalize_name, read_setting_on, workspace_mutation, write_setting_on,
};

pub(super) async fn apply_workspace(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    change: ExternalWorkspaceApply,
    mutations: &mut Vec<DomainMutation>,
) -> AppResult<()> {
    match change {
        ExternalWorkspaceApply::Upsert(record) => {
            let id = record.id.clone();
            if let Some(revision) = upsert_workspace(connection, record).await? {
                mutations.push(workspace_mutation(
                    context,
                    MutationOperation::Upsert,
                    &id,
                    revision,
                ));
            }
        }
        ExternalWorkspaceApply::Delete(delete) => {
            validate_delete(&delete, DomainEntityType::Workspace)?;
            if delete.entity.workspace_id != delete.entity.entity_id {
                return Err(AppError::Validation(
                    "workspace delete key must use the workspace id as entity id".to_string(),
                ));
            }
            let current = get_workspace_on(connection, &delete.entity.workspace_id, true).await?;
            mutations.extend(
                cascade_delete_workspace_children_on(
                    connection,
                    context,
                    &delete.entity.workspace_id,
                    &delete.deleted_at,
                )
                .await?,
            );
            if current.deleted_at.is_some() {
                return Ok(());
            }
            let active_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM workspaces WHERE deleted_at IS NULL")
                    .fetch_one(&mut *connection)
                    .await?;
            let deleting_last = active_count <= 1;
            if let Some(revision) = delete_existing(
                connection,
                "workspaces",
                &delete.entity.workspace_id,
                &delete.entity.entity_id,
                &delete.deleted_at,
            )
            .await?
            {
                mutations.push(workspace_mutation(
                    context,
                    MutationOperation::Delete,
                    &delete.entity.workspace_id,
                    revision,
                ));
                if deleting_last {
                    let (fallback_id, fallback_revision) =
                        create_external_fallback_workspace(connection).await?;
                    mutations.push(workspace_mutation(
                        context,
                        MutationOperation::Upsert,
                        &fallback_id,
                        fallback_revision,
                    ));
                    return Ok(());
                }
                if read_setting_on(connection, "active_workspace_id")
                    .await?
                    .as_deref()
                    == Some(delete.entity.workspace_id.as_str())
                {
                    let next: String = sqlx::query_scalar(
                        r#"
                        SELECT id FROM workspaces
                        WHERE deleted_at IS NULL
                        ORDER BY is_default DESC, updated_at DESC
                        LIMIT 1
                        "#,
                    )
                    .fetch_one(&mut *connection)
                    .await?;
                    write_setting_on(connection, "active_workspace_id", &next).await?;
                }
            }
        }
    }
    Ok(())
}

async fn create_external_fallback_workspace(
    connection: &mut SqliteConnection,
) -> AppResult<(String, i64)> {
    let id = unfour_core::id::new_id();
    let now = Utc::now().to_rfc3339();
    let name = available_default_workspace_name(connection).await?;
    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, last_opened_at, environment_type, mcp_policy,
          created_at, updated_at, revision
        ) VALUES (?1, ?2, 1, ?3, 'dev', 'auto', ?3, ?3, 1)
        "#,
    )
    .bind(&id)
    .bind(name)
    .bind(&now)
    .execute(&mut *connection)
    .await?;
    insert_workspace_companions(connection, &id, &now).await?;
    write_setting_on(connection, "active_workspace_id", &id).await?;
    Ok((id, 1))
}

async fn available_default_workspace_name(connection: &mut SqliteConnection) -> AppResult<String> {
    let base = "Default Workspace";
    for suffix in 1_u32.. {
        let candidate = if suffix == 1 {
            base.to_string()
        } else {
            format!("{base} {suffix}")
        };
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE name = ?1 COLLATE NOCASE AND deleted_at IS NULL)",
        )
        .bind(&candidate)
        .fetch_one(&mut *connection)
        .await?;
        if !exists {
            return Ok(candidate);
        }
    }
    unreachable!("u32 workspace-name suffix space exhausted")
}

async fn upsert_workspace(
    connection: &mut SqliteConnection,
    record: ExternalWorkspaceUpsert,
) -> AppResult<Option<i64>> {
    let name = normalize_name(record.name)?;
    let environment_type = normalize_environment_type(Some(record.environment_type))?;
    let mcp_policy = normalize_mcp_policy(Some(record.mcp_policy))?;
    let current = sqlx::query_as::<_, Workspace>(
        r#"
        SELECT id, name, is_default, last_opened_at, environment_type, mcp_policy,
               created_at, updated_at, deleted_at, revision
        FROM workspaces WHERE id = ?1
        "#,
    )
    .bind(&record.id)
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(current) = current {
        if current.deleted_at.is_none()
            && current.name == name
            && current.environment_type == environment_type
            && current.mcp_policy == mcp_policy
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok(None);
        }
        let revision = sqlx::query_scalar(
            r#"
            UPDATE workspaces
            SET name = ?1, environment_type = ?2, mcp_policy = ?3,
                created_at = ?4, updated_at = ?5, deleted_at = NULL,
                revision = revision + 1
            WHERE id = ?6 RETURNING revision
            "#,
        )
        .bind(name)
        .bind(environment_type)
        .bind(mcp_policy)
        .bind(record.created_at)
        .bind(record.updated_at)
        .bind(record.id)
        .fetch_one(&mut *connection)
        .await?;
        return Ok(Some(revision));
    }

    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, last_opened_at, environment_type, mcp_policy,
          created_at, updated_at, revision
        ) VALUES (?1, ?2, 0, NULL, ?3, ?4, ?5, ?6, 1)
        "#,
    )
    .bind(&record.id)
    .bind(name)
    .bind(environment_type)
    .bind(mcp_policy)
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&mut *connection)
    .await?;
    insert_workspace_companions(connection, &record.id, &record.created_at).await?;
    Ok(Some(1))
}
