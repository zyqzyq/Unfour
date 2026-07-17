use crate::AppState;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use unfour_core::{
    models::{
        ApiCollection, ApiCollectionExportFormat, ApiCollectionExportResult, ApiCollectionFolder,
        ApiCollectionImportResult, ApiEnvironment, ApiHistoryDetail, ApiHistoryItem,
        ApiRequestInput, ApiResponse, ApiSavedRequest, KeyValue,
    },
    AppError, AppResult,
};

use super::trace_command;

#[tauri::command]
pub async fn api_environments_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiEnvironment>> {
    state.command_bus.api_environments_list(workspace_id).await
}

#[tauri::command]
pub async fn api_environment_create(
    workspace_id: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<ApiEnvironment> {
    state
        .command_bus
        .api_environment_create(workspace_id, name)
        .await
}

#[tauri::command]
pub async fn api_environment_update(
    workspace_id: String,
    environment_id: String,
    name: String,
    variables: Vec<KeyValue>,
    state: State<'_, AppState>,
) -> AppResult<ApiEnvironment> {
    state
        .command_bus
        .api_environment_update(workspace_id, environment_id, name, variables)
        .await
}

#[tauri::command]
pub async fn api_environment_delete(
    workspace_id: String,
    environment_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiEnvironment>> {
    state
        .command_bus
        .api_environment_delete(workspace_id, environment_id)
        .await
}

#[tauri::command]
pub async fn api_environment_activate(
    workspace_id: String,
    environment_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiEnvironment>> {
    state
        .command_bus
        .api_environment_activate(workspace_id, environment_id)
        .await
}

#[tauri::command]
pub async fn api_collection_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiCollection>> {
    state.command_bus.api_collection_list(workspace_id).await
}

#[tauri::command]
pub async fn api_collection_export(
    workspace_id: String,
    collection_id: String,
    format: ApiCollectionExportFormat,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ApiCollectionExportResult> {
    let artifact = state
        .command_bus
        .api_collection_export(workspace_id, collection_id, format)
        .await?;
    let extension = match format {
        ApiCollectionExportFormat::Json => "json",
        ApiCollectionExportFormat::Yaml => "yaml",
    };
    let Some(file_path) = app
        .dialog()
        .file()
        .set_file_name(&artifact.suggested_file_name)
        .add_filter("OpenAPI 3.1", &[extension])
        .blocking_save_file()
    else {
        return Ok(ApiCollectionExportResult { saved: false });
    };
    let path = file_path.into_path().map_err(|error| {
        AppError::Config(format!("selected export path is not writable: {error}"))
    })?;
    std::fs::write(path, artifact.content)?;
    Ok(ApiCollectionExportResult { saved: true })
}

#[tauri::command]
pub async fn api_collection_import(
    workspace_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ApiCollectionImportResult> {
    let Some(file_path) = app
        .dialog()
        .file()
        .add_filter("OpenAPI 3.x", &["json", "yaml", "yml"])
        .blocking_pick_file()
    else {
        return Ok(ApiCollectionImportResult {
            imported: false,
            collection: None,
            folder_count: 0,
            request_count: 0,
        });
    };
    let path = file_path.into_path().map_err(|error| {
        AppError::Config(format!("selected import path is not readable: {error}"))
    })?;
    let content = std::fs::read_to_string(path)?;
    state
        .command_bus
        .api_collection_import(workspace_id, content)
        .await
}

#[tauri::command]
pub async fn api_collection_create(
    workspace_id: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<ApiCollection> {
    state
        .command_bus
        .api_collection_create(workspace_id, name)
        .await
}

#[tauri::command]
pub async fn api_collection_rename(
    workspace_id: String,
    collection_id: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<ApiCollection> {
    state
        .command_bus
        .api_collection_rename(workspace_id, collection_id, name)
        .await
}

#[tauri::command]
pub async fn api_collection_delete(
    workspace_id: String,
    collection_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiCollection>> {
    state
        .command_bus
        .api_collection_delete(workspace_id, collection_id)
        .await
}

#[tauri::command]
pub async fn api_collection_folders_list(
    workspace_id: String,
    collection_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiCollectionFolder>> {
    state
        .command_bus
        .api_collection_folders_list(workspace_id, collection_id)
        .await
}

#[tauri::command]
pub async fn api_collection_folder_create(
    workspace_id: String,
    collection_id: String,
    parent_folder_id: Option<String>,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<ApiCollectionFolder> {
    state
        .command_bus
        .api_collection_folder_create(workspace_id, collection_id, parent_folder_id, name)
        .await
}

#[tauri::command]
pub async fn api_collection_folder_rename(
    workspace_id: String,
    folder_id: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<ApiCollectionFolder> {
    state
        .command_bus
        .api_collection_folder_rename(workspace_id, folder_id, name)
        .await
}

#[tauri::command]
pub async fn api_collection_folder_delete(
    workspace_id: String,
    folder_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiCollectionFolder>> {
    state
        .command_bus
        .api_collection_folder_delete(workspace_id, folder_id)
        .await
}

#[tauri::command]
pub async fn api_collection_folder_move(
    workspace_id: String,
    folder_id: String,
    target_parent_folder_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<ApiCollectionFolder> {
    state
        .command_bus
        .api_collection_folder_move(workspace_id, folder_id, target_parent_folder_id)
        .await
}

#[tauri::command]
pub async fn api_collection_folders_reorder(
    workspace_id: String,
    collection_id: String,
    parent_folder_id: Option<String>,
    folder_ids: Vec<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiCollectionFolder>> {
    state
        .command_bus
        .api_collection_folders_reorder(workspace_id, collection_id, parent_folder_id, folder_ids)
        .await
}

#[tauri::command]
pub async fn api_request_move(
    workspace_id: String,
    request_id: String,
    collection_id: Option<String>,
    parent_folder_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<ApiSavedRequest> {
    state
        .command_bus
        .api_request_move(workspace_id, request_id, collection_id, parent_folder_id)
        .await
}

#[tauri::command]
pub async fn api_requests_reorder(
    workspace_id: String,
    collection_id: String,
    parent_folder_id: Option<String>,
    request_ids: Vec<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiSavedRequest>> {
    state
        .command_bus
        .api_requests_reorder(workspace_id, collection_id, parent_folder_id, request_ids)
        .await
}

#[tauri::command]
pub async fn api_send_request(
    input: ApiRequestInput,
    state: State<'_, AppState>,
) -> AppResult<ApiResponse> {
    trace_command(
        "api_send_request",
        state.command_bus.send_api_request(input),
    )
    .await
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
pub async fn api_request_update(
    workspace_id: String,
    request_id: String,
    input: ApiRequestInput,
    state: State<'_, AppState>,
) -> AppResult<ApiSavedRequest> {
    state
        .command_bus
        .update_api_request(workspace_id, request_id, input)
        .await
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
pub async fn api_request_duplicate(
    workspace_id: String,
    request_id: String,
    state: State<'_, AppState>,
) -> AppResult<ApiSavedRequest> {
    state
        .command_bus
        .duplicate_api_request(workspace_id, request_id)
        .await
}

#[tauri::command]
pub async fn api_request_delete(
    workspace_id: String,
    request_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ApiSavedRequest>> {
    state
        .command_bus
        .delete_api_request(workspace_id, request_id)
        .await
}
