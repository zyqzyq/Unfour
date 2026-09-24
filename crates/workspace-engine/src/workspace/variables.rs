use std::collections::{HashMap, HashSet};

use sqlx::SqliteConnection;
use unfour_core::models::{
    KeyValue, WorkspaceEnvironment, WorkspaceEnvironmentVariable, WorkspaceVariable,
    WorkspaceVariableInput,
};
use unfour_core::{AppError, AppResult};

use super::{get_workspace_on, WorkspaceService};

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspaceEnvironmentRow {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
}

impl WorkspaceService {
    pub async fn list_variables(&self, workspace_id: String) -> AppResult<Vec<WorkspaceVariable>> {
        let mut connection = self.db.pool().acquire().await?;
        get_workspace_on(&mut connection, &workspace_id, false).await?;
        list_variables_on(&mut connection, &workspace_id, false).await
    }

    pub async fn list_environments(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<WorkspaceEnvironment>> {
        let mut connection = self.db.pool().acquire().await?;
        get_workspace_on(&mut connection, &workspace_id, false).await?;
        list_environments_on(&mut connection, &workspace_id).await
    }

    pub async fn active_environment_id(&self, workspace_id: &str) -> AppResult<Option<String>> {
        let mut connection = self.db.pool().acquire().await?;
        get_workspace_on(&mut connection, workspace_id, false).await?;
        active_environment_id_on(&mut connection, workspace_id).await
    }

    /// Resolve `{{VARIABLE}}` tokens for one workspace and an explicitly
    /// supplied active environment. Environment values overlay workspace
    /// values; an environment from another workspace is rejected before any
    /// of its variables are read.
    pub async fn resolve_variables(
        &self,
        workspace_id: &str,
        active_environment_id: Option<&str>,
        input: &str,
    ) -> AppResult<String> {
        self.resolve_variables_with_overrides(workspace_id, active_environment_id, input, &[])
            .await
    }

    /// Resolve `{{VARIABLE}}` tokens with request-local values taking
    /// precedence over environment and workspace values.
    pub async fn resolve_variables_with_overrides(
        &self,
        workspace_id: &str,
        active_environment_id: Option<&str>,
        input: &str,
        overrides: &[KeyValue],
    ) -> AppResult<String> {
        let mut connection = self.db.pool().acquire().await?;
        get_workspace_on(&mut connection, workspace_id, false).await?;
        let mut values = HashMap::new();
        for variable in list_variables_on(&mut connection, workspace_id, false)
            .await?
            .into_iter()
            .filter(|variable| variable.is_enabled)
        {
            values.insert(variable.key, variable.value);
        }

        if let Some(environment_id) = active_environment_id.filter(|id| !id.trim().is_empty()) {
            get_environment_on(&mut connection, workspace_id, environment_id, false).await?;
            for variable in
                list_environment_variables_on(&mut connection, workspace_id, environment_id, false)
                    .await?
                    .into_iter()
                    .filter(|variable| variable.is_enabled)
            {
                values.insert(variable.key, variable.value);
            }
        }

        for variable in overrides.iter().filter(|variable| variable.enabled) {
            values.insert(variable.key.clone(), variable.value.clone());
        }

        resolve_template(input, &values)
    }
}

pub(crate) async fn list_variables_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    include_deleted: bool,
) -> AppResult<Vec<WorkspaceVariable>> {
    Ok(sqlx::query_as::<_, WorkspaceVariable>(
        r#"
        SELECT
          id, workspace_id, key, value, is_secret, is_enabled, description,
          sort_order, created_at, updated_at, deleted_at, revision
        FROM workspace_variables
        WHERE workspace_id = ?1 AND (?2 OR deleted_at IS NULL)
        ORDER BY sort_order ASC, created_at ASC, id ASC
        "#,
    )
    .bind(workspace_id)
    .bind(include_deleted)
    .fetch_all(&mut *connection)
    .await?)
}

