mod bundle;

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
        write!(
            w,
            "{}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
        )
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

pub use bundle::{export_diagnostics_bundle, DiagnosticBundle, DiagnosticBundleRequest};

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
