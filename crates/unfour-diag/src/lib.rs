use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::EnvFilter;
use unfour_core::redaction::{
    is_sensitive_key, redact_connection_string, redact_url_query, REDACTED_VALUE,
};
use unfour_paths::UnfourPaths;
use uuid::Uuid;

pub const DEFAULT_LOG_RETENTION_DAYS: u64 = 7;

static LOGGING_METADATA: OnceLock<LoggingMetadata> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edition {
    Oss,
    Pro,
    Team,
}

impl Edition {
    pub fn as_str(self) -> &'static str {
        match self {
            Edition::Oss => "oss",
            Edition::Pro => "pro",
            Edition::Team => "team",
        }
    }
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Dev,
    Beta,
    Stable,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Dev => "dev",
            Channel::Beta => "beta",
            Channel::Stable => "stable",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    Dev,
    Website,
    MicrosoftStore,
    Winget,
}

impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Dev => "dev",
            PackageKind::Website => "website",
            PackageKind::MicrosoftStore => "microsoft_store",
            PackageKind::Winget => "winget",
        }
    }
}

impl fmt::Display for PackageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingConfig {
    pub app_name: String,
    pub edition: Edition,
    pub channel: Channel,
    pub package_kind: PackageKind,
    pub version: String,
    pub commit: Option<String>,
    pub log_level: String,
    pub log_dir: PathBuf,
    pub retention_days: u64,
}

impl LoggingConfig {
    pub fn oss_dev(log_dir: PathBuf) -> Self {
        Self {
            app_name: "Unfour".to_string(),
            edition: Edition::Oss,
            channel: Channel::Dev,
            package_kind: PackageKind::Dev,
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit: None,
            log_level: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "info".to_string()
            },
            log_dir,
            retention_days: DEFAULT_LOG_RETENTION_DAYS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingMetadata {
    pub app_name: String,
    pub edition: Edition,
    pub channel: Channel,
    pub package_kind: PackageKind,
    pub version: String,
    pub commit: Option<String>,
}

impl From<&LoggingConfig> for LoggingMetadata {
    fn from(config: &LoggingConfig) -> Self {
        Self {
            app_name: config.app_name.clone(),
            edition: config.edition,
            channel: config.channel,
            package_kind: config.package_kind,
            version: config.version.clone(),
            commit: config.commit.clone(),
        }
    }
}

pub struct LoggingGuard {
    _guard: WorkerGuard,
}

/// Local-time formatter for the human-readable (non-JSON) log lines.
struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))
    }
}

pub fn init_logging(config: LoggingConfig) -> io::Result<LoggingGuard> {
    fs::create_dir_all(&config.log_dir)?;
    prune_old_logs(&config.log_dir, config.retention_days)?;

    let metadata = LoggingMetadata::from(&config);
    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("unfour")
        .filename_suffix("log")
        .build(&config.log_dir)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let filter = EnvFilter::try_new(&config.log_level)
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_timer(LocalTimer)
        .with_ansi(false)
        .finish();

    let _ = LOGGING_METADATA.set(metadata.clone());
    if tracing::subscriber::set_global_default(subscriber).is_ok() {
        tracing::info!(
            event = "logging_initialized",
            module = "diag",
            operation = "init_logging",
            status = "ok",
            edition = %metadata.edition,
            channel = %metadata.channel,
            package_kind = %metadata.package_kind,
            app_version = %metadata.version,
            retention_days = config.retention_days,
            log_dir = %safe_path_display(&config.log_dir),
        );
    }

    Ok(LoggingGuard { _guard: guard })
}

pub fn metadata() -> Option<&'static LoggingMetadata> {
    LOGGING_METADATA.get()
}

pub fn new_command_id() -> String {
    format!("cmd_{}", Uuid::new_v4())
}

pub fn new_request_id() -> String {
    format!("req_{}", Uuid::new_v4())
}

pub fn sanitize_event_fields(value: Value) -> Value {
    sanitize_value(None, value)
}

