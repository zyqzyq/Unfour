mod layout;

use chrono::Utc;
use unfour_core::models::{Workspace, WorkspaceLayout, WorkspaceState};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

use self::layout::{parse_layout, StoredWorkspaceLayout};

const DEFAULT_ENVIRONMENT_TYPE: &str = "dev";
const DEFAULT_MCP_POLICY: &str = "auto";

#[derive(Clone)]
pub struct WorkspaceService {
    db: LocalDb,
}

impl WorkspaceService {
    pub fn new(db: LocalDb) -> Self {
        Self { db }
    }

    pub async fn ensure_default_workspace(&self) -> AppResult<()> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM workspaces WHERE deleted_at IS NULL")
                .fetch_one(self.db.pool())
                .await?;

        if count.0 > 0 {
            return Ok(());
        }

        let now = Utc::now().to_rfc3339();
        let id = unfour_core::id::new_id();

        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, environment_type, mcp_policy,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, 'Default Workspace', 1, ?2, ?3, ?4, ?2, ?2, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&now)
        .bind(DEFAULT_ENVIRONMENT_TYPE)
        .bind(DEFAULT_MCP_POLICY)
        .execute(self.db.pool())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO workspace_settings (
              workspace_id, layout_json, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, '{}', ?2, ?2, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&now)
        .execute(self.db.pool())
        .await?;

        self.write_setting("active_workspace_id", &id).await
    }

    pub async fn state(&self) -> AppResult<WorkspaceState> {
        let workspaces = self.list().await?;
        let active_workspace_id = self.active_workspace_id(&workspaces).await?;

        Ok(WorkspaceState {
            active_workspace_id,
            workspaces,
        })
    }

    pub async fn state_read_only(&self) -> AppResult<WorkspaceState> {
        let workspaces = self.list().await?;
        let active_workspace_id = self.active_workspace_id_read_only(&workspaces).await?;

        Ok(WorkspaceState {
            active_workspace_id,
            workspaces,
        })
    }

    pub async fn list(&self) -> AppResult<Vec<Workspace>> {
        let items = sqlx::query_as::<_, Workspace>(
            r#"
            SELECT
              id, name, is_default, last_opened_at, environment_type, mcp_policy,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
            FROM workspaces
            WHERE deleted_at IS NULL
            ORDER BY is_default DESC, last_opened_at DESC, created_at ASC
            "#,
        )
        .fetch_all(self.db.pool())
        .await?;

        Ok(items)
    }

    pub async fn create(&self, name: String) -> AppResult<Workspace> {
        self.create_with_options(name, None, None).await
    }

    pub async fn create_with_options(
        &self,
        name: String,
        environment_type: Option<String>,
        mcp_policy: Option<String>,
    ) -> AppResult<Workspace> {
        let name = normalize_name(name)?;
        self.assert_name_unique(&name, None).await?;
        let environment_type = normalize_environment_type(environment_type)?;
        let mcp_policy = normalize_mcp_policy(mcp_policy)?;
        let now = Utc::now().to_rfc3339();
        let id = unfour_core::id::new_id();

        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, environment_type, mcp_policy,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, 0, ?3, ?4, ?5, ?3, ?3, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&name)
        .bind(&now)
        .bind(&environment_type)
        .bind(&mcp_policy)
        .execute(self.db.pool())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO workspace_settings (
              workspace_id, layout_json, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, '{}', ?2, ?2, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&now)
        .execute(self.db.pool())
        .await?;

        self.write_setting("active_workspace_id", &id).await?;
        self.get(&id).await
    }

    pub async fn update_environment(
        &self,
        workspace_id: String,
        environment_type: String,
    ) -> AppResult<Workspace> {
        let environment_type = normalize_environment_type(Some(environment_type))?;
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE workspaces
            SET environment_type = ?1, updated_at = ?2, revision = revision + 1, sync_status = 'pending'
            WHERE id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(environment_type)
        .bind(now)
        .bind(&workspace_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("workspace".to_string()));
        }

        self.get(&workspace_id).await
    }

    pub async fn rename(&self, workspace_id: String, name: String) -> AppResult<Workspace> {
        let name = normalize_name(name)?;
        self.assert_name_unique(&name, Some(&workspace_id)).await?;
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE workspaces
            SET name = ?1, updated_at = ?2, revision = revision + 1, sync_status = 'pending'
            WHERE id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(name)
        .bind(now)
        .bind(&workspace_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("workspace".to_string()));
        }

        self.get(&workspace_id).await
    }

    pub async fn delete(&self, workspace_id: String) -> AppResult<WorkspaceState> {
        let active_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM workspaces WHERE deleted_at IS NULL")
                .fetch_one(self.db.pool())
                .await?;

        if active_count.0 <= 1 {
            return Err(AppError::Validation(
                "at least one workspace must remain".to_string(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE workspaces
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("workspace".to_string()));
        }

        let active = self.read_setting("active_workspace_id").await?;
        if active.as_deref() == Some(&workspace_id) {
            let next: (String,) = sqlx::query_as(
                r#"
                SELECT id FROM workspaces
                WHERE deleted_at IS NULL
                ORDER BY is_default DESC, updated_at DESC
                LIMIT 1
                "#,
            )
            .fetch_one(self.db.pool())
            .await?;
            self.write_setting("active_workspace_id", &next.0).await?;
        }

        self.state().await
    }

    pub async fn set_active(&self, workspace_id: String) -> AppResult<WorkspaceState> {
        self.get(&workspace_id).await?;
        let now = Utc::now().to_rfc3339();

        sqlx::query("UPDATE workspaces SET last_opened_at = ?1, updated_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(&workspace_id)
            .execute(self.db.pool())
            .await?;

        self.write_setting("active_workspace_id", &workspace_id)
            .await?;
        self.state().await
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

    async fn get(&self, workspace_id: &str) -> AppResult<Workspace> {
        let workspace = sqlx::query_as::<_, Workspace>(
            r#"
            SELECT
              id, name, is_default, last_opened_at, environment_type, mcp_policy,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
            FROM workspaces
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(self.db.pool())
        .await?;

        workspace.ok_or_else(|| AppError::NotFound("workspace".to_string()))
    }

    async fn active_workspace_id(&self, workspaces: &[Workspace]) -> AppResult<String> {
        let stored = self.read_setting("active_workspace_id").await?;
        if let Some(id) = stored {
            if workspaces.iter().any(|workspace| workspace.id == id) {
                return Ok(id);
            }
        }

        let fallback = workspaces
            .first()
            .ok_or_else(|| AppError::NotFound("workspace".to_string()))?;
        self.write_setting("active_workspace_id", &fallback.id)
            .await?;
        Ok(fallback.id.clone())
    }

    async fn active_workspace_id_read_only(&self, workspaces: &[Workspace]) -> AppResult<String> {
        let stored = self.read_setting("active_workspace_id").await?;
        if let Some(id) = stored {
            if workspaces.iter().any(|workspace| workspace.id == id) {
                return Ok(id);
            }
        }

        workspaces
            .first()
            .map(|workspace| workspace.id.clone())
            .ok_or_else(|| AppError::NotFound("workspace".to_string()))
    }

    async fn read_setting(&self, key: &str) -> AppResult<Option<String>> {
        let value: Option<(String,)> =
            sqlx::query_as("SELECT value FROM app_settings WHERE key = ?1")
                .bind(key)
                .fetch_optional(self.db.pool())
                .await?;

        Ok(value.map(|item| item.0))
    }

    async fn write_setting(&self, key: &str, value: &str) -> AppResult<()> {
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
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    async fn assert_name_unique(&self, name: &str, except_id: Option<&str>) -> AppResult<()> {
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM workspaces WHERE name COLLATE NOCASE = ?1 AND deleted_at IS NULL AND (?2 IS NULL OR id <> ?2) LIMIT 1",
        )
        .bind(name)
        .bind(except_id)
        .fetch_optional(self.db.pool())
        .await?;

        if existing.is_some() {
            return Err(AppError::Validation(format!(
                "workspace name already exists: {name}"
            )));
        }
        Ok(())
    }
}

fn normalize_name(name: String) -> AppResult<String> {
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

fn normalize_environment_type(value: Option<String>) -> AppResult<String> {
    let value = value
        .and_then(|item| {
            let trimmed = item.trim().to_ascii_lowercase();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
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

fn normalize_mcp_policy(value: Option<String>) -> AppResult<String> {
    let value = value
        .and_then(|item| {
            let trimmed = item.trim().to_ascii_lowercase();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
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
