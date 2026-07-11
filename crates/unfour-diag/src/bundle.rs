use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use unfour_paths::UnfourPaths;
use uuid::Uuid;

use super::{safe_path_display, Channel, Edition, PackageKind};

#[derive(Debug, Clone)]
pub struct DiagnosticBundleRequest {
    pub app_name: String,
    pub version: String,
    pub edition: Edition,
    pub channel: Channel,
    pub package_kind: PackageKind,
    pub commit: Option<String>,
    pub paths: UnfourPaths,
}

impl DiagnosticBundleRequest {
    pub fn oss_dev(version: String, paths: UnfourPaths) -> Self {
        Self {
            app_name: "Unfour".to_string(),
            version,
            edition: Edition::Oss,
            channel: Channel::Dev,
            package_kind: PackageKind::Dev,
            commit: None,
            paths,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticBundle {
    pub bundle_dir: PathBuf,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticBundleManifest {
    app_name: String,
    version: String,
    edition: String,
    channel: String,
    package_kind: String,
    commit: Option<String>,
    platform: String,
    created_at: String,
    safe_paths: SafePathManifest,
    logs: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SafePathManifest {
    product_data_dir: String,
    config_dir: String,
    cache_dir: String,
    logs_dir: String,
    diagnostics_dir: String,
    database_exists: bool,
}

pub fn export_diagnostics_bundle(
    request: &DiagnosticBundleRequest,
) -> io::Result<DiagnosticBundle> {
    fs::create_dir_all(&request.paths.diagnostics_dir)?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let bundle_dir = request
        .paths
        .diagnostics_dir
        .join(format!("diagnostics-{stamp}-{}", Uuid::new_v4().simple()));
    let logs_dir = bundle_dir.join("logs");
    fs::create_dir_all(&logs_dir)?;

    let copied_logs = copy_recent_logs(&request.paths.logs_dir, &logs_dir)?;
    let manifest = DiagnosticBundleManifest {
        app_name: request.app_name.clone(),
        version: request.version.clone(),
        edition: request.edition.as_str().to_string(),
        channel: request.channel.as_str().to_string(),
        package_kind: request.package_kind.as_str().to_string(),
        commit: request.commit.clone(),
        platform: std::env::consts::OS.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        safe_paths: SafePathManifest {
            product_data_dir: safe_path_display(&request.paths.product_data_dir),
            config_dir: safe_path_display(&request.paths.config_dir),
            cache_dir: safe_path_display(&request.paths.cache_dir),
            logs_dir: safe_path_display(&request.paths.logs_dir),
            diagnostics_dir: safe_path_display(&request.paths.diagnostics_dir),
            database_exists: request.paths.database_path.exists(),
        },
        logs: copied_logs,
    };
    let manifest_path = bundle_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(&manifest_path, manifest_json)?;

    Ok(DiagnosticBundle {
        bundle_dir,
        manifest_path,
    })
}

fn copy_recent_logs(source_dir: &Path, target_dir: &Path) -> io::Result<Vec<String>> {
    if !source_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(source_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            name.starts_with("unfour.") && name.ends_with(".log")
        })
        .map(|entry| {
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (modified, entry)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.0.cmp(&left.0));

    let mut copied = Vec::new();
    for (_, entry) in entries.into_iter().take(7) {
        let file_name = entry.file_name();
        fs::copy(entry.path(), target_dir.join(&file_name))?;
        copied.push(file_name.to_string_lossy().to_string());
    }
    Ok(copied)
}
