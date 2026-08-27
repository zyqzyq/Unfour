use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::ipc::Channel;
use tauri_plugin_updater::{Update, UpdaterExt};

pub fn app_config() -> unfour_app::UnfourAppConfig {
    unfour_app::UnfourAppConfig {
        app_name: "Unfour".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        distribution: compiled_distribution(),
        channel: compiled_channel(),
        commit: build_commit(),
    }
}

pub fn internal_updater_enabled() -> bool {
    match (compiled_distribution(), env!("UNFOUR_UPDATER_ENABLED")) {
        (unfour_app::AppDistribution::Standard, "1") => true,
        (unfour_app::AppDistribution::MicrosoftStore, "0") => false,
        (distribution, value) => panic!(
            "invalid compiled updater state for distribution {}: {value}",
            distribution.as_str()
        ),
    }
}

fn compiled_distribution() -> unfour_app::AppDistribution {
    match env!("UNFOUR_DISTRIBUTION") {
        "standard" => unfour_app::AppDistribution::Standard,
        "microsoft-store" => unfour_app::AppDistribution::MicrosoftStore,
        value => panic!("invalid compiled UNFOUR_DISTRIBUTION: {value}"),
    }
}

fn compiled_channel() -> unfour_app::ReleaseChannel {
    match env!("UNFOUR_RELEASE_CHANNEL") {
        "test" => unfour_app::ReleaseChannel::Test,
        "stable" => unfour_app::ReleaseChannel::Stable,
        value => panic!("invalid compiled UNFOUR_RELEASE_CHANNEL: {value}"),
    }
}

fn build_commit() -> Option<String> {
    match env!("UNFOUR_BUILD_COMMIT") {
        "" | "unknown" => None,
        value => Some(value.to_string()),
    }
}

