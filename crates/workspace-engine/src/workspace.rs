mod delete_cascade;
mod external_apply;
mod layout;
mod snapshot;
mod variable_executor;
mod variable_persistence;
mod variables;

use chrono::Utc;
use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityKey, DomainEntityType, DomainMutation,
    MutationOperation,
};
use unfour_core::models::{Workspace, WorkspaceLayout, WorkspaceState};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

use self::layout::{parse_layout, StoredWorkspaceLayout};

const DEFAULT_ENVIRONMENT_TYPE: &str = "dev";
const DEFAULT_MCP_POLICY: &str = "auto";

#[derive(Clone)]
pub struct WorkspaceService {
    pub(crate) db: LocalDb,
}

impl WorkspaceService {
    pub fn new(db: LocalDb) -> Self {
        Self { db }
    }

    pub async fn ensure_default_workspace_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
    ) -> AppResult<DomainCommandResult<()>> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workspaces WHERE deleted_at IS NULL")
                .fetch_one(&mut *connection)
                .await?;
        if count > 0 {
            return Ok(DomainCommandResult::unchanged(()));
        }

        let now = Utc::now().to_rfc3339();
        let id = unfour_core::id::new_id();
        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, environment_type, mcp_policy,
              created_at, updated_at, revision
            )
            VALUES (?1, 'Default Workspace', 1, ?2, ?3, ?4, ?2, ?2, 1)
            "#,
        )
        .bind(&id)
        .bind(&now)
        .bind(DEFAULT_ENVIRONMENT_TYPE)
        .bind(DEFAULT_MCP_POLICY)
        .execute(&mut *connection)
        .await?;
        insert_workspace_companions(connection, &id, &now).await?;
        write_setting_on(connection, "active_workspace_id", &id).await?;

        Ok(DomainCommandResult::new(
            (),
            vec![workspace_mutation(
                context,
                MutationOperation::Upsert,
                &id,
                1,
            )],
        ))
    }

    pub async fn state(&self) -> AppResult<WorkspaceState> {
        let mut connection = self.db.pool().acquire().await?;
        let state = state_on(&mut connection).await?;
        if read_setting_on(&mut connection, "active_workspace_id")
            .await?
            .as_deref()
            != Some(state.active_workspace_id.as_str())
        {
            write_setting_on(
                &mut connection,
                "active_workspace_id",
                &state.active_workspace_id,
            )
            .await?;
        }
        Ok(state)
    }

    pub async fn state_read_only(&self) -> AppResult<WorkspaceState> {
        let mut connection = self.db.pool().acquire().await?;
        state_on(&mut connection).await
    }

    pub async fn list(&self) -> AppResult<Vec<Workspace>> {
        let mut connection = self.db.pool().acquire().await?;
        list_on(&mut connection).await
    }

    pub async fn create_with_options_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        name: String,
        environment_type: Option<String>,
        mcp_policy: Option<String>,
    ) -> AppResult<DomainCommandResult<Workspace>> {
        let name = normalize_name(name)?;
        assert_name_unique_on(connection, &name, None).await?;
        let environment_type = normalize_environment_type(environment_type)?;
        let mcp_policy = normalize_mcp_policy(mcp_policy)?;
        let now = Utc::now().to_rfc3339();
        let id = unfour_core::id::new_id();

        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, environment_type, mcp_policy,
              created_at, updated_at, revision
            )
            VALUES (?1, ?2, 0, ?3, ?4, ?5, ?3, ?3, 1)
            "#,
        )
        .bind(&id)
        .bind(&name)
        .bind(&now)
        .bind(&environment_type)
        .bind(&mcp_policy)
        .execute(&mut *connection)
        .await?;
        insert_workspace_companions(connection, &id, &now).await?;
        write_setting_on(connection, "active_workspace_id", &id).await?;

        let workspace = get_workspace_on(connection, &id, false).await?;
        Ok(DomainCommandResult::new(
            workspace,
            vec![workspace_mutation(
                context,
                MutationOperation::Upsert,
                &id,
                1,
            )],
        ))
    }

    pub async fn update_environment_type_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        environment_type: String,
    ) -> AppResult<DomainCommandResult<Workspace>> {
        let environment_type = normalize_environment_type(Some(environment_type))?;
        let current = get_workspace_on(connection, &workspace_id, false).await?;
        if current.environment_type == environment_type {
            return Ok(DomainCommandResult::unchanged(current));
        }
        update_workspace_field(
            connection,
            "environment_type",
            &environment_type,
            &workspace_id,
        )
        .await?;
        changed_workspace(connection, context, &workspace_id).await
    }

    pub async fn update_mcp_policy_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        mcp_policy: String,
    ) -> AppResult<DomainCommandResult<Workspace>> {
        let mcp_policy = normalize_mcp_policy(Some(mcp_policy))?;
        let current = get_workspace_on(connection, &workspace_id, false).await?;
        if current.mcp_policy == mcp_policy {
            return Ok(DomainCommandResult::unchanged(current));
        }
        update_workspace_field(connection, "mcp_policy", &mcp_policy, &workspace_id).await?;
        changed_workspace(connection, context, &workspace_id).await
    }

    pub async fn rename_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        name: String,
    ) -> AppResult<DomainCommandResult<Workspace>> {
        let name = normalize_name(name)?;
        let current = get_workspace_on(connection, &workspace_id, false).await?;
        if current.name == name {
            return Ok(DomainCommandResult::unchanged(current));
        }
        assert_name_unique_on(connection, &name, Some(&workspace_id)).await?;
        update_workspace_field(connection, "name", &name, &workspace_id).await?;
        changed_workspace(connection, context, &workspace_id).await
    }

    pub async fn set_default_on(
        &self,
        connection: &mut SqliteConnection,
        _context: &CommandContext,
        workspace_id: String,
    ) -> AppResult<DomainCommandResult<WorkspaceState>> {
        get_workspace_on(connection, &workspace_id, false).await?;
        let current: Vec<(String, bool)> = sqlx::query_as(
            "SELECT id, is_default FROM workspaces WHERE deleted_at IS NULL ORDER BY id",
        )
        .fetch_all(&mut *connection)
        .await?;
        for (id, is_default) in current {
            let next = id == workspace_id;
            if is_default == next {
                continue;
            }
            sqlx::query(
                r#"
                UPDATE workspaces
                SET is_default = ?1
                WHERE id = ?2 AND deleted_at IS NULL
                "#,
            )
            .bind(next)
            .bind(&id)
            .execute(&mut *connection)
            .await?;
        }
        let state = state_on(connection).await?;
        Ok(DomainCommandResult::unchanged(state))
    }

    pub async fn delete_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
    ) -> AppResult<DomainCommandResult<WorkspaceState>> {
        let active_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workspaces WHERE deleted_at IS NULL")
                .fetch_one(&mut *connection)
                .await?;
        if active_count <= 1 {
            return Err(AppError::Validation(
                "at least one workspace must remain".to_string(),
            ));
        }
        get_workspace_on(connection, &workspace_id, false).await?;
        let now = Utc::now().to_rfc3339();
        let mut mutations = delete_cascade::cascade_delete_workspace_children_on(
            connection,
            context,
            &workspace_id,
            &now,
        )
        .await?;
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE workspaces
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
            WHERE id = ?2 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .fetch_one(&mut *connection)
        .await?;

        let active = read_setting_on(connection, "active_workspace_id").await?;
        if active.as_deref() == Some(&workspace_id) {
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

        mutations.push(workspace_mutation(
            context,
            MutationOperation::Delete,
            &workspace_id,
            revision,
        ));
        Ok(DomainCommandResult::new(
            state_on(connection).await?,
            mutations,
        ))
    }

    pub async fn set_active_on(
        &self,
        connection: &mut SqliteConnection,
        _context: &CommandContext,
        workspace_id: String,
    ) -> AppResult<DomainCommandResult<WorkspaceState>> {
        let current = get_workspace_on(connection, &workspace_id, false).await?;
        let now = Utc::now().to_rfc3339();
        if current.last_opened_at.as_deref() != Some(now.as_str()) {
            sqlx::query(
                r#"
                UPDATE workspaces
                SET last_opened_at = ?1
                WHERE id = ?2 AND deleted_at IS NULL
                "#,
            )
            .bind(&now)
            .bind(&workspace_id)
            .execute(&mut *connection)
            .await?;
        }
        write_setting_on(connection, "active_workspace_id", &workspace_id).await?;
        Ok(DomainCommandResult::unchanged(state_on(connection).await?))
    }

    pub async fn layout(&self, workspace_id: String) -> AppResult<WorkspaceLayout> {
        self.get(&workspace_id).await?;
        let row: (String, String) = sqlx::query_as(
            r#"
            SELECT layout_json, updated_at
            FROM workspace_settings
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            "#,
        )
        .bind(&workspace_id)
        .fetch_one(self.db.pool())
        .await?;
        Ok(parse_layout(&workspace_id, &row.0, &row.1))
    }

    pub async fn update_layout(
        &self,
        workspace_id: String,
        layout: WorkspaceLayout,
    ) -> AppResult<WorkspaceLayout> {
        self.get(&workspace_id).await?;
        let stored = StoredWorkspaceLayout::try_from_layout(&workspace_id, layout)?;
        let now = Utc::now().to_rfc3339();
        let layout_json = serde_json::to_string(&stored)?;
        sqlx::query(
            r#"
            UPDATE workspace_settings
            SET layout_json = ?1, updated_at = ?2, revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(layout_json)
        .bind(&now)
        .bind(&workspace_id)
        .execute(self.db.pool())
        .await?;
        self.layout(workspace_id).await
    }

    pub(crate) async fn get(&self, workspace_id: &str) -> AppResult<Workspace> {
        let mut connection = self.db.pool().acquire().await?;
        get_workspace_on(&mut connection, workspace_id, false).await
    }
}

pub(crate) async fn get_workspace_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    include_deleted: bool,
) -> AppResult<Workspace> {
    let workspace = sqlx::query_as::<_, Workspace>(
        r#"
        SELECT
          id, name, is_default, last_opened_at, environment_type, mcp_policy,
          created_at, updated_at, deleted_at, revision
        FROM workspaces
        WHERE id = ?1 AND (?2 OR deleted_at IS NULL)
        "#,
    )
    .bind(workspace_id)
    .bind(include_deleted)
    .fetch_optional(&mut *connection)
    .await?;
    workspace.ok_or_else(|| AppError::NotFound("workspace".to_string()))
}

