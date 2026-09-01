//! Desktop anonymous active-installation telemetry.
//!
//! Construction is local and side-effect free. Network attempts happen only
//! when the Desktop frontend explicitly asks to record `app_active` after the
//! normal workbench is available.

mod installation;
mod repository;
mod transport;

use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use installation::TelemetryInstallationStore;
use repository::TelemetryRepository;
use serde::Serialize;
use tokio::sync::Mutex;
use transport::{HttpTelemetryTransport, TelemetryTransport};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;

pub const ACTIVE_EVENT: &str = "app_active";
pub const SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryConfig {
    version: String,
    platform: String,
    arch: String,
    channel: String,
    distribution: String,
    endpoint: Option<String>,
}

impl TelemetryConfig {
    /// Builds telemetry metadata from the same compiled channel/distribution
    /// values used by the Desktop app. An absent endpoint disables all network
    /// I/O, which is the Test/dev build policy.
    pub fn for_current_target(
        version: impl Into<String>,
        channel: &str,
        distribution: &str,
        endpoint: Option<&str>,
    ) -> AppResult<Self> {
        Self::new(
            version,
            std::env::consts::OS,
            std::env::consts::ARCH,
            channel,
            distribution,
            endpoint,
        )
    }

    pub fn new(
        version: impl Into<String>,
        platform: &str,
        arch: &str,
        channel: &str,
        distribution: &str,
        endpoint: Option<&str>,
    ) -> AppResult<Self> {
        let version = version.into();
        if version.trim().is_empty() {
            return Err(AppError::Config(
                "telemetry version cannot be empty".to_string(),
            ));
        }
        let platform = match platform {
            "windows" | "macos" | "linux" => platform,
            value => {
                return Err(AppError::Unsupported(format!(
                    "telemetry platform {value} is unsupported"
                )))
            }
        };
        let arch = match arch {
            "x86_64" | "x64" => "x64",
            "aarch64" | "arm64" => "arm64",
            value => {
                return Err(AppError::Unsupported(format!(
                    "telemetry architecture {value} is unsupported"
                )))
            }
        };
        if !matches!(channel, "stable" | "test") {
            return Err(AppError::Config(format!(
                "telemetry channel {channel} is invalid"
            )));
        }
        if !matches!(distribution, "standard" | "microsoft-store") {
            return Err(AppError::Config(format!(
                "telemetry distribution {distribution} is invalid"
            )));
        }
        let endpoint = endpoint
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if endpoint
            .as_deref()
            .is_some_and(|value| !value.starts_with("https://"))
        {
            return Err(AppError::Config(
                "telemetry endpoint must use HTTPS".to_string(),
            ));
        }
        Ok(Self {
            version,
            platform: platform.to_string(),
            arch: arch.to_string(),
            channel: channel.to_string(),
            distribution: distribution.to_string(),
            endpoint,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryPreferences {
    pub enabled: bool,
    pub notice_shown: bool,
    pub network_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TelemetrySendOutcome {
    Sent,
    AlreadySentToday,
    Disabled,
    NetworkDisabled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppActivePayload {
    event: &'static str,
    installation_id: String,
    version: String,
    platform: String,
    arch: String,
    channel: String,
    distribution: String,
    schema_version: u8,
}

trait UtcDateClock: Send + Sync {
    fn today_utc(&self) -> NaiveDate;
}

struct SystemUtcDateClock;

impl UtcDateClock for SystemUtcDateClock {
    fn today_utc(&self) -> NaiveDate {
        Utc::now().date_naive()
    }
}

#[derive(Clone)]
pub struct TelemetryService {
    config: TelemetryConfig,
    installation: TelemetryInstallationStore,
    repository: TelemetryRepository,
    clock: Arc<dyn UtcDateClock>,
    transport: Arc<dyn TelemetryTransport>,
    send_lock: Arc<Mutex<()>>,
}

impl TelemetryService {
    pub fn new(db: LocalDb, secrets: SecretStore, config: TelemetryConfig) -> Self {
        Self::with_dependencies(
            db,
            secrets,
            config,
            Arc::new(SystemUtcDateClock),
            Arc::new(HttpTelemetryTransport::new()),
        )
    }

    fn with_dependencies(
        db: LocalDb,
        secrets: SecretStore,
        config: TelemetryConfig,
        clock: Arc<dyn UtcDateClock>,
        transport: Arc<dyn TelemetryTransport>,
    ) -> Self {
        Self {
            config,
            installation: TelemetryInstallationStore::new(secrets),
            repository: TelemetryRepository::new(db),
            clock,
            transport,
            send_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn preferences(&self) -> AppResult<TelemetryPreferences> {
        let stored = self.repository.preferences().await?;
        Ok(TelemetryPreferences {
            enabled: stored.enabled,
            notice_shown: stored.notice_shown,
            network_enabled: self.config.endpoint.is_some(),
        })
    }

    pub async fn set_enabled(&self, enabled: bool) -> AppResult<TelemetryPreferences> {
        self.repository.set_enabled(enabled).await?;
        self.preferences().await
    }

    pub async fn mark_notice_shown(&self) -> AppResult<TelemetryPreferences> {
        self.repository.mark_notice_shown().await?;
        self.preferences().await
    }

    /// Attempts the one allowed event for the current UTC calendar day.
    /// Every failure is collapsed to `Failed`; callers must not surface it as a
    /// startup error or retry it aggressively.
    pub async fn record_active(&self) -> TelemetrySendOutcome {
        let _guard = self.send_lock.lock().await;
        match self.try_record_active().await {
            Ok(outcome) => outcome,
            Err(_) => TelemetrySendOutcome::Failed,
        }
    }

    async fn try_record_active(&self) -> AppResult<TelemetrySendOutcome> {
        if !self.repository.preferences().await?.enabled {
            return Ok(TelemetrySendOutcome::Disabled);
        }
        let Some(endpoint) = self.config.endpoint.as_deref() else {
            return Ok(TelemetrySendOutcome::NetworkDisabled);
        };
        let today = self.clock.today_utc();
        let today_text = today.format("%Y-%m-%d").to_string();
        if self
            .repository
            .last_successful_active_utc_date()
            .await?
            .as_deref()
            == Some(today_text.as_str())
        {
            return Ok(TelemetrySendOutcome::AlreadySentToday);
        }

        let payload = AppActivePayload {
            event: ACTIVE_EVENT,
            installation_id: self.installation.get_or_create().await?,
            version: self.config.version.clone(),
            platform: self.config.platform.clone(),
            arch: self.config.arch.clone(),
            channel: self.config.channel.clone(),
            distribution: self.config.distribution.clone(),
            schema_version: SCHEMA_VERSION,
        };
        match self.transport.send(endpoint, &payload).await {
            Ok(true) => {
                self.repository
                    .set_last_successful_active_utc_date(&today_text)
                    .await?;
                Ok(TelemetrySendOutcome::Sent)
            }
            Ok(false) | Err(()) => Ok(TelemetrySendOutcome::Failed),
        }
    }
}

#[cfg(test)]
mod tests;