fn updater_endpoint() -> Option<String> {
    internal_updater_enabled().then(|| env!("UNFOUR_UPDATE_ENDPOINT").to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMeta {
    name: String,
    version: String,
    distribution: String,
    channel: String,
    commit: Option<String>,
    updater_enabled: bool,
    endpoint: Option<String>,
}

#[tauri::command]
pub fn get_update_info() -> UpdateMeta {
    let metadata = build_metadata();
    UpdateMeta {
        name: metadata.name,
        version: metadata.version,
        distribution: metadata.distribution,
        channel: metadata.channel,
        commit: metadata.commit,
        updater_enabled: metadata.updater_enabled,
        endpoint: metadata.updater_endpoint,
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildMetadata {
    name: String,
    version: String,
    distribution: String,
    channel: String,
    commit: Option<String>,
    profile: String,
    account_api_url: String,
    account_web_url: String,
    updater_enabled: bool,
    updater_endpoint: Option<String>,
    default_storage_profile: String,
}

fn build_metadata() -> BuildMetadata {
    BuildMetadata {
        name: "Unfour".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        distribution: compiled_distribution().as_str().to_string(),
        channel: env!("UNFOUR_RELEASE_CHANNEL").to_string(),
        commit: build_commit(),
        profile: env!("UNFOUR_BUILD_PROFILE").to_string(),
        account_api_url: env!("UNFOUR_ACCOUNT_API_URL").to_string(),
        account_web_url: env!("UNFOUR_ACCOUNT_WEB_URL").to_string(),
        updater_enabled: internal_updater_enabled(),
        updater_endpoint: updater_endpoint(),
        default_storage_profile: env!("UNFOUR_DEFAULT_STORAGE_PROFILE").to_string(),
    }
}

/// Packaging-only metadata export. The MSIX wrapper validates this payload
/// before it wraps the executable, preventing stale Standard binaries from
/// entering a Microsoft Store package.
pub fn handle_build_metadata_cli() -> Result<bool, String> {
    let mut arguments = std::env::args_os().skip(1);
    let Some(first) = arguments.next() else {
        return Ok(false);
    };
    if first != "--write-build-metadata" {
        return Ok(false);
    }
    let path = arguments
        .next()
        .ok_or_else(|| "--write-build-metadata requires an absolute output path".to_string())?;
    if arguments.next().is_some() {
        return Err("--write-build-metadata accepts exactly one output path".to_string());
    }
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() {
        return Err("--write-build-metadata output path must be absolute".to_string());
    }
    let mut json = serde_json::to_string_pretty(&build_metadata())
        .map_err(|error| format!("could not serialize build metadata: {error}"))?;
    json.push('\n');
    std::fs::write(&path, json).map_err(|error| {
        format!(
            "could not write build metadata to {}: {error}",
            path.display()
        )
    })?;
    Ok(true)
}

#[tauri::command]
pub async fn check_for_update(
    app: tauri::AppHandle,
    pending: tauri::State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>, UpdateCommandError> {
    ensure_internal_updater(compiled_distribution())?;
    let endpoint = env!("UNFOUR_UPDATE_ENDPOINT");
    let url =
        url::Url::parse(endpoint).map_err(|error| UpdateCommandError::check(error.to_string()))?;
    let update = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|error| UpdateCommandError::check(error.to_string()))?
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| UpdateCommandError::check(error.to_string()))?
        .check()
        .await
        .map_err(|error| UpdateCommandError::check(error.to_string()))?;
    let info = update.as_ref().map(|candidate| UpdateInfo {
        version: candidate.version.clone(),
        current_version: candidate.current_version.clone(),
        date: candidate.date.map(|date| date.to_string()),
        body: candidate.body.clone(),
    });
    let mut guard = pending.0.lock().map_err(|_| {
        UpdateCommandError::check("update state is unavailable; restart Unfour and try again")
    })?;
    *guard = update;
    Ok(info)
}

#[tauri::command]
pub async fn install_update(
    pending: tauri::State<'_, PendingUpdate>,
    on_event: Channel<UpdateDownloadEvent>,
) -> Result<(), UpdateCommandError> {
    ensure_internal_updater(compiled_distribution())?;
    let mut update = pending
        .0
        .lock()
        .map_err(|_| {
            UpdateCommandError::check("update state is unavailable; restart Unfour and try again")
        })?
        .clone()
        .ok_or_else(|| UpdateCommandError::check("check for updates again before installing"))?;

    update.timeout = Some(Duration::from_secs(180));
    let mut started = false;
    let mut progress = UpdateProgressBatcher::new(Instant::now());
    let bytes = update
        .download(
            |chunk_length, content_length| {
                if !started {
                    let _ = on_event.send(UpdateDownloadEvent::Started { content_length });
                    started = true;
                }
                if let Some(chunk_length) = progress.push(chunk_length, Instant::now()) {
                    let _ = on_event.send(UpdateDownloadEvent::Progress { chunk_length });
                }
            },
            || {},
        )
        .await
        .map_err(update_download_error)?;

    if !started {
        let _ = on_event.send(UpdateDownloadEvent::Started {
            content_length: Some(0),
        });
    }
    if let Some(chunk_length) = progress.flush(Instant::now()) {
        let _ = on_event.send(UpdateDownloadEvent::Progress { chunk_length });
    }
    let _ = on_event.send(UpdateDownloadEvent::Downloaded);
    let _ = on_event.send(UpdateDownloadEvent::Installing);
    update.install(bytes).map_err(|error| {
        UpdateCommandError::installer(format!(
            "The operating system could not start the update installer: {error}. Close any MCP client using Unfour and try again"
        ))
    })?;
    *pending.0.lock().map_err(|_| {
        UpdateCommandError::check("update state is unavailable; restart Unfour and try again")
    })? = None;
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current_version: String,
    date: Option<String>,
    body: Option<String>,
}

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum UpdateDownloadEvent {
    Started {
        #[serde(rename = "contentLength")]
        content_length: Option<u64>,
    },
    Progress {
        #[serde(rename = "chunkLength")]
        chunk_length: usize,
    },
    Downloaded,
    Installing,
}

const UPDATE_PROGRESS_INTERVAL: Duration = Duration::from_millis(120);
const UPDATE_PROGRESS_BATCH_BYTES: usize = 256 * 1024;

struct UpdateProgressBatcher {
    pending_bytes: usize,
    last_emit: Instant,
}

impl UpdateProgressBatcher {
    fn new(now: Instant) -> Self {
        Self {
            pending_bytes: 0,
            last_emit: now,
        }
    }

    fn push(&mut self, chunk_length: usize, now: Instant) -> Option<usize> {
        self.pending_bytes = self.pending_bytes.saturating_add(chunk_length);
        if self.pending_bytes >= UPDATE_PROGRESS_BATCH_BYTES
            || now.saturating_duration_since(self.last_emit) >= UPDATE_PROGRESS_INTERVAL
        {
            self.flush(now)
        } else {
            None
        }
    }

    fn flush(&mut self, now: Instant) -> Option<usize> {
        if self.pending_bytes == 0 {
            return None;
        }
        self.last_emit = now;
        Some(std::mem::take(&mut self.pending_bytes))
    }
}

#[derive(Debug, serde::Serialize)]
pub struct UpdateCommandError {
    code: &'static str,
    message: String,
}

impl UpdateCommandError {
    fn managed_by_store() -> Self {
        Self {
            code: "updates_managed_by_microsoft_store",
            message: "Updates are managed by Microsoft Store".to_string(),
        }
    }

    fn check(message: impl Into<String>) -> Self {
        Self {
            code: "check_failed",
            message: message.into(),
        }
    }

    fn download(message: impl Into<String>) -> Self {
        Self {
            code: "update_download_failed",
            message: message.into(),
        }
    }

    fn signature(message: impl Into<String>) -> Self {
        Self {
            code: "update_signature_verification_failed",
            message: message.into(),
        }
    }

    fn installer(message: impl Into<String>) -> Self {
        Self {
            code: "installer_start_failed",
            message: message.into(),
        }
    }
}

fn ensure_internal_updater(
    distribution: unfour_app::AppDistribution,
) -> Result<(), UpdateCommandError> {
    match distribution {
        unfour_app::AppDistribution::Standard => Ok(()),
        unfour_app::AppDistribution::MicrosoftStore => Err(UpdateCommandError::managed_by_store()),
    }
}

fn update_download_error(error: tauri_plugin_updater::Error) -> UpdateCommandError {
    match error {
        tauri_plugin_updater::Error::Minisign(error) => UpdateCommandError::signature(format!(
            "update signature verification failed: {error}. The downloaded package was rejected and was not installed"
        )),
        tauri_plugin_updater::Error::Base64(error) => UpdateCommandError::signature(format!(
            "update signature verification failed: {error}. The downloaded package was rejected and was not installed"
        )),
        tauri_plugin_updater::Error::SignatureUtf8(error) => UpdateCommandError::signature(format!(
            "update signature verification failed: {error}. The downloaded package was rejected and was not installed"
        )),
        tauri_plugin_updater::Error::Reqwest(error) if error.is_body() || error.is_decode() => {
            UpdateCommandError::download(format!(
                "update download failed while reading the response body: {error}. Check the network, proxy, or security software and try again"
            ))
        }
        error => UpdateCommandError::download(format!(
            "update download failed: {error}. Check the network and try again"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microsoft_store_distribution_cannot_access_internal_updater() {
        let error = ensure_internal_updater(unfour_app::AppDistribution::MicrosoftStore)
            .expect_err("Store updater must be disabled");
        assert_eq!(error.code, "updates_managed_by_microsoft_store");
        assert!(ensure_internal_updater(unfour_app::AppDistribution::Standard).is_ok());
    }

    #[test]
    fn compiled_metadata_matches_distribution_policy() {
        let metadata = build_metadata();
        assert_eq!(metadata.distribution, compiled_distribution().as_str());
        assert_eq!(metadata.updater_enabled, internal_updater_enabled());
        if metadata.distribution == "standard" {
            assert_eq!(
                metadata.updater_endpoint,
                Some(format!(
                    "https://release.unfour.dev/{}/latest.json",
                    metadata.channel
                ))
            );
        } else {
            assert_eq!(metadata.distribution, "microsoft-store");
            assert!(!metadata.updater_enabled);
            assert_eq!(metadata.updater_endpoint, None);
        }
    }

    #[test]
    fn progress_events_use_frontend_field_names() {
        let value = serde_json::to_value(UpdateDownloadEvent::Progress { chunk_length: 4096 })
            .expect("serialize progress event");
        assert_eq!(value["event"], "progress");
        assert_eq!(value["chunkLength"], 4096);
        assert!(value.get("chunk_length").is_none());
    }

    #[test]
    fn progress_batches_small_chunks() {
        let now = Instant::now();
        let mut progress = UpdateProgressBatcher::new(now);
        let quarter = UPDATE_PROGRESS_BATCH_BYTES / 4;
        assert_eq!(progress.push(quarter, now), None);
        assert_eq!(progress.push(quarter, now), None);
        assert_eq!(progress.push(quarter, now), None);
        assert_eq!(
            progress.push(quarter, now),
            Some(UPDATE_PROGRESS_BATCH_BYTES)
        );
    }
}
