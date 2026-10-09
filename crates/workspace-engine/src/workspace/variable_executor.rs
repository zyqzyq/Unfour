use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityKey, DomainEntityType, DomainMutation,
    MutationOperation,
};
use unfour_core::models::{
    WorkspaceEnvironment, WorkspaceEnvironmentVariable, WorkspaceVariable, WorkspaceVariableInput,
};
use unfour_core::{AppError, AppResult};

use super::variable_persistence::{
    insert_environment_variable, insert_workspace_variable, replace_environment_variables_exact,
    replace_workspace_variables_exact, same_environment_variable, same_workspace_variable,
    soft_delete, update_environment_variable, update_workspace_variable,
};
use super::variables::{
    active_environment_id_on, assert_environment_name_unique_on, get_environment_on,
    get_environment_variable_on, get_variable_on, list_environment_variables_on,
    list_environments_on, list_variables_on, non_empty_optional, normalize_environment_name,
    validate_variables,
};
use super::{get_workspace_on, WorkspaceService};

impl WorkspaceService {
    pub async fn replace_variables_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        variables: Vec<WorkspaceVariableInput>,
    ) -> AppResult<DomainCommandResult<Vec<WorkspaceVariable>>> {
        get_workspace_on(connection, &workspace_id, false).await?;
        validate_variables(&variables)?;
        let mutations =
            replace_workspace_variables_exact(connection, context, &workspace_id, variables)
                .await?;
        Ok(DomainCommandResult::new(
            list_variables_on(connection, &workspace_id, false).await?,
            mutations,
        ))
    }

    pub async fn create_variable_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        mut input: WorkspaceVariableInput,
    ) -> AppResult<DomainCommandResult<WorkspaceVariable>> {
        get_workspace_on(connection, &workspace_id, false).await?;
        input.id = None;
        validate_variables(std::slice::from_ref(&input))?;
        let id = unfour_core::id::new_id();
        let now = Utc::now().to_rfc3339();
        insert_workspace_variable(
            connection,
            &workspace_id,
            &id,
            &input,
            input.sort_order,
            &now,
        )
        .await?;
        let value = get_variable_on(connection, &workspace_id, &id, false).await?;
        Ok(DomainCommandResult::new(
            value,
            vec![entity_mutation(
                context,
                DomainEntityType::WorkspaceVariable,
                MutationOperation::Upsert,
                &workspace_id,
                &id,
                1,
            )],
        ))
    }

    pub async fn update_variable_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        variable_id: String,
        mut input: WorkspaceVariableInput,
    ) -> AppResult<DomainCommandResult<WorkspaceVariable>> {
        get_workspace_on(connection, &workspace_id, false).await?;
        input.id = Some(variable_id.clone());
        validate_variables(std::slice::from_ref(&input))?;
        let current = get_variable_on(connection, &workspace_id, &variable_id, false).await?;
        if same_workspace_variable(&current, &input, input.sort_order) {
            return Ok(DomainCommandResult::unchanged(current));
        }
        let revision = update_workspace_variable(
            connection,
            &workspace_id,
            &variable_id,
            &input,
            input.sort_order,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        Ok(DomainCommandResult::new(
            get_variable_on(connection, &workspace_id, &variable_id, false).await?,
            vec![entity_mutation(
                context,
                DomainEntityType::WorkspaceVariable,
                MutationOperation::Upsert,
                &workspace_id,
                &variable_id,
                revision,
            )],
        ))
    }

    pub async fn delete_variable_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        variable_id: String,
    ) -> AppResult<DomainCommandResult<Vec<WorkspaceVariable>>> {
        get_variable_on(connection, &workspace_id, &variable_id, false).await?;
        let revision = soft_delete(
            connection,
            "workspace_variables",
            &workspace_id,
            None,
            &variable_id,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        Ok(DomainCommandResult::new(
            list_variables_on(connection, &workspace_id, false).await?,
            vec![entity_mutation(
                context,
                DomainEntityType::WorkspaceVariable,
                MutationOperation::Delete,
                &workspace_id,
                &variable_id,
                revision,
            )],
        ))
    }

    pub async fn create_environment_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        name: String,
    ) -> AppResult<DomainCommandResult<WorkspaceEnvironment>> {
        get_workspace_on(connection, &workspace_id, false).await?;
        let name = normalize_environment_name(name)?;
        assert_environment_name_unique_on(connection, &workspace_id, &name, None).await?;
        let existing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workspace_environments WHERE workspace_id = ?1 AND deleted_at IS NULL",
        )
        .bind(&workspace_id)
        .fetch_one(&mut *connection)
        .await?;
        let sort_order: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM workspace_environments WHERE workspace_id = ?1 AND deleted_at IS NULL",
        )
        .bind(&workspace_id)
        .fetch_one(&mut *connection)
        .await?;
        let id = unfour_core::id::new_id();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO workspace_environments (
              id, workspace_id, name, sort_order, created_at, updated_at, revision
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(name)
        .bind(sort_order)
        .bind(&now)
        .execute(&mut *connection)
        .await?;
        if existing == 0 {
            sqlx::query(
                "UPDATE workspace_local_state SET active_environment_id = ?1, updated_at = ?2 WHERE workspace_id = ?3",
            )
            .bind(&id)
            .bind(&now)
            .bind(&workspace_id)
            .execute(&mut *connection)
            .await?;
        }
        Ok(DomainCommandResult::new(
            get_environment_on(connection, &workspace_id, &id, false).await?,
            vec![entity_mutation(
                context,
                DomainEntityType::WorkspaceEnvironment,
                MutationOperation::Upsert,
                &workspace_id,
                &id,
                1,
            )],
        ))
    }

    pub async fn update_environment_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_id: String,
        name: String,
        variables: Vec<WorkspaceVariableInput>,
    ) -> AppResult<DomainCommandResult<WorkspaceEnvironment>> {
        get_workspace_on(connection, &workspace_id, false).await?;
        let current = get_environment_on(connection, &workspace_id, &environment_id, false).await?;
        let name = normalize_environment_name(name)?;
        validate_variables(&variables)?;
        assert_environment_name_unique_on(connection, &workspace_id, &name, Some(&environment_id))
            .await?;
        let now = Utc::now().to_rfc3339();
        let mut mutations = Vec::new();
        if current.name != name {
            let revision: i64 = sqlx::query_scalar(
                r#"
                UPDATE workspace_environments
                SET name = ?1, updated_at = ?2, revision = revision + 1
                WHERE id = ?3 AND workspace_id = ?4 AND deleted_at IS NULL
                RETURNING revision
                "#,
            )
            .bind(name)
            .bind(&now)
            .bind(&environment_id)
            .bind(&workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            mutations.push(entity_mutation(
                context,
                DomainEntityType::WorkspaceEnvironment,
                MutationOperation::Upsert,
                &workspace_id,
                &environment_id,
                revision,
            ));
        }
        mutations.extend(
            replace_environment_variables_exact(
                connection,
                context,
                &workspace_id,
                &environment_id,
                variables,
            )
            .await?,
        );
        Ok(DomainCommandResult::new(
            get_environment_on(connection, &workspace_id, &environment_id, false).await?,
            mutations,
        ))
    }

    pub async fn update_environment_metadata_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_id: String,
        name: String,
        sort_order: i64,
    ) -> AppResult<DomainCommandResult<WorkspaceEnvironment>> {
        let current = get_environment_on(connection, &workspace_id, &environment_id, false).await?;
        let name = normalize_environment_name(name)?;
        assert_environment_name_unique_on(connection, &workspace_id, &name, Some(&environment_id))
            .await?;
        if current.name == name && current.sort_order == sort_order {
            return Ok(DomainCommandResult::unchanged(current));
        }
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE workspace_environments
            SET name = ?1, sort_order = ?2, updated_at = ?3, revision = revision + 1
            WHERE id = ?4 AND workspace_id = ?5 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(name)
        .bind(sort_order)
        .bind(Utc::now().to_rfc3339())
        .bind(&environment_id)
        .bind(&workspace_id)
        .fetch_one(&mut *connection)
        .await?;
        Ok(DomainCommandResult::new(
            get_environment_on(connection, &workspace_id, &environment_id, false).await?,
            vec![entity_mutation(
                context,
                DomainEntityType::WorkspaceEnvironment,
                MutationOperation::Upsert,
                &workspace_id,
                &environment_id,
                revision,
            )],
        ))
    }

    pub async fn reorder_environments_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_ids: Vec<String>,
    ) -> AppResult<DomainCommandResult<Vec<WorkspaceEnvironment>>> {
        get_workspace_on(connection, &workspace_id, false).await?;
        let current = list_environments_on(connection, &workspace_id).await?;
        let current_ids: HashSet<_> = current.iter().map(|item| item.id.as_str()).collect();
        let desired_ids: HashSet<_> = environment_ids.iter().map(String::as_str).collect();
        if current_ids != desired_ids || environment_ids.len() != desired_ids.len() {
            return Err(AppError::Validation(
                "environment reorder must contain each active environment exactly once".to_string(),
            ));
        }
        let by_id: HashMap<_, _> = current
            .iter()
            .map(|environment| (environment.id.as_str(), environment))
            .collect();
        let now = Utc::now().to_rfc3339();
        let mut mutations = Vec::new();
        for (index, id) in environment_ids.iter().enumerate() {
            let sort_order = i64::try_from(index).unwrap_or(i64::MAX);
            if by_id[id.as_str()].sort_order == sort_order {
                continue;
            }
            let revision: i64 = sqlx::query_scalar(
                r#"
                UPDATE workspace_environments
                SET sort_order = ?1, updated_at = ?2, revision = revision + 1
                WHERE id = ?3 AND workspace_id = ?4 AND deleted_at IS NULL
                RETURNING revision
                "#,
            )
            .bind(sort_order)
            .bind(&now)
            .bind(id)
            .bind(&workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            mutations.push(entity_mutation(
                context,
                DomainEntityType::WorkspaceEnvironment,
                MutationOperation::Upsert,
                &workspace_id,
                id,
                revision,
            ));
        }
        Ok(DomainCommandResult::new(
            list_environments_on(connection, &workspace_id).await?,
            mutations,
        ))
    }

    pub async fn delete_environment_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_id: String,
    ) -> AppResult<DomainCommandResult<Vec<WorkspaceEnvironment>>> {
        get_environment_on(connection, &workspace_id, &environment_id, false).await?;
        let child_variables =
            list_environment_variables_on(connection, &workspace_id, &environment_id, false)
                .await?;
        let now = Utc::now().to_rfc3339();
        let environment_revision = soft_delete(
            connection,
            "workspace_environments",
            &workspace_id,
            None,
            &environment_id,
            &now,
        )
        .await?;
        let mut mutations = vec![entity_mutation(
            context,
            DomainEntityType::WorkspaceEnvironment,
            MutationOperation::Delete,
            &workspace_id,
            &environment_id,
            environment_revision,
        )];
        for variable in child_variables {
            let revision = soft_delete(
                connection,
                "workspace_environment_variables",
                &workspace_id,
                Some(&environment_id),
                &variable.id,
                &now,
            )
            .await?;
            mutations.push(
                entity_mutation(
                    context,
                    DomainEntityType::WorkspaceEnvironmentVariable,
                    MutationOperation::Delete,
                    &workspace_id,
                    &variable.id,
                    revision,
                )
                .with_parent_entity_id(&environment_id),
            );
        }

        update_active_environment_after_delete_on(connection, &workspace_id, &environment_id, &now)
            .await?;

        Ok(DomainCommandResult::new(
            list_environments_on(connection, &workspace_id).await?,
            mutations,
        ))
    }

    pub async fn set_active_environment_on(
        &self,
        connection: &mut SqliteConnection,
        _context: &CommandContext,
        workspace_id: String,
        environment_id: Option<String>,
    ) -> AppResult<DomainCommandResult<Vec<WorkspaceEnvironment>>> {
        get_workspace_on(connection, &workspace_id, false).await?;
        let environment_id = non_empty_optional(environment_id);
        if let Some(environment_id) = environment_id.as_deref() {
            get_environment_on(connection, &workspace_id, environment_id, false).await?;
        }
        let current = active_environment_id_on(connection, &workspace_id).await?;
        if current != environment_id {
            sqlx::query(
                "UPDATE workspace_local_state SET active_environment_id = ?1, updated_at = ?2 WHERE workspace_id = ?3",
            )
            .bind(environment_id)
            .bind(Utc::now().to_rfc3339())
            .bind(&workspace_id)
            .execute(&mut *connection)
            .await?;
        }
        Ok(DomainCommandResult::unchanged(
            list_environments_on(connection, &workspace_id).await?,
        ))
    }

    pub async fn create_environment_variable_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_id: String,
        mut input: WorkspaceVariableInput,
    ) -> AppResult<DomainCommandResult<WorkspaceEnvironmentVariable>> {
        get_environment_on(connection, &workspace_id, &environment_id, false).await?;
        input.id = None;
        validate_variables(std::slice::from_ref(&input))?;
        let id = unfour_core::id::new_id();
        let now = Utc::now().to_rfc3339();
        insert_environment_variable(
            connection,
            &workspace_id,
            &environment_id,
            &id,
            &input,
            input.sort_order,
            &now,
        )
        .await?;
        Ok(DomainCommandResult::new(
            get_environment_variable_on(connection, &workspace_id, &environment_id, &id, false)
                .await?,
            vec![entity_mutation(
                context,
                DomainEntityType::WorkspaceEnvironmentVariable,
                MutationOperation::Upsert,
                &workspace_id,
                &id,
                1,
            )
            .with_parent_entity_id(&environment_id)],
        ))
    }

    pub async fn update_environment_variable_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_id: String,
        variable_id: String,
        mut input: WorkspaceVariableInput,
    ) -> AppResult<DomainCommandResult<WorkspaceEnvironmentVariable>> {
        input.id = Some(variable_id.clone());
        validate_variables(std::slice::from_ref(&input))?;
        let current = get_environment_variable_on(
            connection,
            &workspace_id,
            &environment_id,
            &variable_id,
            false,
        )
        .await?;
        if same_environment_variable(&current, &input, input.sort_order) {
            return Ok(DomainCommandResult::unchanged(current));
        }
        let revision = update_environment_variable(
            connection,
            &workspace_id,
            &environment_id,
            &variable_id,
            &input,
            input.sort_order,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        Ok(DomainCommandResult::new(
            get_environment_variable_on(
                connection,
                &workspace_id,
                &environment_id,
                &variable_id,
                false,
            )
            .await?,
            vec![entity_mutation(
                context,
                DomainEntityType::WorkspaceEnvironmentVariable,
                MutationOperation::Upsert,
                &workspace_id,
                &variable_id,
                revision,
            )
            .with_parent_entity_id(&environment_id)],
        ))
    }

    pub async fn replace_environment_variables_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_id: String,
        variables: Vec<WorkspaceVariableInput>,
    ) -> AppResult<DomainCommandResult<Vec<WorkspaceEnvironmentVariable>>> {
        get_environment_on(connection, &workspace_id, &environment_id, false).await?;
        validate_variables(&variables)?;
        let mutations = replace_environment_variables_exact(
            connection,
            context,
            &workspace_id,
            &environment_id,
            variables,
        )
        .await?;
        Ok(DomainCommandResult::new(
            list_environment_variables_on(connection, &workspace_id, &environment_id, false)
                .await?,
            mutations,
        ))
    }

    pub async fn delete_environment_variable_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_id: String,
        variable_id: String,
    ) -> AppResult<DomainCommandResult<Vec<WorkspaceEnvironmentVariable>>> {
        get_environment_variable_on(
            connection,
            &workspace_id,
            &environment_id,
            &variable_id,
            false,
        )
        .await?;
        let revision = soft_delete(
            connection,
            "workspace_environment_variables",
            &workspace_id,
            Some(&environment_id),
            &variable_id,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        Ok(DomainCommandResult::new(
            list_environment_variables_on(connection, &workspace_id, &environment_id, false)
                .await?,
            vec![entity_mutation(
                context,
                DomainEntityType::WorkspaceEnvironmentVariable,
                MutationOperation::Delete,
                &workspace_id,
                &variable_id,
                revision,
            )
            .with_parent_entity_id(&environment_id)],
        ))
    }
}

