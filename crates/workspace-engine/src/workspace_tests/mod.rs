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
    assert_eq!(created.environment_type, "dev");
    assert_eq!(created.mcp_policy, "auto");
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
async fn create_with_options_stores_environment_and_policy() {
    let service = service().await;

    let created = service
        .create_with_options(
            "Production".to_string(),
            Some("prod".to_string()),
            Some("read_only".to_string()),
        )
        .await
        .expect("create prod workspace");

    assert_eq!(created.environment_type, "prod");
    assert_eq!(created.mcp_policy, "read_only");
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
