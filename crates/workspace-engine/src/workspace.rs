use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use unfour_core::models::{
    KeyValue, Workspace, WorkspaceEnvironment, WorkspaceLayout, WorkspaceLayoutTab, WorkspaceState,
};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use uuid::Uuid;

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
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, 'Default Workspace', 1, ?2, ?2, ?2, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&now)
        .execute(self.db.pool())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO workspace_settings (
              workspace_id, layout_json, env_json, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, '{}', '{}', ?2, ?2, 1, 'local')
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
              id, name, is_default, last_opened_at, created_at, updated_at,
              deleted_at, revision, sync_status, remote_id
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
        let name = normalize_name(name)?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, ?2, 0, ?3, ?3, ?3, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&name)
        .bind(&now)
        .execute(self.db.pool())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO workspace_settings (
              workspace_id, layout_json, env_json, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, '{}', '{}', ?2, ?2, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&now)
        .execute(self.db.pool())
        .await?;

        self.write_setting("active_workspace_id", &id).await?;
        self.get(&id).await
    }

    pub async fn rename(&self, workspace_id: String, name: String) -> AppResult<Workspace> {
        let name = normalize_name(name)?;
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

    pub async fn environment(&self, workspace_id: String) -> AppResult<WorkspaceEnvironment> {
        self.get(&workspace_id).await?;

        let row: (String, String) = sqlx::query_as(
            r#"
            SELECT env_json, updated_at
            FROM workspace_settings
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            "#,
        )
        .bind(&workspace_id)
        .fetch_one(self.db.pool())
        .await?;

        let variables = serde_json::from_str::<Vec<KeyValue>>(&row.0).unwrap_or_default();

        Ok(WorkspaceEnvironment {
            workspace_id,
            variables,
            updated_at: row.1,
        })
    }

    pub async fn update_environment(
        &self,
        workspace_id: String,
        variables: Vec<KeyValue>,
    ) -> AppResult<WorkspaceEnvironment> {
        self.get(&workspace_id).await?;
        validate_environment(&variables)?;

        let now = Utc::now().to_rfc3339();
        let env_json = serde_json::to_string(&variables)?;

        sqlx::query(
            r#"
            UPDATE workspace_settings
            SET env_json = ?1, updated_at = ?2, revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(env_json)
        .bind(&now)
        .bind(&workspace_id)
        .execute(self.db.pool())
        .await?;

        self.environment(workspace_id).await
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
              id, name, is_default, last_opened_at, created_at, updated_at,
              deleted_at, revision, sync_status, remote_id
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

fn validate_environment(variables: &[KeyValue]) -> AppResult<()> {
    let mut seen = HashSet::new();
    for variable in variables {
        let key = variable.key.trim();
        if key.is_empty() {
            continue;
        }
        let valid = key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'));
        if !valid {
            return Err(AppError::Validation(format!(
                "invalid environment variable name: {}",
                variable.key
            )));
        }
        if variable.enabled && !seen.insert(key.to_ascii_lowercase()) {
            return Err(AppError::Validation(format!(
                "duplicate environment variable name: {}",
                variable.key
            )));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredWorkspaceLayout {
    sidebar_collapsed: bool,
    active_tab_id: String,
    tabs: Vec<WorkspaceLayoutTab>,
    selected_api_request_id: Option<String>,
    selected_database_connection_id: Option<String>,
    selected_ssh_connection_id: Option<String>,
}

impl StoredWorkspaceLayout {
    fn try_from_layout(workspace_id: &str, layout: WorkspaceLayout) -> AppResult<Self> {
        if layout.workspace_id != workspace_id {
            return Err(AppError::Validation(
                "layout workspace_id does not match command workspace_id".to_string(),
            ));
        }

        validate_layout_tabs(&layout.active_tab_id, &layout.tabs)?;

        Ok(Self {
            sidebar_collapsed: layout.sidebar_collapsed,
            active_tab_id: layout.active_tab_id,
            tabs: layout.tabs,
            selected_api_request_id: non_empty_optional(layout.selected_api_request_id),
            selected_database_connection_id: non_empty_optional(
                layout.selected_database_connection_id,
            ),
            selected_ssh_connection_id: non_empty_optional(layout.selected_ssh_connection_id),
        })
    }
}

fn parse_layout(workspace_id: &str, value: &str, updated_at: &str) -> WorkspaceLayout {
    let stored = serde_json::from_str::<StoredWorkspaceLayout>(value).unwrap_or_else(|_| {
        StoredWorkspaceLayout {
            sidebar_collapsed: false,
            active_tab_id: "api-main".to_string(),
            tabs: default_layout_tabs(),
            selected_api_request_id: None,
            selected_database_connection_id: None,
            selected_ssh_connection_id: None,
        }
    });

    let mut tabs = stored.tabs;
    if validate_layout_tabs(&stored.active_tab_id, &tabs).is_err() {
        tabs = default_layout_tabs();
    }
    let active_tab_id = if tabs.iter().any(|tab| tab.id == stored.active_tab_id) {
        stored.active_tab_id
    } else {
        "api-main".to_string()
    };

    WorkspaceLayout {
        workspace_id: workspace_id.to_string(),
        sidebar_collapsed: stored.sidebar_collapsed,
        active_tab_id,
        tabs,
        selected_api_request_id: stored.selected_api_request_id,
        selected_database_connection_id: stored.selected_database_connection_id,
        selected_ssh_connection_id: stored.selected_ssh_connection_id,
        updated_at: updated_at.to_string(),
    }
}

fn validate_layout_tabs(active_tab_id: &str, tabs: &[WorkspaceLayoutTab]) -> AppResult<()> {
    if tabs.is_empty() {
        return Err(AppError::Validation(
            "layout must include at least one tab".to_string(),
        ));
    }
    if active_tab_id.trim().is_empty() {
        return Err(AppError::Validation(
            "layout active_tab_id cannot be empty".to_string(),
        ));
    }

    for tab in tabs {
        if tab.id.trim().is_empty() || tab.title.trim().is_empty() {
            return Err(AppError::Validation(
                "layout tabs must have non-empty id and title".to_string(),
            ));
        }
        if !matches!(tab.kind.as_str(), "api" | "ssh" | "database") {
            return Err(AppError::Validation(format!(
                "unsupported layout tab kind: {}",
                tab.kind
            )));
        }
    }

    if !tabs.iter().any(|tab| tab.id == active_tab_id) {
        return Err(AppError::Validation(
            "layout active_tab_id must reference an open tab".to_string(),
        ));
    }

    Ok(())
}

fn default_layout_tabs() -> Vec<WorkspaceLayoutTab> {
    vec![
        WorkspaceLayoutTab {
            id: "api-main".to_string(),
            title: "API Client".to_string(),
            kind: "api".to_string(),
        },
        WorkspaceLayoutTab {
            id: "ssh-main".to_string(),
            title: "SSH Terminal".to_string(),
            kind: "ssh".to_string(),
        },
        WorkspaceLayoutTab {
            id: "database-main".to_string(),
            title: "Database".to_string(),
            kind: "database".to_string(),
        },
    ]
}

fn non_empty_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn service() -> WorkspaceService {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory sqlite");
        let db = LocalDb::from_pool(pool);
        db.migrate().await.expect("run migrations");
        let service = WorkspaceService::new(db);
        service
            .ensure_default_workspace()
            .await
            .expect("ensure default workspace");
        service
    }

    #[tokio::test]
    async fn layout_returns_defaults_for_new_workspace() {
        let service = service().await;
        let state = service.state().await.expect("workspace state");

        let layout = service
            .layout(state.active_workspace_id)
            .await
            .expect("workspace layout");

        assert_eq!(layout.active_tab_id, "api-main");
        assert!(!layout.sidebar_collapsed);
        assert_eq!(layout.tabs.len(), 3);
        assert!(layout
            .tabs
            .iter()
            .any(|tab| tab.id == "database-main" && tab.kind == "database"));
    }

    #[tokio::test]
    async fn layout_update_persists_valid_layout() {
        let service = service().await;
        let state = service.state().await.expect("workspace state");
        let workspace_id = state.active_workspace_id;
        let mut layout = service
            .layout(workspace_id.clone())
            .await
            .expect("workspace layout");
        layout.sidebar_collapsed = true;
        layout.active_tab_id = "database-main".to_string();
        layout.selected_database_connection_id = Some("db-1".to_string());

        let updated = service
            .update_layout(workspace_id.clone(), layout)
            .await
            .expect("update layout");
        let loaded = service.layout(workspace_id).await.expect("reload layout");

        assert!(updated.sidebar_collapsed);
        assert_eq!(loaded.active_tab_id, "database-main");
        assert_eq!(
            loaded.selected_database_connection_id.as_deref(),
            Some("db-1")
        );
    }

    #[tokio::test]
    async fn layout_update_rejects_active_tab_outside_open_tabs() {
        let service = service().await;
        let state = service.state().await.expect("workspace state");
        let workspace_id = state.active_workspace_id;
        let mut layout = service
            .layout(workspace_id.clone())
            .await
            .expect("workspace layout");
        layout.active_tab_id = "missing-tab".to_string();

        let result = service.update_layout(workspace_id, layout).await;

        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[tokio::test]
    async fn environment_update_rejects_duplicate_enabled_names() {
        let service = service().await;
        let state = service.state().await.expect("workspace state");

        let result = service
            .update_environment(
                state.active_workspace_id,
                vec![
                    KeyValue {
                        key: "base_url".to_string(),
                        value: "https://example.test".to_string(),
                        enabled: true,
                    },
                    KeyValue {
                        key: "BASE_URL".to_string(),
                        value: "https://other.test".to_string(),
                        enabled: true,
                    },
                ],
            )
            .await;

        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[tokio::test]
    async fn create_switch_rename_and_delete_preserve_active_workspace() {
        let service = service().await;
        let default_id = service
            .state()
            .await
            .expect("workspace state")
            .active_workspace_id;

        let created = service
            .create("  Client Ops  ".to_string())
            .await
            .expect("create workspace");
        let created_state = service.state().await.expect("state after create");
        assert_eq!(created.name, "Client Ops");
        assert_eq!(created_state.active_workspace_id, created.id);

        let renamed = service
            .rename(created.id.clone(), "Client Ops EU".to_string())
            .await
            .expect("rename workspace");
        assert_eq!(renamed.name, "Client Ops EU");
        assert_eq!(renamed.sync_status, "pending");

        let switched = service
            .set_active(default_id.clone())
            .await
            .expect("switch active workspace");
        assert_eq!(switched.active_workspace_id, default_id);

        let deleted = service
            .delete(created.id.clone())
            .await
            .expect("delete inactive workspace");
        assert_eq!(deleted.active_workspace_id, default_id);
        assert!(!deleted
            .workspaces
            .iter()
            .any(|workspace| workspace.id == created.id));
    }

    #[tokio::test]
    async fn delete_active_workspace_falls_back_to_default() {
        let service = service().await;
        let default_id = service
            .state()
            .await
            .expect("workspace state")
            .active_workspace_id;
        let created = service
            .create("Scratch".to_string())
            .await
            .expect("create workspace");

        let state = service
            .delete(created.id)
            .await
            .expect("delete active workspace");

        assert_eq!(state.active_workspace_id, default_id);
        assert_eq!(state.workspaces.len(), 1);
    }

    #[tokio::test]
    async fn environment_update_persists_valid_variables() {
        let service = service().await;
        let workspace_id = service
            .state()
            .await
            .expect("workspace state")
            .active_workspace_id;

        let environment = service
            .update_environment(
                workspace_id.clone(),
                vec![
                    KeyValue {
                        key: "base_url".to_string(),
                        value: "https://api.example.test".to_string(),
                        enabled: true,
                    },
                    KeyValue {
                        key: "disabled_duplicate_is_allowed".to_string(),
                        value: "ignored".to_string(),
                        enabled: false,
                    },
                    KeyValue {
                        key: "DISABLED_DUPLICATE_IS_ALLOWED".to_string(),
                        value: "also ignored".to_string(),
                        enabled: false,
                    },
                ],
            )
            .await
            .expect("update environment");
        let loaded = service
            .environment(workspace_id)
            .await
            .expect("load environment");

        assert_eq!(environment.variables.len(), 3);
        assert_eq!(loaded.variables[0].key, "base_url");
        assert_eq!(loaded.variables[0].value, "https://api.example.test");
    }

    #[tokio::test]
    async fn environment_update_rejects_invalid_variable_names() {
        let service = service().await;
        let state = service.state().await.expect("workspace state");

        let result = service
            .update_environment(
                state.active_workspace_id,
                vec![KeyValue {
                    key: "bad name".to_string(),
                    value: "nope".to_string(),
                    enabled: true,
                }],
            )
            .await;

        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[tokio::test]
    async fn state_read_only_does_not_write_fallback_active_workspace() {
        let service = service().await;
        sqlx::query("DELETE FROM app_settings WHERE key = 'active_workspace_id'")
            .execute(service.db.pool())
            .await
            .expect("remove active workspace setting");

        let state = service
            .state_read_only()
            .await
            .expect("read-only state should work");

        assert!(!state.active_workspace_id.is_empty());
        assert_eq!(
            service
                .read_setting("active_workspace_id")
                .await
                .expect("read active workspace setting"),
            None
        );
    }
}