pub fn safe_path_display(path: &Path) -> String {
    let parts = path
        .components()
        .rev()
        .take(3)
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    parts
        .into_iter()
        .rev()
        .collect::<PathBuf>()
        .display()
        .to_string()
}

pub fn prune_old_logs(log_dir: &Path, retention_days: u64) -> io::Result<()> {
    if retention_days == 0 || !log_dir.exists() {
        return Ok(());
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    for entry in fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("unfour.") || !name.ends_with(".log") {
            continue;
        }
        let modified = entry.metadata()?.modified().unwrap_or(SystemTime::now());
        if modified < cutoff {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub fn app_error_kind(error: &unfour_core::AppError) -> &'static str {
    error.code()
}

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

pub fn log_command_started(command: &str, command_id: &str) {
    let meta = metadata();
    tracing::info!(
        event = "command_started",
        module = "command_bus",
        operation = command,
        command_id = command_id,
        status = "started",
        edition = meta.map(|m| m.edition.as_str()).unwrap_or("oss"),
        package_kind = meta.map(|m| m.package_kind.as_str()).unwrap_or("dev"),
    );
}

pub fn log_command_completed(command: &str, command_id: &str, duration_ms: u128) {
    let meta = metadata();
    tracing::info!(
        event = "command_completed",
        module = "command_bus",
        operation = command,
        command_id = command_id,
        duration_ms = duration_ms,
        status = "ok",
        edition = meta.map(|m| m.edition.as_str()).unwrap_or("oss"),
        package_kind = meta.map(|m| m.package_kind.as_str()).unwrap_or("dev"),
    );
}

pub fn log_command_failed(command: &str, command_id: &str, duration_ms: u128, error_kind: &str) {
    let meta = metadata();
    tracing::error!(
        event = "command_failed",
        module = "command_bus",
        operation = command,
        command_id = command_id,
        duration_ms = duration_ms,
        status = "error",
        error_kind = error_kind,
        edition = meta.map(|m| m.edition.as_str()).unwrap_or("oss"),
        package_kind = meta.map(|m| m.package_kind.as_str()).unwrap_or("dev"),
    );
}

pub fn log_operation_event(
    event: &str,
    module: &str,
    operation: &str,
    status: &str,
    duration_ms: Option<u128>,
    error_kind: Option<&str>,
    fields: Value,
) {
    let meta = metadata();
    let fields = sanitize_event_fields(fields);
    let fields = fields.to_string();
    let duration_ms = duration_ms.unwrap_or_default();
    let error_kind = error_kind.unwrap_or("");

    if status == "error" || status == "failed" {
        tracing::error!(
            event = event,
            module = module,
            operation = operation,
            duration_ms = duration_ms,
            status = status,
            error_kind = error_kind,
            edition = meta.map(|m| m.edition.as_str()).unwrap_or("oss"),
            package_kind = meta.map(|m| m.package_kind.as_str()).unwrap_or("dev"),
            fields = %fields,
        );
    } else {
        tracing::info!(
            event = event,
            module = module,
            operation = operation,
            duration_ms = duration_ms,
            status = status,
            error_kind = error_kind,
            edition = meta.map(|m| m.edition.as_str()).unwrap_or("oss"),
            package_kind = meta.map(|m| m.package_kind.as_str()).unwrap_or("dev"),
            fields = %fields,
        );
    }
}

fn sanitize_value(key: Option<&str>, value: Value) -> Value {
    if key.is_some_and(is_sensitive_key) {
        return Value::String(REDACTED_VALUE.to_string());
    }

    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let value = sanitize_value(Some(&key), value);
                    (key, value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| sanitize_value(None, item))
                .collect(),
        ),
        Value::String(value) => {
            let redacted_url = redact_url_query(&value);
            let redacted = redact_connection_string(&redacted_url);
            Value::String(redacted)
        }
        other => other,
    }
}

#[cfg(test)]
mod lib_tests;