pub(crate) async fn list_on(connection: &mut SqliteConnection) -> AppResult<Vec<Workspace>> {
    Ok(sqlx::query_as::<_, Workspace>(
        r#"
        SELECT
          id, name, is_default, last_opened_at, environment_type, mcp_policy,
          created_at, updated_at, deleted_at, revision
        FROM workspaces
        WHERE deleted_at IS NULL
        ORDER BY is_default DESC, last_opened_at DESC, created_at ASC
        "#,
    )
    .fetch_all(&mut *connection)
    .await?)
}

pub(crate) async fn state_on(connection: &mut SqliteConnection) -> AppResult<WorkspaceState> {
    let workspaces = list_on(connection).await?;
    let stored = read_setting_on(connection, "active_workspace_id").await?;
    let active_workspace_id = stored
        .filter(|id| workspaces.iter().any(|workspace| workspace.id == *id))
        .or_else(|| workspaces.first().map(|workspace| workspace.id.clone()))
        .ok_or_else(|| AppError::NotFound("workspace".to_string()))?;
    Ok(WorkspaceState {
        active_workspace_id,
        workspaces,
    })
}

pub(crate) async fn read_setting_on(
    connection: &mut SqliteConnection,
    key: &str,
) -> AppResult<Option<String>> {
    let value: Option<String> = sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(&mut *connection)
        .await?;
    Ok(value)
}

