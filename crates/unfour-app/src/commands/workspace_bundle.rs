use crate::AppState;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use unfour_command_bus::WorkspaceBundlePreview;
use unfour_core::{
    models::{ApiCollectionExportResult, Workspace},
    AppError, AppResult,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleFilePreview {
    content: String,
    preview: WorkspaceBundlePreview,
}
#[tauri::command]
pub async fn workspace_bundle_pick(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<BundleFilePreview>> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Unfour Workspace", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|_| AppError::Validation("invalid import path".into()))?;
    // Bound the actual read, including files that grow during selection.
    use std::io::Read;
    let mut content = String::new();
    std::fs::File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_string(&mut content)?;
    let preview = state.command_bus.workspace_bundle_preview(&content).await?;
    Ok(Some(BundleFilePreview { content, preview }))
}
#[tauri::command]
pub async fn workspace_bundle_import(
    content: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<Workspace> {
    state
        .command_bus
        .workspace_bundle_import(content, name)
        .await
}
#[tauri::command]
pub async fn workspace_bundle_export(
    workspace_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ApiCollectionExportResult> {
    let content = state
        .command_bus
        .workspace_bundle_export(workspace_id)
        .await?;
    let Some(file) = app
        .dialog()
        .file()
        .set_file_name("workspace.unfour-workspace.json")
        .add_filter("Unfour Workspace", &["json"])
        .blocking_save_file()
    else {
        return Ok(ApiCollectionExportResult { saved: false });
    };
    let path = file
        .into_path()
        .map_err(|_| AppError::Validation("invalid export path".into()))?;
    std::fs::write(path, content)?;
    Ok(ApiCollectionExportResult { saved: true })
}
