mod external_apply;
mod snapshot;
mod workspace_cascade;

use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainEntityKey, DomainEntityType, DomainMutation, MutationOperation,
};
use unfour_core::{AppError, AppResult};

use super::{validate_workspace_id, SshService};

pub(super) fn mutation(
    context: &CommandContext,
    entity_type: DomainEntityType,
    operation: MutationOperation,
    workspace_id: &str,
    entity_id: &str,
    parent_entity_id: Option<&str>,
    revision: i64,
) -> DomainMutation {
    let mut key = DomainEntityKey::new(entity_type, workspace_id, entity_id);
    key.parent_entity_id = parent_entity_id.map(str::to_string);
    DomainMutation::new(context.origin, operation, key, revision)
}

pub(super) async fn validate_workspace_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?1 AND deleted_at IS NULL)",
    )
    .bind(workspace_id)
    .fetch_one(&mut *connection)
    .await?;
    if !exists {
        return Err(AppError::NotFound("workspace".to_string()));
    }
    Ok(())
}

pub(super) async fn delete_task_steps_on(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    workspace_id: &str,
    task_id: &str,
    deleted_at: &str,
) -> AppResult<Vec<DomainMutation>> {
    delete_live_steps_on(connection, context, workspace_id, Some(task_id), deleted_at).await
}

pub(super) async fn delete_live_steps_on(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    workspace_id: &str,
    task_id: Option<&str>,
    deleted_at: &str,
) -> AppResult<Vec<DomainMutation>> {
    let steps: Vec<(String, String)> = match task_id {
        Some(task_id) => {
            sqlx::query_as(
                r#"
                SELECT id, task_id FROM ssh_task_step
                WHERE workspace_id = ?1 AND task_id = ?2 AND deleted_at IS NULL
                ORDER BY position, id
                "#,
            )
            .bind(workspace_id)
            .bind(task_id)
            .fetch_all(&mut *connection)
            .await?
        }
        None => {
            sqlx::query_as(
                r#"
                SELECT id, task_id FROM ssh_task_step
                WHERE workspace_id = ?1 AND deleted_at IS NULL
                ORDER BY task_id, position, id
                "#,
            )
            .bind(workspace_id)
            .fetch_all(&mut *connection)
            .await?
        }
    };
    let mut mutations = Vec::with_capacity(steps.len());
    for (step_id, parent_task_id) in steps {
        let Some(revision) = sqlx::query_scalar(
            r#"
            UPDATE ssh_task_step
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
            WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(deleted_at)
        .bind(workspace_id)
        .bind(&step_id)
        .fetch_optional(&mut *connection)
        .await?
        else {
            continue;
        };
        mutations.push(mutation(
            context,
            DomainEntityType::SshTaskStep,
            MutationOperation::Delete,
            workspace_id,
            &step_id,
            Some(&parent_task_id),
            revision,
        ));
    }
    Ok(mutations)
}

impl SshService {
    /// Enumerate live SSH Task domain entities in dependency order: every Task
    /// key precedes all Step keys, and each Step carries its stable Task parent.
    pub async fn list_task_domain_entities(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<DomainEntityKey>> {
        let mut connection = self.db.pool().acquire().await?;
        self.list_task_domain_entities_on(&mut connection, &workspace_id)
            .await
    }

    /// Enumerate live SSH Task domain entities using the caller's SQLite
    /// connection without acquiring a connection or starting a transaction.
    pub async fn list_task_domain_entities_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
    ) -> AppResult<Vec<DomainEntityKey>> {
        validate_workspace_id(workspace_id)?;
        validate_workspace_on(connection, workspace_id).await?;
        let task_ids = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id FROM ssh_task
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY sort_order, id
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        let steps = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT step.id, step.task_id
            FROM ssh_task_step step
            INNER JOIN ssh_task task
              ON task.id = step.task_id AND task.workspace_id = step.workspace_id
            WHERE step.workspace_id = ?1
              AND step.deleted_at IS NULL AND task.deleted_at IS NULL
            ORDER BY task.sort_order, task.id, step.position, step.id
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        let mut keys = Vec::with_capacity(task_ids.len() + steps.len());
        keys.extend(
            task_ids.into_iter().map(|task_id| {
                DomainEntityKey::new(DomainEntityType::SshTask, workspace_id, task_id)
            }),
        );
        keys.extend(steps.into_iter().map(|(step_id, task_id)| {
            DomainEntityKey::new(DomainEntityType::SshTaskStep, workspace_id, step_id)
                .with_parent_entity_id(task_id)
        }));
        Ok(keys)
    }
}

fn validate_domain_key(key: &DomainEntityKey) -> AppResult<()> {
    validate_workspace_id(&key.workspace_id)?;
    if key.entity_id.trim().is_empty() || key.entity_id.chars().count() > 128 {
        return Err(AppError::Validation(
            "invalid SSH task domain entity id".to_string(),
        ));
    }
    Ok(())
}
