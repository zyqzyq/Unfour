use crate::AppState;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use unfour_core::models::{ApiCollectionExportResult, WorkspaceEnvironment};
use unfour_core::{AppError, AppResult};

#[tauri::command]
pub async fn workspace_environment_import_preview(
    workspace_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<serde_json::Value>> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Environment", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|_| AppError::Validation("invalid import path".into()))?;
    if std::fs::metadata(&path)?.len() > 10 * 1024 * 1024 {
        return Err(AppError::Validation(
            "environment import file is too large".into(),
        ));
    }
    let content = std::fs::read_to_string(path)?;
    let preview = state
        .command_bus
        .workspace_environment_import_preview(workspace_id, &content)
        .await?;
    Ok(Some(
        serde_json::json!({"content":content,"preview":preview}),
    ))
}

#[tauri::command]
pub async fn workspace_environment_import(
    workspace_id: String,
    content: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceEnvironment> {
    state
        .command_bus
        .workspace_environment_import(workspace_id, content)
        .await
}

#[tauri::command]
pub async fn workspace_environment_export(
    workspace_id: String,
    environment_id: String,
    format: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ApiCollectionExportResult> {
    let artifact = state
        .command_bus
        .workspace_environment_export(workspace_id, environment_id, format)
        .await?;
    let Some(file) = app
        .dialog()
        .file()
        .set_file_name(&artifact.suggested_file_name)
        .add_filter("Environment", &["json"])
        .blocking_save_file()
    else {
        return Ok(ApiCollectionExportResult { saved: false });
    };
    let path = file
        .into_path()
        .map_err(|_| AppError::Validation("invalid export path".into()))?;
    std::fs::write(path, artifact.content)?;
    Ok(ApiCollectionExportResult { saved: true })
}