pub(crate) async fn write_setting_on(
    connection: &mut SqliteConnection,
    key: &str,
    value: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO app_settings (key, value, updated_at)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
    )
    .bind(key)
    .bind(value)
    .bind(Utc::now().to_rfc3339())
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub(crate) async fn insert_workspace_companions(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    now: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO workspace_settings (
          workspace_id, layout_json, created_at, updated_at, revision, sync_status
        ) VALUES (?1, '{}', ?2, ?2, 1, 'local')
        "#,
    )
    .bind(workspace_id)
    .bind(now)
    .execute(&mut *connection)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO workspace_local_state (
          workspace_id, active_environment_id, created_at, updated_at
        ) VALUES (?1, NULL, ?2, ?2)
        "#,
    )
    .bind(workspace_id)
    .bind(now)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn assert_name_unique_on(
    connection: &mut SqliteConnection,
    name: &str,
    except_id: Option<&str>,
) -> AppResult<()> {
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM workspaces WHERE name COLLATE NOCASE = ?1 AND deleted_at IS NULL AND (?2 IS NULL OR id <> ?2) LIMIT 1",
    )
    .bind(name)
    .bind(except_id)
    .fetch_optional(&mut *connection)
    .await?;
    if existing.is_some() {
        return Err(AppError::Validation(format!(
            "workspace name already exists: {name}"
        )));
    }
    Ok(())
}

