use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::NaiveDate;

use super::*;
use crate::installation::{
    INSTALLATION_ID_LENGTH, TELEMETRY_INSTALLATION_KEY, TELEMETRY_INSTALLATION_SCOPE,
};

#[derive(Clone)]
struct FixedClock(Arc<Mutex<NaiveDate>>);

impl FixedClock {
    fn new(value: &str) -> Self {
        Self(Arc::new(Mutex::new(
            NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("valid test date"),
        )))
    }

    fn set(&self, value: &str) {
        *self.0.lock().expect("clock lock") =
            NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("valid test date");
    }
}

impl UtcDateClock for FixedClock {
    fn today_utc(&self) -> NaiveDate {
        *self.0.lock().expect("clock lock")
    }
}

#[derive(Default)]
struct MockTransport {
    responses: Mutex<VecDeque<Result<bool, ()>>>,
    payloads: Mutex<Vec<AppActivePayload>>,
}

impl MockTransport {
    fn with_responses(responses: impl IntoIterator<Item = Result<bool, ()>>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            payloads: Mutex::new(Vec::new()),
        }
    }

    fn payloads(&self) -> Vec<AppActivePayload> {
        self.payloads.lock().expect("payload lock").clone()
    }
}

#[async_trait]
impl TelemetryTransport for MockTransport {
    async fn send(&self, _endpoint: &str, payload: &AppActivePayload) -> Result<bool, ()> {
        self.payloads
            .lock()
            .expect("payload lock")
            .push(payload.clone());
        self.responses
            .lock()
            .expect("response lock")
            .pop_front()
            .unwrap_or(Ok(true))
    }
}

struct Harness {
    service: TelemetryService,
    db: LocalDb,
    clock: FixedClock,
    transport: Arc<MockTransport>,
    root: std::path::PathBuf,
}

impl Harness {
    async fn new(responses: impl IntoIterator<Item = Result<bool, ()>>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "unfour-telemetry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let db = LocalDb::connect_path(root.join("unfour.sqlite"))
            .await
            .expect("create test database");
        db.migrate().await.expect("migrate test database");
        let secrets = SecretStore::in_memory("unfour-telemetry-test");
        let clock = FixedClock::new("2026-09-01");
        let transport = Arc::new(MockTransport::with_responses(responses));
        let config = TelemetryConfig::new(
            "0.9.1",
            "windows",
            "x86_64",
            "stable",
            "standard",
            Some("https://telemetry.example.test/v1/active"),
        )
        .expect("valid test config");
        let service = TelemetryService::with_dependencies(
            db.clone(),
            secrets.clone(),
            config,
            Arc::new(clock.clone()),
            transport.clone(),
        );
        Self {
            service,
            db,
            clock,
            transport,
            root,
        }
    }

    async fn close(self) {
        self.db.pool().close().await;
        let _ = std::fs::remove_dir_all(self.root);
    }
}

#[tokio::test]
async fn telemetry_installation_id_is_43_char_base64url_without_padding_and_persists() {
    let secrets = SecretStore::in_memory("telemetry-installation-test");
    let store = TelemetryInstallationStore::new(secrets);

    let first = store.get_or_create().await.expect("first installation id");
    let second = store
        .get_or_create()
        .await
        .expect("persisted installation id");

    assert_eq!(first.len(), INSTALLATION_ID_LENGTH);
    assert_eq!(first, second);
    assert!(!first.contains('='));
    assert!(first
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'));
}

#[tokio::test]
async fn telemetry_identity_is_independent_from_account_installation_identity() {
    const ACCOUNT_SCOPE: &str = "pro-installation";
    const ACCOUNT_KEY: &str = "installation-id";
    let secrets = SecretStore::in_memory("telemetry-account-isolation-test");
    secrets
        .put_named_secret(ACCOUNT_SCOPE, ACCOUNT_KEY, &"A".repeat(43))
        .await
        .expect("seed account identity");

    let telemetry_id = TelemetryInstallationStore::new(secrets.clone())
        .get_or_create()
        .await
        .expect("telemetry identity");

    assert_ne!(
        (TELEMETRY_INSTALLATION_SCOPE, TELEMETRY_INSTALLATION_KEY),
        (ACCOUNT_SCOPE, ACCOUNT_KEY)
    );
    assert_ne!(telemetry_id, "A".repeat(43));
    assert_eq!(
        secrets
            .get_named_secret(ACCOUNT_SCOPE, ACCOUNT_KEY)
            .await
            .expect("account identity remains"),
        "A".repeat(43)
    );
}

#[tokio::test]
async fn one_success_is_allowed_per_utc_day_and_the_next_day_can_send() {
    let harness = Harness::new([Ok(true), Ok(true)]).await;

    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::Sent
    );
    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::AlreadySentToday
    );
    assert_eq!(harness.transport.payloads().len(), 1);

    harness.clock.set("2026-09-02");
    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::Sent
    );
    assert_eq!(harness.transport.payloads().len(), 2);
    harness.close().await;
}

