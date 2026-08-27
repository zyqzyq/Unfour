use crate::AppState;
use serde::Serialize;
use tauri::State;
use unfour_core::AppResult;

use super::trace_command;

/// The distribution channel surfaced to the frontend. Serialized as the
/// stable, locale-independent values (`"standard"` / `"microsoft-store"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppDistributionDto {
    Standard,
    MicrosoftStore,
}

impl From<crate::AppDistribution> for AppDistributionDto {
    fn from(distribution: crate::AppDistribution) -> Self {
        match distribution {
            crate::AppDistribution::Standard => Self::Standard,
            crate::AppDistribution::MicrosoftStore => Self::MicrosoftStore,
        }
    }
}

/// The release channel surfaced to the frontend. Serialized as the lowercase
/// string so it stays stable and locale-independent (`"test"` / `"stable"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppChannelDto {
    Test,
    Stable,
}

impl From<crate::ReleaseChannel> for AppChannelDto {
    fn from(channel: crate::ReleaseChannel) -> Self {
        match channel {
            crate::ReleaseChannel::Test => Self::Test,
            crate::ReleaseChannel::Stable => Self::Stable,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub distribution: AppDistributionDto,
    pub channel: AppChannelDto,
    pub commit: Option<String>,
}

/// Expose the compile-time build identity the frontend needs for the About
/// page. Every field comes from the Rust [`AppState`] config, never guessed
/// from the repo name, package name, env vars, or feature flags.
#[tauri::command]
pub async fn get_app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    trace_command("get_app_info", async {
        Ok(AppInfo {
            name: state.config.app_name.clone(),
            version: state.config.app_version.clone(),
            distribution: state.config.distribution.into(),
            channel: state.config.channel.into(),
            commit: state.config.commit.clone(),
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_values_are_stable_and_lowercase() {
        assert_eq!(
            serde_json::to_string(&AppDistributionDto::Standard).unwrap(),
            "\"standard\""
        );
        assert_eq!(
            serde_json::to_string(&AppDistributionDto::MicrosoftStore).unwrap(),
            "\"microsoft-store\""
        );
        assert_eq!(
            serde_json::to_string(&AppChannelDto::Test).unwrap(),
            "\"test\""
        );
        assert_eq!(
            serde_json::to_string(&AppChannelDto::Stable).unwrap(),
            "\"stable\""
        );
    }

    #[test]
    fn app_info_serializes_with_camel_case_and_null_commit() {
        let info = AppInfo {
            name: "Unfour".to_string(),
            version: "0.1.0".to_string(),
            distribution: AppDistributionDto::Standard,
            channel: AppChannelDto::Test,
            commit: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["name"], "Unfour");
        assert_eq!(json["version"], "0.1.0");
        assert_eq!(json["distribution"], "standard");
        assert_eq!(json["channel"], "test");
        assert!(json["commit"].is_null());
    }
}
