use crate::app_error::AppResult;
use crate::models::{
    ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiResponse, ApiSavedRequest,
    DatabaseBrowseInput, DatabaseBrowseResult, DatabaseConnection, DatabaseConnectionInput,
    DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema, DatabaseTestResult, KeyValue,
    SystemHealth, Workspace, WorkspaceEnvironment, WorkspaceLayout, WorkspaceState,
};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn system_health(state: State<'_, AppState>) -> AppResult<SystemHealth> {
    state.command_bus.system_health().await
}

#[tauri::command]
pub async fn workspace_list(state: State<'_, AppState>) -> AppResult<WorkspaceState> {
    state.command_bus.list_workspaces().await
}

#[tauri::command]
pub async fn workspace_create(name: String, state: State<'_, AppState>) -> AppResult<Workspace> {
    state.command_bus.create_workspace(name).await
}

#[tauri::command]
pub async fn workspace_rename(
    workspace_id: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<Workspace> {
    state.command_bus.rename_workspace(workspace_id, name).await
}

#[tauri::command]
pub async fn workspace_delete(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceState> {
    state.command_bus.delete_workspace(workspace_id).await
}

#[tauri::command]
pub async fn workspace_set_active(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceState> {
    state.command_bus.set_active_workspace(workspace_id).await
}

#[tauri::command]
pub async fn workspace_environment_get(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceEnvironment> {
    state.command_bus.workspace_environment(workspace_id).await
}

#[tauri::command]
pub async fn workspace_environment_update(
    workspace_id: String,
    variables: Vec<KeyValue>,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceEnvironment> {
    state
        .command_bus
        .workspace_environment_update(workspace_id, variables)
        .await
}

#[tauri::command]
pub async fn workspace_layout_get(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceLayout> {
    state.command_bus.workspace_layout(workspace_id).await
}

#[tauri::command]
pub async fn workspace_layout_update(
    workspace_id: String,
    layout: WorkspaceLayout,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceLayout> {
    state
        .command_bus
        .workspace_layout_update(workspace_id, layout)
        .await
}

#[tauri::command]
pub async fn api_send_request(
    input: ApiRequestInput,
    state: State<'_, AppState>,
) -> AppResult<ApiResponse> {
    state.command_bus.send_api_request(input).await
}

#[tauri::command]
pub async fn api_history_list(
    workspace_id: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiHistoryItem>> {
    state
        .command_bus
        .list_api_history(workspace_id, limit)
        .await
}

#[tauri::command]
pub async fn api_history_detail(
    workspace_id: String,
    history_id: String,
    state: State<'_, AppState>,
) -> AppResult<ApiHistoryDetail> {
    state
        .command_bus
        .api_history_detail(workspace_id, history_id)
        .await
}

#[tauri::command]
pub async fn api_request_save(
    input: ApiRequestInput,
    state: State<'_, AppState>,
) -> AppResult<ApiSavedRequest> {
    state.command_bus.save_api_request(input).await
}

#[tauri::command]
pub async fn api_saved_requests(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiSavedRequest>> {
    state
        .command_bus
        .list_saved_api_requests(workspace_id)
        .await
}

#[tauri::command]
pub async fn database_connections_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<DatabaseConnection>> {
    state
        .command_bus
        .list_database_connections(workspace_id)
        .await
}

#[tauri::command]
pub async fn database_connection_save(
    input: DatabaseConnectionInput,
    state: State<'_, AppState>,
) -> AppResult<DatabaseConnection> {
    state.command_bus.save_database_connection(input).await
}

#[tauri::command]
pub async fn database_connection_delete(
    workspace_id: String,
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<DatabaseConnection>> {
    state
        .command_bus
        .delete_database_connection(workspace_id, connection_id)
        .await
}

#[tauri::command]
pub async fn database_connection_test(
    workspace_id: String,
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<DatabaseTestResult> {
    state
        .command_bus
        .test_database_connection(workspace_id, connection_id)
        .await
}

#[tauri::command]
pub async fn database_schema_get(
    workspace_id: String,
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<DatabaseSchema> {
    state
        .command_bus
        .database_schema(workspace_id, connection_id)
        .await
}

#[tauri::command]
pub async fn database_query_execute(
    input: DatabaseQueryInput,
    state: State<'_, AppState>,
) -> AppResult<DatabaseQueryResult> {
    state.command_bus.execute_database_query(input).await
}

#[tauri::command]
pub async fn database_table_browse(
    input: DatabaseBrowseInput,
    state: State<'_, AppState>,
) -> AppResult<DatabaseBrowseResult> {
    state.command_bus.browse_database_table(input).await
}