#[tokio::test]
async fn network_failure_does_not_update_the_success_date() {
    let harness = Harness::new([Err(()), Ok(true)]).await;

    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::Failed
    );
    assert_eq!(
        harness
            .service
            .repository
            .last_successful_active_utc_date()
            .await
            .expect("read date"),
        None
    );
    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::Sent
    );
    harness.close().await;
}

#[tokio::test]
async fn non_2xx_does_not_update_the_success_date_but_2xx_does() {
    let harness = Harness::new([Ok(false), Ok(true)]).await;

    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::Failed
    );
    assert_eq!(
        harness
            .service
            .repository
            .last_successful_active_utc_date()
            .await
            .expect("read date"),
        None
    );
    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::Sent
    );
    assert_eq!(
        harness
            .service
            .repository
            .last_successful_active_utc_date()
            .await
            .expect("read date")
            .as_deref(),
        Some("2026-09-01")
    );
    harness.close().await;
}

#[tokio::test]
async fn disabled_telemetry_never_calls_the_transport() {
    let harness = Harness::new([Ok(true)]).await;
    harness
        .service
        .set_enabled(false)
        .await
        .expect("disable telemetry");

    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::Disabled
    );
    assert!(harness.transport.payloads().is_empty());
    harness.close().await;
}

#[tokio::test]
async fn payload_schema_contains_only_the_approved_fields() {
    let harness = Harness::new([Ok(true)]).await;
    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::Sent
    );
    let payloads = harness.transport.payloads();
    let value = serde_json::to_value(&payloads[0]).expect("serialize payload");
    let object = value.as_object().expect("payload object");

    assert_eq!(object.len(), 8);
    assert_eq!(value["event"], "app_active");
    assert_eq!(value["version"], "0.9.1");
    assert_eq!(value["platform"], "windows");
    assert_eq!(value["arch"], "x64");
    assert_eq!(value["channel"], "stable");
    assert_eq!(value["distribution"], "standard");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(
        value["installation_id"]
            .as_str()
            .expect("installation id")
            .len(),
        43
    );
    for forbidden in [
        "timestamp",
        "account_id",
        "user_id",
        "github_id",
        "email",
        "workspace_id",
        "hostname",
        "timezone",
        "locale",
        "ip",
        "country",
    ] {
        assert!(
            !object.contains_key(forbidden),
            "forbidden field {forbidden}"
        );
    }
    harness.close().await;
}

#[test]
fn supported_target_values_are_normalized_to_the_payload_contract() {
    let x64 = TelemetryConfig::new("1.0.0", "linux", "x86_64", "test", "microsoft-store", None)
        .expect("x64 config");
    let arm64 = TelemetryConfig::new("1.0.0", "macos", "aarch64", "stable", "standard", None)
        .expect("arm64 config");

    assert_eq!(x64.arch, "x64");
    assert_eq!(arm64.arch, "arm64");
    assert!(TelemetryConfig::new("1.0.0", "freebsd", "x86_64", "test", "standard", None).is_err());
    assert!(TelemetryConfig::new("1.0.0", "linux", "riscv64", "test", "standard", None).is_err());
}