pub(crate) async fn get_variable_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    variable_id: &str,
    include_deleted: bool,
) -> AppResult<WorkspaceVariable> {
    sqlx::query_as::<_, WorkspaceVariable>(
        r#"
        SELECT
          id, workspace_id, key, value, is_secret, is_enabled, description,
          sort_order, created_at, updated_at, deleted_at, revision
        FROM workspace_variables
        WHERE id = ?1 AND workspace_id = ?2 AND (?3 OR deleted_at IS NULL)
        "#,
    )
    .bind(variable_id)
    .bind(workspace_id)
    .bind(include_deleted)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::NotFound("workspace variable".to_string()))
}

pub(crate) async fn list_environments_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
) -> AppResult<Vec<WorkspaceEnvironment>> {
    let active_environment_id = active_environment_id_on(connection, workspace_id).await?;
    let rows = sqlx::query_as::<_, WorkspaceEnvironmentRow>(
        r#"
        SELECT id, workspace_id, name, sort_order, created_at, updated_at,
               deleted_at, revision
        FROM workspace_environments
        WHERE workspace_id = ?1 AND deleted_at IS NULL
        ORDER BY sort_order ASC, created_at ASC, id ASC
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&mut *connection)
    .await?;

    let mut environments = Vec::with_capacity(rows.len());
    for row in rows {
        let variables =
            list_environment_variables_on(connection, workspace_id, &row.id, false).await?;
        environments.push(environment_from_row(
            row,
            variables,
            active_environment_id.as_deref(),
        ));
    }
    Ok(environments)
}

pub(crate) async fn get_environment_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    environment_id: &str,
    include_deleted: bool,
) -> AppResult<WorkspaceEnvironment> {
    let row = sqlx::query_as::<_, WorkspaceEnvironmentRow>(
        r#"
        SELECT id, workspace_id, name, sort_order, created_at, updated_at,
               deleted_at, revision
        FROM workspace_environments
        WHERE id = ?1 AND workspace_id = ?2 AND (?3 OR deleted_at IS NULL)
        "#,
    )
    .bind(environment_id)
    .bind(workspace_id)
    .bind(include_deleted)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::NotFound("workspace environment".to_string()))?;
    let variables =
        list_environment_variables_on(connection, workspace_id, environment_id, include_deleted)
            .await?;
    let active = active_environment_id_on(connection, workspace_id).await?;
    Ok(environment_from_row(row, variables, active.as_deref()))
}

pub(crate) async fn list_environment_variables_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    environment_id: &str,
    include_deleted: bool,
) -> AppResult<Vec<WorkspaceEnvironmentVariable>> {
    Ok(sqlx::query_as::<_, WorkspaceEnvironmentVariable>(
        r#"
        SELECT
          id, workspace_id, environment_id, key, value, is_secret,
          is_enabled, description, sort_order, created_at, updated_at,
          deleted_at, revision
        FROM workspace_environment_variables
        WHERE workspace_id = ?1 AND environment_id = ?2
          AND (?3 OR deleted_at IS NULL)
        ORDER BY sort_order ASC, created_at ASC, id ASC
        "#,
    )
    .bind(workspace_id)
    .bind(environment_id)
    .bind(include_deleted)
    .fetch_all(&mut *connection)
    .await?)
}

pub(crate) async fn get_environment_variable_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    environment_id: &str,
    variable_id: &str,
    include_deleted: bool,
) -> AppResult<WorkspaceEnvironmentVariable> {
    sqlx::query_as::<_, WorkspaceEnvironmentVariable>(
        r#"
        SELECT
          id, workspace_id, environment_id, key, value, is_secret,
          is_enabled, description, sort_order, created_at, updated_at,
          deleted_at, revision
        FROM workspace_environment_variables
        WHERE id = ?1 AND workspace_id = ?2 AND environment_id = ?3
          AND (?4 OR deleted_at IS NULL)
        "#,
    )
    .bind(variable_id)
    .bind(workspace_id)
    .bind(environment_id)
    .bind(include_deleted)
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| AppError::NotFound("workspace environment variable".to_string()))
}