async fn update_workspace_field(
    connection: &mut SqliteConnection,
    field: &str,
    value: &str,
    workspace_id: &str,
) -> AppResult<()> {
    let sql = match field {
        "name" => {
            "UPDATE workspaces SET name = ?1, updated_at = ?2, revision = revision + 1 WHERE id = ?3 AND deleted_at IS NULL"
        }
        "environment_type" => {
            "UPDATE workspaces SET environment_type = ?1, updated_at = ?2, revision = revision + 1 WHERE id = ?3 AND deleted_at IS NULL"
        }
        "mcp_policy" => {
            "UPDATE workspaces SET mcp_policy = ?1, updated_at = ?2, revision = revision + 1 WHERE id = ?3 AND deleted_at IS NULL"
        }
        _ => return Err(AppError::Config("unsupported workspace field".to_string())),
    };
    let result = sqlx::query(sql)
        .bind(value)
        .bind(Utc::now().to_rfc3339())
        .bind(workspace_id)
        .execute(&mut *connection)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("workspace".to_string()));
    }
    Ok(())
}

async fn changed_workspace(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    workspace_id: &str,
) -> AppResult<DomainCommandResult<Workspace>> {
    let workspace = get_workspace_on(connection, workspace_id, false).await?;
    Ok(DomainCommandResult::new(
        workspace.clone(),
        vec![workspace_mutation(
            context,
            MutationOperation::Upsert,
            workspace_id,
            workspace.revision,
        )],
    ))
}

pub(crate) fn workspace_mutation(
    context: &CommandContext,
    operation: MutationOperation,
    workspace_id: &str,
    revision: i64,
) -> DomainMutation {
    DomainMutation::new(
        context.origin,
        operation,
        DomainEntityKey::new(DomainEntityType::Workspace, workspace_id, workspace_id),
        revision,
    )
}

pub(crate) fn normalize_name(name: String) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "workspace name cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 80 {
        return Err(AppError::Validation(
            "workspace name must be 80 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_environment_type(value: Option<String>) -> AppResult<String> {
    let value = value
        .and_then(|item| {
            let trimmed = item.trim().to_ascii_lowercase();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .unwrap_or_else(|| DEFAULT_ENVIRONMENT_TYPE.to_string());
    if matches!(value.as_str(), "dev" | "test" | "prod") {
        Ok(value)
    } else {
        Err(AppError::Validation(
            "workspace environment_type must be one of: dev, test, prod".to_string(),
        ))
    }
}

pub(crate) fn normalize_mcp_policy(value: Option<String>) -> AppResult<String> {
    let value = value
        .and_then(|item| {
            let trimmed = item.trim().to_ascii_lowercase();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .unwrap_or_else(|| DEFAULT_MCP_POLICY.to_string());
    if matches!(
        value.as_str(),
        "auto" | "disabled" | "read_only" | "guarded" | "full_access"
    ) {
        Ok(value)
    } else {
        Err(AppError::Validation(
            "workspace mcp_policy must be one of: auto, disabled, read_only, guarded, full_access"
                .to_string(),
        ))
    }
}

#[cfg(test)]
#[path = "workspace_tests/mod.rs"]
mod workspace_tests;
