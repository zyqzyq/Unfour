use sqlx::SqliteConnection;
use unfour_core::domain::{CommandContext, DomainEntityType, DomainMutation, MutationOperation};
use unfour_core::AppResult;

use super::variable_executor::{entity_mutation, entity_mutation_with_parent};
use super::variable_persistence::soft_delete;

/// Soft-delete every live workspace-owned descendant, children first, using one
/// `deleted_at`. SQL `ON DELETE CASCADE` does not fire on tombstones.
pub(crate) async fn cascade_delete_workspace_children_on(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    workspace_id: &str,
    deleted_at: &str,
) -> AppResult<Vec<DomainMutation>> {
    let mut mutations = Vec::new();
    let environment_variables: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT id, environment_id
        FROM workspace_environment_variables
        WHERE workspace_id = ?1 AND deleted_at IS NULL
        ORDER BY environment_id, id
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&mut *connection)
    .await?;
    for (id, environment_id) in environment_variables {
        let revision = soft_delete(
            connection,
            "workspace_environment_variables",
            workspace_id,
            Some(&environment_id),
            &id,
            deleted_at,
        )
        .await?;
        mutations.push(entity_mutation_with_parent(
            context,
            DomainEntityType::WorkspaceEnvironmentVariable,
            MutationOperation::Delete,
            workspace_id,
            &id,
            &environment_id,
            revision,
        ));
    }

    let environments: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM workspace_environments
        WHERE workspace_id = ?1 AND deleted_at IS NULL
        ORDER BY id
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&mut *connection)
    .await?;
    for id in environments {
        let revision = soft_delete(
            connection,
            "workspace_environments",
            workspace_id,
            None,
            &id,
            deleted_at,
        )
        .await?;
        mutations.push(entity_mutation(
            context,
            DomainEntityType::WorkspaceEnvironment,
            MutationOperation::Delete,
            workspace_id,
            &id,
            revision,
        ));
    }

    let variables: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM workspace_variables
        WHERE workspace_id = ?1 AND deleted_at IS NULL
        ORDER BY id
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&mut *connection)
    .await?;
    for id in variables {
        let revision = soft_delete(
            connection,
            "workspace_variables",
            workspace_id,
            None,
            &id,
            deleted_at,
        )
        .await?;
        mutations.push(entity_mutation(
            context,
            DomainEntityType::WorkspaceVariable,
            MutationOperation::Delete,
            workspace_id,
            &id,
            revision,
        ));
    }

    sqlx::query(
        r#"
        UPDATE workspace_local_state
        SET active_environment_id = NULL, updated_at = ?1
        WHERE workspace_id = ?2
        "#,
    )
    .bind(deleted_at)
    .bind(workspace_id)
    .execute(&mut *connection)
    .await?;

    Ok(mutations)
}
