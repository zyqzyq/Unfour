use crate::AppState;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use unfour_command_bus::{WorkspaceBundleOptions, WorkspaceBundlePreview};
use unfour_core::{
    models::{ApiCollectionExportResult, Workspace},
    AppError, AppResult,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleFilePreview {
    content: String,
    preview: Option<WorkspaceBundlePreview>,
    encrypted: bool,
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
        .take(48 * 1024 * 1024 + 1)
        .read_to_string(&mut content)?;
    if content.len() > 48 * 1024 * 1024 {
        return Err(AppError::Validation("WORKSPACE_BUNDLE_TOO_LARGE".into()));
    }
    let encrypted = serde_json::from_str::<serde_json::Value>(&content)
        .ok()
        .is_some_and(|v| v["format"] == "unfour-workspace-encrypted");
    let preview = if encrypted {
        None
    } else {
        Some(
            state
                .command_bus
                .workspace_bundle_preview_with_options(&content, WorkspaceBundleOptions::default())
                .await?,
        )
    };
    Ok(Some(BundleFilePreview {
        content,
        preview,
        encrypted,
    }))
}
#[tauri::command]
pub async fn workspace_bundle_preview(
    content: String,
    options: WorkspaceBundleOptions,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceBundlePreview> {
    state
        .command_bus
        .workspace_bundle_preview_with_options(&content, options)
        .await
}
#[tauri::command]
pub async fn workspace_bundle_import(
    content: String,
    name: String,
    options: Option<WorkspaceBundleOptions>,
    state: State<'_, AppState>,
) -> AppResult<Workspace> {
    state
        .command_bus
        .workspace_bundle_import_with_options(content, name, options.unwrap_or_default())
        .await
}
#[tauri::command]
pub async fn workspace_bundle_export(
    workspace_id: String,
    options: Option<WorkspaceBundleOptions>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ApiCollectionExportResult> {
    let artifact = state
        .command_bus
        .workspace_bundle_export_with_options(workspace_id, options.unwrap_or_default())
        .await?;
    let Some(file) = app
        .dialog()
        .file()
        .set_file_name(&artifact.suggested_file_name)
        .add_filter("Unfour Workspace", &["json"])
        .blocking_save_file()
    else {
        return Ok(ApiCollectionExportResult { saved: false });
    };
    let path = file
        .into_path()
        .map_err(|_| AppError::Validation("invalid export path".into()))?;
    // Complete the write before replacing an existing backup (including on Windows).
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Validation("invalid export path".into()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(artifact.content.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(&path)
        .map_err(|error| AppError::from(error.error))?;
    Ok(ApiCollectionExportResult { saved: true })
}