pub(crate) fn entity_mutation(
    context: &CommandContext,
    entity_type: DomainEntityType,
    operation: MutationOperation,
    workspace_id: &str,
    entity_id: &str,
    revision: i64,
) -> DomainMutation {
    DomainMutation::new(
        context.origin,
        operation,
        DomainEntityKey::new(entity_type, workspace_id, entity_id),
        revision,
    )
}

pub(crate) fn entity_mutation_with_parent(
    context: &CommandContext,
    entity_type: DomainEntityType,
    operation: MutationOperation,
    workspace_id: &str,
    entity_id: &str,
    parent_entity_id: &str,
    revision: i64,
) -> DomainMutation {
    entity_mutation(
        context,
        entity_type,
        operation,
        workspace_id,
        entity_id,
        revision,
    )
    .with_parent_entity_id(parent_entity_id)
}

pub(crate) async fn update_active_environment_after_delete_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    deleted_environment_id: &str,
    now: &str,
) -> AppResult<()> {
    let selected: Option<String> = sqlx::query_scalar(
        "SELECT active_environment_id FROM workspace_local_state WHERE workspace_id = ?1",
    )
    .bind(workspace_id)
    .fetch_optional(&mut *connection)
    .await?
    .flatten();
    if selected.as_deref() != Some(deleted_environment_id) {
        return Ok(());
    }
    let fallback: Option<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM workspace_environments
        WHERE workspace_id = ?1 AND deleted_at IS NULL
        ORDER BY sort_order ASC, created_at ASC, id ASC LIMIT 1
        "#,
    )
    .bind(workspace_id)
    .fetch_optional(&mut *connection)
    .await?;
    sqlx::query(
        "UPDATE workspace_local_state SET active_environment_id = ?1, updated_at = ?2 WHERE workspace_id = ?3",
    )
    .bind(fallback)
    .bind(now)
    .bind(workspace_id)
    .execute(&mut *connection)
    .await?;
    Ok(())
}