pub(crate) async fn active_environment_id_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
) -> AppResult<Option<String>> {
    let active: Option<String> = sqlx::query_scalar(
        r#"
        SELECT environment.id
        FROM workspace_local_state AS settings
        JOIN workspace_environments AS environment
          ON environment.id = settings.active_environment_id
         AND environment.workspace_id = settings.workspace_id
         AND environment.deleted_at IS NULL
        WHERE settings.workspace_id = ?1
        "#,
    )
    .bind(workspace_id)
    .fetch_optional(&mut *connection)
    .await?;
    Ok(active)
}

pub(crate) async fn assert_environment_name_unique_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    name: &str,
    exclude_id: Option<&str>,
) -> AppResult<()> {
    let existing: Option<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM workspace_environments
        WHERE workspace_id = ?1 AND name COLLATE NOCASE = ?2
          AND deleted_at IS NULL AND (?3 IS NULL OR id <> ?3)
        LIMIT 1
        "#,
    )
    .bind(workspace_id)
    .bind(name)
    .bind(exclude_id)
    .fetch_optional(&mut *connection)
    .await?;
    if existing.is_some() {
        return Err(AppError::Validation(format!(
            "environment name already exists in this workspace: {name}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_variables(variables: &[WorkspaceVariableInput]) -> AppResult<()> {
    let mut keys = HashSet::new();
    let mut ids = HashSet::new();
    for variable in variables {
        let key = variable.key.trim();
        if key.is_empty() {
            return Err(AppError::Validation(
                "variable key cannot be empty".to_string(),
            ));
        }
        if key.chars().count() > 120 {
            return Err(AppError::Validation(
                "variable key must be 120 characters or fewer".to_string(),
            ));
        }
        let normalized = key.to_ascii_lowercase();
        if !keys.insert(normalized) {
            return Err(AppError::Validation(format!(
                "duplicate workspace variable key: {key}"
            )));
        }
        if let Some(id) = non_empty_optional(variable.id.clone()) {
            if !ids.insert(id) {
                return Err(AppError::Validation("duplicate variable id".to_string()));
            }
        }
    }
    Ok(())
}

pub(crate) const MAX_ENVIRONMENT_NAME_CHARS: usize = 80;

pub(crate) fn normalize_environment_name(name: String) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation(
            "environment name cannot be empty".to_string(),
        ));
    }
    if name.chars().count() > MAX_ENVIRONMENT_NAME_CHARS {
        return Err(AppError::Validation(format!(
            "environment name must be {MAX_ENVIRONMENT_NAME_CHARS} characters or fewer"
        )));
    }
    Ok(name.to_string())
}

pub(crate) fn normalize_description(description: Option<String>) -> Option<String> {
    description.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

pub(crate) fn non_empty_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn environment_from_row(
    row: WorkspaceEnvironmentRow,
    variables: Vec<WorkspaceEnvironmentVariable>,
    active_environment_id: Option<&str>,
) -> WorkspaceEnvironment {
    WorkspaceEnvironment {
        is_active: active_environment_id == Some(row.id.as_str()),
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        sort_order: row.sort_order,
        variables,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
        revision: row.revision,
    }
}

fn resolve_template(input: &str, values: &HashMap<String, String>) -> AppResult<String> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            output.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let key = after_start[..end].trim();
        if key.is_empty() {
            output.push_str("{{}}");
        } else if let Some(value) = values.get(key) {
            output.push_str(value);
        } else {
            return Err(AppError::Validation(format!("unresolved variable: {key}")));
        }
        rest = &after_start[end + 2..];
    }
    output.push_str(rest);
    Ok(output)
}
