use chrono::Utc;
use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainEntityKey, DomainEntityType, DomainMutation, MutationOperation,
};
use unfour_core::AppResult;

use super::{delete_live_steps_on, delete_task_steps_on, mutation, SshService};

impl SshService {
    /// Soft-delete every live SSH Task entity in a workspace, children first.
    /// Used when the workspace itself is tombstoned so leftover live tasks and
    /// steps cannot remain as orphans. SQL `ON DELETE CASCADE` does not fire
    /// on tombstones. Does not require the workspace row to still be live —
    /// the caller may already have tombstoned it.
    ///
    /// The second return value is the complete materialized set: entities this
    /// cascade actually tombstoned, plus already-deleted equivalents.
    pub async fn delete_workspace_ssh_task_entities_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: &str,
        deleted_at: Option<&str>,
    ) -> AppResult<(Vec<DomainMutation>, Vec<DomainEntityKey>)> {
        let deleted_at = deleted_at
            .map(str::to_string)
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let mut mutations = Vec::new();

        let task_ids: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT id FROM ssh_task
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY sort_order, id
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        for task_id in task_ids {
            mutations.extend(
                delete_task_steps_on(connection, context, workspace_id, &task_id, &deleted_at)
                    .await?,
            );
            sqlx::query(
                "DELETE FROM ssh_task_local_binding WHERE workspace_id = ?1 AND task_id = ?2",
            )
            .bind(workspace_id)
            .bind(&task_id)
            .execute(&mut *connection)
            .await?;
            sqlx::query("DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2")
                .bind(workspace_id)
                .bind(&task_id)
                .execute(&mut *connection)
                .await?;
            let Some(revision) =
                soft_delete_task_on(connection, workspace_id, &task_id, &deleted_at).await?
            else {
                continue;
            };
            mutations.push(mutation(
                context,
                DomainEntityType::SshTask,
                MutationOperation::Delete,
                workspace_id,
                &task_id,
                None,
                revision,
            ));
        }

        mutations.extend(
            delete_live_steps_on(connection, context, workspace_id, None, &deleted_at).await?,
        );

        let mut materialized: Vec<DomainEntityKey> = mutations
            .iter()
            .map(|mutation| mutation.entity.clone())
            .collect();
        materialized.extend(already_tombstoned_ssh_task_keys(connection, workspace_id).await?);
        Ok((mutations, materialized))
    }
}

async fn already_tombstoned_ssh_task_keys(
    connection: &mut SqliteConnection,
    workspace_id: &str,
) -> AppResult<Vec<DomainEntityKey>> {
    let mut keys = Vec::new();
    let steps: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT id, task_id FROM ssh_task_step
        WHERE workspace_id = ?1 AND deleted_at IS NOT NULL
        ORDER BY id
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&mut *connection)
    .await?;
    keys.extend(steps.into_iter().map(|(id, task_id)| {
        DomainEntityKey::new(DomainEntityType::SshTaskStep, workspace_id, id)
            .with_parent_entity_id(task_id)
    }));
    let tasks: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM ssh_task
        WHERE workspace_id = ?1 AND deleted_at IS NOT NULL
        ORDER BY id
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&mut *connection)
    .await?;
    keys.extend(
        tasks
            .into_iter()
            .map(|id| DomainEntityKey::new(DomainEntityType::SshTask, workspace_id, id)),
    );
    Ok(keys)
}

async fn soft_delete_task_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    task_id: &str,
    deleted_at: &str,
) -> AppResult<Option<i64>> {
    Ok(sqlx::query_scalar(
        r#"
        UPDATE ssh_task
        SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
        WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
        RETURNING revision
        "#,
    )
    .bind(deleted_at)
    .bind(workspace_id)
    .bind(task_id)
    .fetch_optional(&mut *connection)
    .await?)
}
