use super::super::*;
use super::convert::*;
use std::collections::HashSet;

impl SshService {
    pub async fn list_tasks(&self, workspace_id: String) -> AppResult<Vec<SshTask>> {
        validate_workspace_id(&workspace_id)?;
        let rows = sqlx::query_as::<_, StoredTask>(
            r#"
            SELECT id, workspace_id, name, description, sort_order,
                   created_at, updated_at, deleted_at
            FROM ssh_task
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY sort_order, name COLLATE NOCASE, id
            "#,
        )
        .bind(&workspace_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(task_from_row).collect())
    }

    pub async fn get_task(&self, workspace_id: &str, task_id: &str) -> AppResult<SshTaskDetail> {
        validate_workspace_id(workspace_id)?;
        validate_task_id(task_id)?;
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
        .fetch_optional(self.db.pool())
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
        .fetch_all(self.db.pool())
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
        .fetch_optional(self.db.pool())
        .await?
        .map(binding_from_row);
        Ok(SshTaskDetail {
            task,
            steps,
            local_binding,
        })
    }

    pub async fn reorder_tasks(&self, input: SshTasksReorderInput) -> AppResult<Vec<SshTask>> {
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
        let mut transaction = self.db.pool().begin().await?;
        let active_ids = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id
            FROM ssh_task
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            "#,
        )
        .bind(&input.workspace_id)
        .fetch_all(&mut *transaction)
        .await?;
        let active = active_ids.iter().collect::<HashSet<_>>();
        if active.len() != unique.len() || active != unique {
            return Err(AppError::Validation(
                "SSH task reorder must contain every active task in the workspace exactly once"
                    .to_string(),
            ));
        }

        for (position, task_id) in input.task_ids.iter().enumerate() {
            sqlx::query(
                r#"
                UPDATE ssh_task
                SET sort_order = ?1
                WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
                  AND sort_order <> ?1
                "#,
            )
            .bind(position as i64)
            .bind(&input.workspace_id)
            .bind(task_id)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        self.list_tasks(input.workspace_id).await
    }

    pub async fn duplicate_task(
        &self,
        workspace_id: String,
        task_id: String,
    ) -> AppResult<SshTaskDetail> {
        let detail = self.get_task(&workspace_id, &task_id).await?;
        self.save_task(SshTaskSaveInput {
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
        })
        .await
    }

    pub async fn delete_task(&self, workspace_id: String, task_id: String) -> AppResult<()> {
        self.get_task(&workspace_id, &task_id).await?;
        let running: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2 AND status = 'running'",
        )
        .bind(&workspace_id)
        .bind(&task_id)
        .fetch_one(self.db.pool())
        .await?;
        if running > 0 {
            return Err(AppError::Validation(
                "cannot delete an SSH task while it is running".to_string(),
            ));
        }
        let log_paths = self.task_log_paths(&workspace_id, Some(&task_id)).await?;
        let now = Utc::now().to_rfc3339();
        let mut transaction = self.db.pool().begin().await?;
        sqlx::query(
            r#"
            UPDATE ssh_task_step
            SET deleted_at = ?1, updated_at = ?1
            WHERE workspace_id = ?2 AND task_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&task_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM ssh_task_local_binding WHERE workspace_id = ?1 AND task_id = ?2")
            .bind(&workspace_id)
            .bind(&task_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2")
            .bind(&workspace_id)
            .bind(&task_id)
            .execute(&mut *transaction)
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE ssh_task
            SET deleted_at = ?1, updated_at = ?1
            WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&task_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("SSH task".to_string()));
        }
        transaction.commit().await?;
        remove_task_logs(log_paths, &self.task_log_dir);
        Ok(())
    }
}
