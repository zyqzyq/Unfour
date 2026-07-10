use crate::AppState;
use serde::Serialize;
use tauri::State;
use unfour_core::AppResult;

use super::trace_command;

/// The build edition surfaced to the frontend. Serialized as the lowercase
/// string so it stays stable and locale-independent (`"community"` / `"pro"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppEditionDto {
    Community,
    Pro,
}

impl From<crate::AppEdition> for AppEditionDto {
    fn from(edition: crate::AppEdition) -> Self {
        match edition {
            crate::AppEdition::Community => Self::Community,
            crate::AppEdition::Pro => Self::Pro,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub edition: AppEditionDto,
}

/// Expose the build identity (version + edition) the frontend needs for the
/// settings page. Both fields come from the Rust [`AppState`] config, never
/// guessed from the repo name, package name, env vars, or feature flags.
#[tauri::command]
pub async fn get_app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    trace_command("get_app_info", async {
        Ok(AppInfo {
            version: state.config.app_version.clone(),
            edition: state.config.edition.into(),
        })
    })
    .await
}
