use super::super::*;
use super::convert::*;
use std::collections::HashMap;

impl SshService {
    pub async fn save_task(&self, mut input: SshTaskSaveInput) -> AppResult<SshTaskDetail> {
        validate_workspace_id(&input.workspace_id)?;
        let name = normalize_task_name(&input.name)?;
        if input.description.chars().count() > 2_000 {
            return Err(AppError::Validation(
                "SSH task description must be 2000 characters or fewer".to_string(),
            ));
        }
        if input.steps.len() > 100 {
            return Err(AppError::Validation(
                "SSH task cannot contain more than 100 steps".to_string(),
            ));
        }
        if let Some(connection_id) = input.default_connection_id.as_deref() {
            self.get_connection(&input.workspace_id, connection_id)
                .await?;
        }
        input.steps.sort_by_key(|step| step.position);
        for (position, step) in input.steps.iter_mut().enumerate() {
            step.position = position as i64;
            step.name = normalize_step_name(&step.name, &step.step_type, position)?;
        }

        let now = Utc::now().to_rfc3339();
        let id = input.id.clone().unwrap_or_else(unfour_core::id::new_id);
        validate_task_id(&id)?;
        let existing = sqlx::query(
            "SELECT created_at FROM ssh_task WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL",
        )
                .bind(&input.workspace_id)
                .bind(&id)
                .fetch_optional(self.db.pool())
                .await?;
        if input.id.is_some() && existing.is_none() {
            return Err(AppError::NotFound("SSH task".to_string()));
        }
        let created_at = existing
            .as_ref()
            .map(|row| row.get::<String, _>("created_at"))
            .unwrap_or_else(|| now.clone());

        let existing_steps = sqlx::query(
            "SELECT id, created_at, config_version FROM ssh_task_step WHERE workspace_id = ?1 AND task_id = ?2",
        )
        .bind(&input.workspace_id)
        .bind(&id)
        .fetch_all(self.db.pool())
        .await?
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("id"),
                (
                    row.get::<String, _>("created_at"),
                    row.get::<i64, _>("config_version"),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
        let had_binding: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ssh_task_local_binding WHERE workspace_id = ?1 AND task_id = ?2)",
        )
        .bind(&input.workspace_id)
        .bind(&id)
        .fetch_one(self.db.pool())
        .await?;

        for step in &mut input.steps {
            let step_id = step.id.get_or_insert_with(unfour_core::id::new_id).clone();
            validate_task_id(&step_id)?;
            let config_version = match existing_steps.get(&step_id) {
                Some((_, stored_version)) => {
                    if step
                        .config_version
                        .is_some_and(|requested| requested != *stored_version)
                    {
                        return Err(AppError::Validation(format!(
                            "SSH task step config version cannot be changed by a normal update (stored {stored_version})"
                        )));
                    }
                    *stored_version
                }
                None => step.config_version.unwrap_or(1),
            };
            validate_step_config(&step.step_type, config_version, &step.config_json)?;
            step.config_version = Some(config_version);
        }

        let mut transaction = self.db.pool().begin().await?;
        if existing.is_some() {
            sqlx::query(
                r#"
                UPDATE ssh_task
                SET name = ?1, description = ?2, updated_at = ?3
                WHERE workspace_id = ?4 AND id = ?5 AND deleted_at IS NULL
                "#,
            )
            .bind(&name)
            .bind(input.description.trim())
            .bind(&now)
            .bind(&input.workspace_id)
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        } else {
            let sort_order: i64 = sqlx::query_scalar(
                r#"
                SELECT COALESCE(MAX(sort_order), -1) + 1
                FROM ssh_task
                WHERE workspace_id = ?1 AND deleted_at IS NULL
                "#,
            )
            .bind(&input.workspace_id)
            .fetch_one(&mut *transaction)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO ssh_task (
                  id, workspace_id, name, description, sort_order,
                  created_at, updated_at, deleted_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)
                "#,
            )
            .bind(&id)
            .bind(&input.workspace_id)
            .bind(&name)
            .bind(input.description.trim())
            .bind(sort_order)
            .bind(&created_at)
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query(
            r#"
            UPDATE ssh_task_step
            SET deleted_at = ?1, updated_at = ?1
            WHERE workspace_id = ?2 AND task_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&input.workspace_id)
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
        for step in input.steps {
            let step_id = step.id.expect("step ids are assigned before persistence");
            let config_version = step
                .config_version
                .expect("config versions are assigned before persistence");
            if existing_steps.contains_key(&step_id) {
                sqlx::query(
                    r#"
                    UPDATE ssh_task_step
                    SET name = ?1, step_type = ?2, position = ?3, enabled = ?4,
                        config_json = ?5, updated_at = ?6, deleted_at = NULL
                    WHERE workspace_id = ?7 AND task_id = ?8 AND id = ?9
                    "#,
                )
                .bind(step.name)
                .bind(step.step_type)
                .bind(step.position)
                .bind(if step.enabled { 1_i64 } else { 0_i64 })
                .bind(serde_json::to_string(&step.config_json)?)
                .bind(&now)
                .bind(&input.workspace_id)
                .bind(&id)
                .bind(step_id)
                .execute(&mut *transaction)
                .await?;
            } else {
                sqlx::query(
                    r#"
                    INSERT INTO ssh_task_step (
                      id, workspace_id, task_id, name, step_type, position, enabled,
                      config_version, config_json, created_at, updated_at, deleted_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)
                    "#,
                )
                .bind(step_id)
                .bind(&input.workspace_id)
                .bind(&id)
                .bind(step.name)
                .bind(step.step_type)
                .bind(step.position)
                .bind(if step.enabled { 1_i64 } else { 0_i64 })
                .bind(config_version)
                .bind(serde_json::to_string(&step.config_json)?)
                .bind(&now)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
            }
        }
        if input.default_connection_id.is_some() || had_binding {
            sqlx::query(
                r#"
                INSERT INTO ssh_task_local_binding (
                  task_id, workspace_id, default_connection_id, last_used_connection_id,
                  created_at, updated_at
                ) VALUES (?1, ?2, ?3, NULL, ?4, ?4)
                ON CONFLICT(task_id) DO UPDATE SET
                  default_connection_id = excluded.default_connection_id,
                  updated_at = excluded.updated_at
                "#,
            )
            .bind(&id)
            .bind(&input.workspace_id)
            .bind(input.default_connection_id.as_deref())
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        self.get_task(&input.workspace_id, &id).await
    }
}
