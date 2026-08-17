use super::super::domain::{delete_task_steps_on, mutation};
use super::super::*;
use super::convert::*;
use std::collections::HashSet;
use std::path::PathBuf;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityType, MutationOperation,
};

impl SshService {
    pub async fn list_tasks(&self, workspace_id: String) -> AppResult<Vec<SshTask>> {
        validate_workspace_id(&workspace_id)?;
        let mut connection = self.db.pool().acquire().await?;
        self.list_tasks_on(&mut connection, &workspace_id).await
    }

    pub async fn list_tasks_on(
        &self,
        connection: &mut sqlx::SqliteConnection,
        workspace_id: &str,
    ) -> AppResult<Vec<SshTask>> {
        let rows = sqlx::query_as::<_, StoredTask>(
            r#"
            SELECT id, workspace_id, name, description, sort_order,
                   created_at, updated_at, deleted_at
            FROM ssh_task
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY sort_order, name COLLATE NOCASE, id
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        Ok(rows.into_iter().map(task_from_row).collect())
    }

    pub async fn get_task(&self, workspace_id: &str, task_id: &str) -> AppResult<SshTaskDetail> {
        validate_workspace_id(workspace_id)?;
        validate_task_id(task_id)?;
        let mut connection = self.db.pool().acquire().await?;
        self.get_task_on(&mut connection, workspace_id, task_id)
            .await
    }

    pub async fn get_task_on(
        &self,
        connection: &mut sqlx::SqliteConnection,
        workspace_id: &str,
        task_id: &str,
    ) -> AppResult<SshTaskDetail> {
        let task = sqlx::query_as::<_, StoredTask>(
            r#"
            SELECT id, workspace_id, name, description, sort_order,
                   created_at, updated_at, deleted_at
            FROM ssh_task
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_optional(&mut *connection)
        .await?
        .map(task_from_row)
        .ok_or_else(|| AppError::NotFound("SSH task".to_string()))?;
        let rows = sqlx::query_as::<_, StoredStep>(
            r#"
            SELECT id, workspace_id, task_id, name, step_type, position, enabled,
                   config_version, config_json, created_at, updated_at, deleted_at
            FROM ssh_task_step
            WHERE workspace_id = ?1 AND task_id = ?2 AND deleted_at IS NULL
            ORDER BY position, id
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_all(&mut *connection)
        .await?;
        let steps = rows
            .into_iter()
            .map(step_from_row)
            .collect::<AppResult<Vec<_>>>()?;
        let local_binding = sqlx::query_as::<_, StoredBinding>(
            r#"
            SELECT task_id, workspace_id, default_connection_id, last_used_connection_id,
                   created_at, updated_at
            FROM ssh_task_local_binding
            WHERE workspace_id = ?1 AND task_id = ?2
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_optional(&mut *connection)
        .await?
        .map(binding_from_row);
        Ok(SshTaskDetail {
            task,
            steps,
            local_binding,
        })
    }

    pub async fn reorder_tasks(&self, input: SshTasksReorderInput) -> AppResult<Vec<SshTask>> {
        let context = CommandContext::local("ssh.task.reorder");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .reorder_tasks_on(&mut transaction, &context, input)
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    pub async fn reorder_tasks_on(
        &self,
        connection: &mut sqlx::SqliteConnection,
        context: &CommandContext,
        input: SshTasksReorderInput,
    ) -> AppResult<DomainCommandResult<Vec<SshTask>>> {
        validate_workspace_id(&input.workspace_id)?;
        for task_id in &input.task_ids {
            validate_task_id(task_id)?;
        }
        let unique = input.task_ids.iter().collect::<HashSet<_>>();
        if unique.len() != input.task_ids.len() {
            return Err(AppError::Validation(
                "SSH task reorder cannot contain duplicate task ids".to_string(),
            ));
        }
        let current = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT id, sort_order
            FROM ssh_task
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            "#,
        )
        .bind(&input.workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        let active = current.iter().map(|(id, _)| id).collect::<HashSet<_>>();
        if active.len() != unique.len() || active != unique {
            return Err(AppError::Validation(
                "SSH task reorder must contain every active task in the workspace exactly once"
                    .to_string(),
            ));
        }

        let mut mutations = Vec::new();
        for (position, task_id) in input.task_ids.iter().enumerate() {
            let sort_order = i64::try_from(position).unwrap_or(i64::MAX);
            let current_sort_order = current
                .iter()
                .find(|(id, _)| id == task_id)
                .map(|(_, value)| *value)
                .expect("validated SSH task reorder id");
            if current_sort_order == sort_order {
                continue;
            }
            let revision: i64 = sqlx::query_scalar(
                r#"
                UPDATE ssh_task
                SET sort_order = ?1, revision = revision + 1
                WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
                RETURNING revision
                "#,
            )
            .bind(sort_order)
            .bind(&input.workspace_id)
            .bind(task_id)
            .fetch_one(&mut *connection)
            .await?;
            mutations.push(mutation(
                context,
                DomainEntityType::SshTask,
                MutationOperation::Upsert,
                &input.workspace_id,
                task_id,
                None,
                revision,
            ));
        }
        Ok(DomainCommandResult::new(
            self.list_tasks_on(connection, &input.workspace_id).await?,
            mutations,
        ))
    }

    pub async fn duplicate_task(
        &self,
        workspace_id: String,
        task_id: String,
    ) -> AppResult<SshTaskDetail> {
        let context = CommandContext::local("ssh.task.duplicate");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .duplicate_task_on(&mut transaction, &context, workspace_id, task_id)
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    pub async fn duplicate_task_on(
        &self,
        connection: &mut sqlx::SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        task_id: String,
    ) -> AppResult<DomainCommandResult<SshTaskDetail>> {
        let detail = self
            .get_task_on(connection, &workspace_id, &task_id)
            .await?;
        self.save_task_on(
            connection,
            context,
            SshTaskSaveInput {
                id: None,
                workspace_id,
                name: format!("{} Copy", detail.task.name),
                description: detail.task.description,
                default_connection_id: detail
                    .local_binding
                    .and_then(|binding| binding.default_connection_id),
                steps: detail
                    .steps
                    .into_iter()
                    .map(|step| SshTaskStepInput {
                        id: None,
                        name: step.name,
                        step_type: step.step_type,
                        position: step.position,
                        enabled: step.enabled,
                        config_version: Some(step.config_version),
                        config_json: step.config_json,
                    })
                    .collect(),
            },
        )
        .await
    }

    pub async fn delete_task(&self, workspace_id: String, task_id: String) -> AppResult<()> {
        let context = CommandContext::local("ssh.task.delete");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .delete_task_on(&mut transaction, &context, workspace_id, task_id)
            .await?;
        transaction.commit().await?;
        self.remove_task_log_paths(outcome.value);
        Ok(())
    }

    pub async fn delete_task_on(
        &self,
        connection: &mut sqlx::SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        task_id: String,
    ) -> AppResult<DomainCommandResult<Vec<PathBuf>>> {
        self.get_task_on(connection, &workspace_id, &task_id)
            .await?;
        let running: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2 AND status = 'running'",
        )
        .bind(&workspace_id)
        .bind(&task_id)
        .fetch_one(&mut *connection)
        .await?;
        if running > 0 {
            return Err(AppError::Validation(
                "cannot delete an SSH task while it is running".to_string(),
            ));
        }
        let log_paths = sqlx::query_scalar::<_, String>(
            "SELECT log_path FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2",
        )
        .bind(&workspace_id)
        .bind(&task_id)
        .fetch_all(&mut *connection)
        .await?
        .into_iter()
        .map(PathBuf::from)
        .collect();
        let now = Utc::now().to_rfc3339();
        let mut mutations =
            delete_task_steps_on(connection, context, &workspace_id, &task_id, &now).await?;
        sqlx::query("DELETE FROM ssh_task_local_binding WHERE workspace_id = ?1 AND task_id = ?2")
            .bind(&workspace_id)
            .bind(&task_id)
            .execute(&mut *connection)
            .await?;
        sqlx::query("DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2")
            .bind(&workspace_id)
            .bind(&task_id)
            .execute(&mut *connection)
            .await?;
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE ssh_task
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
            WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&task_id)
        .fetch_one(&mut *connection)
        .await?;
        mutations.push(mutation(
            context,
            DomainEntityType::SshTask,
            MutationOperation::Delete,
            &workspace_id,
            &task_id,
            None,
            revision,
        ));
        Ok(DomainCommandResult::new(log_paths, mutations))
    }

    pub fn remove_task_log_paths(&self, paths: Vec<PathBuf>) {
        remove_task_logs(paths, &self.task_log_dir);
    }
}
