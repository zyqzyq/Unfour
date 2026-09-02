use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::NaiveDate;

use super::*;
use crate::installation::{
    INSTALLATION_ID_LENGTH, TELEMETRY_INSTALLATION_KEY, TELEMETRY_INSTALLATION_SCOPE,
};

static NEXT_TEST_ROOT_ID: AtomicU64 = AtomicU64::new(0);

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
        Self::new_with_endpoint(responses, Some("https://telemetry.example.test/v1/active")).await
    }

    async fn new_with_endpoint(
        responses: impl IntoIterator<Item = Result<bool, ()>>,
        endpoint: Option<&str>,
    ) -> Self {
        let root = std::env::temp_dir().join(format!(
            "unfour-telemetry-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos(),
            NEXT_TEST_ROOT_ID.fetch_add(1, Ordering::Relaxed),
        ));
        let db = LocalDb::connect_path(root.join("unfour.sqlite"))
            .await
            .expect("create test database");
        db.migrate().await.expect("migrate test database");
        let secrets = SecretStore::in_memory("unfour-telemetry-test");
        let clock = FixedClock::new("2026-09-01");
        let transport = Arc::new(MockTransport::with_responses(responses));
        let config =
            TelemetryConfig::new("0.9.2", "windows", "x86_64", "stable", "standard", endpoint)
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
async fn network_disabled_telemetry_never_calls_the_transport() {
    let harness = Harness::new_with_endpoint([Ok(true)], None).await;

    assert_eq!(
        harness.service.record_active().await,
        TelemetrySendOutcome::NetworkDisabled
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
    assert_eq!(value["version"], "0.9.2");
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

#[test]
fn telemetry_endpoint_requires_valid_https_without_credentials_or_fragment() {
    let valid = TelemetryConfig::new(
        "1.0.0",
        "windows",
        "x86_64",
        "stable",
        "standard",
        Some("https://telemetry.example.test/v1/active"),
    );
    assert!(valid.is_ok());

    for endpoint in [
        "http://telemetry.example.test/v1/active",
        "not a url",
        "https://",
        "https://@telemetry.example.test/v1/active",
        "https://user@telemetry.example.test/v1/active",
        "https://:password@telemetry.example.test/v1/active",
        "https://telemetry.example.test/v1/active#fragment",
    ] {
        assert!(
            TelemetryConfig::new(
                "1.0.0",
                "windows",
                "x86_64",
                "stable",
                "standard",
                Some(endpoint),
            )
            .is_err(),
            "endpoint should be rejected: {endpoint}"
        );
    }
}

fn test_payload() -> AppActivePayload {
    AppActivePayload {
        event: ACTIVE_EVENT,
        installation_id: "A".repeat(43),
        version: "0.9.2".to_string(),
        platform: "windows".to_string(),
        arch: "x64".to_string(),
        channel: "stable".to_string(),
        distribution: "standard".to_string(),
        schema_version: SCHEMA_VERSION,
    }
}

#[tokio::test]
async fn http_transport_does_not_follow_redirects_and_treats_3xx_as_failure() {
    use crate::transport::{HttpTelemetryTransport, TelemetryTransport};

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind redirect test server");
    let endpoint = format!(
        "http://{}/v1/active",
        listener.local_addr().expect("redirect server address")
    );
    let location = endpoint.clone();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept telemetry request");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("set request read timeout");
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream
            .write_all(response.as_bytes())
            .expect("write redirect response");
        drop(stream);

        listener
            .set_nonblocking(true)
            .expect("set redirect listener nonblocking");
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match listener.accept() {
                Ok(_) => return true,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return false;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return false,
            }
        }
    });

    let payload = test_payload();
    let result = HttpTelemetryTransport::new()
        .send(&endpoint, &payload)
        .await;
    let followed_redirect = server.join().expect("join redirect test server");

    assert_eq!(result, Ok(false));
    assert!(!followed_redirect, "telemetry client followed the redirect");
}
