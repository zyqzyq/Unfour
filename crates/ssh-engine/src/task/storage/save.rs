use super::super::domain::{mutation, validate_workspace_on};
use super::super::*;
use super::convert::*;
use std::collections::{HashMap, HashSet};
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityType, MutationOperation,
};

#[derive(sqlx::FromRow)]
struct ExistingTask {
    name: String,
    description: String,
}

#[derive(sqlx::FromRow)]
struct ExistingStep {
    id: String,
    name: String,
    step_type: String,
    position: i64,
    enabled: i64,
    config_version: i64,
    config_json: String,
    deleted_at: Option<String>,
}

impl SshService {
    pub async fn save_task(&self, input: SshTaskSaveInput) -> AppResult<SshTaskDetail> {
        let context = CommandContext::local("ssh.task.save");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self.save_task_on(&mut transaction, &context, input).await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    pub async fn save_task_on(
        &self,
        connection: &mut sqlx::SqliteConnection,
        context: &CommandContext,
        mut input: SshTaskSaveInput,
    ) -> AppResult<DomainCommandResult<SshTaskDetail>> {
        validate_workspace_id(&input.workspace_id)?;
        validate_workspace_on(connection, &input.workspace_id).await?;
        let name = normalize_task_name(&input.name)?;
        let description = input.description.trim().to_string();
        if description.chars().count() > 2_000 {
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
            validate_default_connection_on(connection, &input.workspace_id, connection_id).await?;
        }
        input.steps.sort_by_key(|step| step.position);
        for (position, step) in input.steps.iter_mut().enumerate() {
            step.position = i64::try_from(position).unwrap_or(i64::MAX);
            step.name = normalize_step_name(&step.name, &step.step_type, position)?;
        }

        let now = Utc::now().to_rfc3339();
        let id = input.id.clone().unwrap_or_else(unfour_core::id::new_id);
        validate_task_id(&id)?;
        let existing = sqlx::query_as::<_, ExistingTask>(
            r#"
            SELECT name, description
            FROM ssh_task
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(&input.workspace_id)
        .bind(&id)
        .fetch_optional(&mut *connection)
        .await?;
        if input.id.is_some() && existing.is_none() {
            return Err(AppError::NotFound("SSH task".to_string()));
        }

        let existing_steps = sqlx::query_as::<_, ExistingStep>(
            r#"
            SELECT id, name, step_type, position, enabled, config_version,
                   config_json, deleted_at
            FROM ssh_task_step
            WHERE workspace_id = ?1 AND task_id = ?2
            "#,
        )
        .bind(&input.workspace_id)
        .bind(&id)
        .fetch_all(&mut *connection)
        .await?
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>();
        let mut desired_ids = HashSet::new();
        for step in &mut input.steps {
            let step_id = step.id.get_or_insert_with(unfour_core::id::new_id).clone();
            validate_task_id(&step_id)?;
            if !desired_ids.insert(step_id.clone()) {
                return Err(AppError::Validation(
                    "SSH task steps cannot contain duplicate ids".to_string(),
                ));
            }
            if !existing_steps.contains_key(&step_id) {
                let owner: Option<(String, String)> =
                    sqlx::query_as("SELECT workspace_id, task_id FROM ssh_task_step WHERE id = ?1")
                        .bind(&step_id)
                        .fetch_optional(&mut *connection)
                        .await?;
                if owner.is_some() {
                    return Err(AppError::Validation(
                        "SSH task step id already belongs to another task".to_string(),
                    ));
                }
            }
            let config_version = match existing_steps.get(&step_id) {
                Some(stored) => {
                    if step
                        .config_version
                        .is_some_and(|requested| requested != stored.config_version)
                    {
                        return Err(AppError::Validation(format!(
                            "SSH task step config version cannot be changed by a normal update (stored {})",
                            stored.config_version
                        )));
                    }
                    stored.config_version
                }
                None => step.config_version.unwrap_or(1),
            };
            validate_step_config(&step.step_type, config_version, &step.config_json)?;
            step.config_version = Some(config_version);
        }
        let had_binding: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ssh_task_local_binding WHERE workspace_id = ?1 AND task_id = ?2)",
        )
        .bind(&input.workspace_id)
        .bind(&id)
        .fetch_one(&mut *connection)
        .await?;

        let mut mutations = Vec::new();
        match existing.as_ref() {
            Some(current) if current.name != name || current.description != description => {
                let revision: i64 = sqlx::query_scalar(
                    r#"
                    UPDATE ssh_task
                    SET name = ?1, description = ?2, updated_at = ?3,
                        revision = revision + 1
                    WHERE workspace_id = ?4 AND id = ?5 AND deleted_at IS NULL
                    RETURNING revision
                    "#,
                )
                .bind(&name)
                .bind(&description)
                .bind(&now)
                .bind(&input.workspace_id)
                .bind(&id)
                .fetch_one(&mut *connection)
                .await?;
                mutations.push(mutation(
                    context,
                    DomainEntityType::SshTask,
                    MutationOperation::Upsert,
                    &input.workspace_id,
                    &id,
                    None,
                    revision,
                ));
            }
            Some(_) => {}
            None => {
                let sort_order: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COALESCE(MAX(sort_order), -1) + 1
                    FROM ssh_task
                    WHERE workspace_id = ?1 AND deleted_at IS NULL
                    "#,
                )
                .bind(&input.workspace_id)
                .fetch_one(&mut *connection)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO ssh_task (
                      id, workspace_id, name, description, sort_order,
                      created_at, updated_at, deleted_at, revision
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, NULL, 1)
                    "#,
                )
                .bind(&id)
                .bind(&input.workspace_id)
                .bind(&name)
                .bind(&description)
                .bind(sort_order)
                .bind(&now)
                .execute(&mut *connection)
                .await?;
                mutations.push(mutation(
                    context,
                    DomainEntityType::SshTask,
                    MutationOperation::Upsert,
                    &input.workspace_id,
                    &id,
                    None,
                    1,
                ));
            }
        }

        // Stage active rows as soft-deleted. Desired rows are restored below;
        // omitted rows retain this timestamp and receive a Delete mutation.
        sqlx::query(
            "UPDATE ssh_task_step SET deleted_at = ?1 WHERE workspace_id = ?2 AND task_id = ?3 AND deleted_at IS NULL",
        )
        .bind(&now)
        .bind(&input.workspace_id)
        .bind(&id)
        .execute(&mut *connection)
        .await?;

        for step in input.steps {
            let step_id = step.id.expect("step ids are assigned before persistence");
            let config_version = step
                .config_version
                .expect("config versions are assigned before persistence");
            let config_json = serde_json::to_string(&step.config_json)?;
            if let Some(current) = existing_steps.get(&step_id) {
                let current_config: serde_json::Value = serde_json::from_str(&current.config_json)
                    .map_err(|error| {
                        AppError::Config(format!("stored SSH task step config is invalid: {error}"))
                    })?;
                let changed = current.deleted_at.is_some()
                    || current.name != step.name
                    || current.step_type != step.step_type
                    || current.position != step.position
                    || (current.enabled != 0) != step.enabled
                    || current.config_version != config_version
                    || current_config != step.config_json;
                if changed {
                    let revision: i64 = sqlx::query_scalar(
                        r#"
                        UPDATE ssh_task_step
                        SET name = ?1, step_type = ?2, position = ?3, enabled = ?4,
                            config_json = ?5, updated_at = ?6, deleted_at = NULL,
                            revision = revision + 1
                        WHERE workspace_id = ?7 AND task_id = ?8 AND id = ?9
                        RETURNING revision
                        "#,
                    )
                    .bind(&step.name)
                    .bind(&step.step_type)
                    .bind(step.position)
                    .bind(if step.enabled { 1_i64 } else { 0_i64 })
                    .bind(config_json)
                    .bind(&now)
                    .bind(&input.workspace_id)
                    .bind(&id)
                    .bind(&step_id)
                    .fetch_one(&mut *connection)
                    .await?;
                    mutations.push(mutation(
                        context,
                        DomainEntityType::SshTaskStep,
                        MutationOperation::Upsert,
                        &input.workspace_id,
                        &step_id,
                        Some(&id),
                        revision,
                    ));
                } else {
                    sqlx::query(
                        "UPDATE ssh_task_step SET deleted_at = NULL WHERE workspace_id = ?1 AND task_id = ?2 AND id = ?3",
                    )
                    .bind(&input.workspace_id)
                    .bind(&id)
                    .bind(&step_id)
                    .execute(&mut *connection)
                    .await?;
                }
            } else {
                sqlx::query(
                    r#"
                    INSERT INTO ssh_task_step (
                      id, workspace_id, task_id, name, step_type, position, enabled,
                      config_version, config_json, created_at, updated_at, deleted_at,
                      revision
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, NULL, 1)
                    "#,
                )
                .bind(&step_id)
                .bind(&input.workspace_id)
                .bind(&id)
                .bind(&step.name)
                .bind(&step.step_type)
                .bind(step.position)
                .bind(if step.enabled { 1_i64 } else { 0_i64 })
                .bind(config_version)
                .bind(config_json)
                .bind(&now)
                .execute(&mut *connection)
                .await?;
                mutations.push(mutation(
                    context,
                    DomainEntityType::SshTaskStep,
                    MutationOperation::Upsert,
                    &input.workspace_id,
                    &step_id,
                    Some(&id),
                    1,
                ));
            }
        }
        for current in existing_steps.values().filter(|current| {
            current.deleted_at.is_none() && !desired_ids.contains(current.id.as_str())
        }) {
            let revision: i64 = sqlx::query_scalar(
                r#"
                UPDATE ssh_task_step
                SET updated_at = ?1, revision = revision + 1
                WHERE workspace_id = ?2 AND task_id = ?3 AND id = ?4
                RETURNING revision
                "#,
            )
            .bind(&now)
            .bind(&input.workspace_id)
            .bind(&id)
            .bind(&current.id)
            .fetch_one(&mut *connection)
            .await?;
            mutations.push(mutation(
                context,
                DomainEntityType::SshTaskStep,
                MutationOperation::Delete,
                &input.workspace_id,
                &current.id,
                Some(&id),
                revision,
            ));
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
                  updated_at = CASE
                    WHEN ssh_task_local_binding.default_connection_id IS excluded.default_connection_id
                    THEN ssh_task_local_binding.updated_at
                    ELSE excluded.updated_at
                  END
                "#,
            )
            .bind(&id)
            .bind(&input.workspace_id)
            .bind(input.default_connection_id.as_deref())
            .bind(&now)
            .execute(&mut *connection)
            .await?;
        }
        let detail = self
            .get_task_on(connection, &input.workspace_id, &id)
            .await?;
        Ok(DomainCommandResult::new(detail, mutations))
    }
}

async fn validate_default_connection_on(
    connection: &mut sqlx::SqliteConnection,
    workspace_id: &str,
    connection_id: &str,
) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM connections
          WHERE id = ?1 AND workspace_id = ?2
            AND connection_type = 'ssh' AND deleted_at IS NULL
        )
        "#,
    )
    .bind(connection_id)
    .bind(workspace_id)
    .fetch_one(&mut *connection)
    .await?;
    if !exists {
        return Err(AppError::NotFound("ssh connection".to_string()));
    }
    Ok(())
}
