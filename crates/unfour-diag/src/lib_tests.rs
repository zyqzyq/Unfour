use std::path::PathBuf;

use serde_json::json;
use unfour_paths::UnfourPaths;

use super::*;

#[test]
fn default_logging_config_keeps_seven_days_of_logs() {
    let config = LoggingConfig::unified_dev(PathBuf::from("logs"));

    assert_eq!(DEFAULT_LOG_RETENTION_DAYS, 7);
    assert_eq!(config.retention_days, 7);
    assert_eq!(config.channel.as_str(), "test");
    assert_eq!(config.distribution.as_str(), "standard");
}

#[test]
fn channel_and_distribution_have_only_two_stable_values_each() {
    assert_eq!(Channel::Test.as_str(), "test");
    assert_eq!(Channel::Stable.as_str(), "stable");
    assert_eq!(serde_json::to_string(&Channel::Test).unwrap(), "\"test\"");
    assert_eq!(
        serde_json::to_string(&Channel::Stable).unwrap(),
        "\"stable\""
    );

    assert_eq!(Distribution::Standard.as_str(), "standard");
    assert_eq!(Distribution::MicrosoftStore.as_str(), "microsoft-store");
    assert_eq!(
        serde_json::to_string(&Distribution::Standard).unwrap(),
        "\"standard\""
    );
    assert_eq!(
        serde_json::to_string(&Distribution::MicrosoftStore).unwrap(),
        "\"microsoft-store\""
    );
}

#[test]
fn diagnostic_manifest_carries_full_identity_and_commit() {
    let root = std::env::temp_dir().join(format!(
        "unfour-diag-identity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let product_data_dir = root.join("Unfour");
    let paths = UnfourPaths {
        product_data_dir: product_data_dir.clone(),
        database_path: product_data_dir.join("unfour.sqlite"),
        config_dir: product_data_dir.join("config"),
        cache_dir: product_data_dir.join("cache"),
        backups_dir: product_data_dir.join("backups"),
        logs_dir: product_data_dir.join("logs"),
        diagnostics_dir: product_data_dir.join("diagnostics"),
    };
    std::fs::create_dir_all(&paths.logs_dir).expect("create logs dir");
    std::fs::create_dir_all(&paths.diagnostics_dir).expect("create diagnostics dir");

    let mut request = DiagnosticBundleRequest::unified_dev("0.1.0".to_string(), paths);
    request.channel = Channel::Stable;
    request.distribution = Distribution::Standard;
    request.commit = Some("0123456789abcdef0123456789abcdef01234567".to_string());
    let bundle = export_diagnostics_bundle(&request).expect("export bundle");
    let manifest = std::fs::read_to_string(bundle.manifest_path).expect("read manifest");

    assert!(manifest.contains("\"channel\": \"stable\""));
    assert!(manifest.contains("\"distribution\": \"standard\""));
    assert!(manifest.contains("0123456789abcdef0123456789abcdef01234567"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn event_fields_are_redacted_before_logging() {
    let fields = json!({
        "authorization": "Bearer secret",
        "cookie": "session=secret",
        "license_key": "license-secret",
        "nested": {
            "private_key": "-----BEGIN KEY-----",
            "url": "https://api.example.test/items?access_token=secret&page=1"
        }
    });

    let redacted = sanitize_event_fields(fields);
    let text = redacted.to_string();

    assert!(text.contains("<redacted>"));
    assert!(!text.contains("Bearer secret"));
    assert!(!text.contains("session=secret"));
    assert!(!text.contains("license-secret"));
    assert!(!text.contains("BEGIN KEY"));
    assert!(!text.contains("access_token=secret"));
}

#[test]
fn command_id_is_generated_with_command_prefix() {
    let command_id = new_command_id();

    assert!(command_id.starts_with("cmd_"));
    assert!(command_id.len() > "cmd_".len());
}

#[test]
fn diagnostic_bundle_includes_logs_and_excludes_sqlite_database() {
    let root = std::env::temp_dir().join(format!(
        "unfour-diag-bundle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let product_data_dir = root.join("Unfour");
    let paths = UnfourPaths {
        product_data_dir: product_data_dir.clone(),
        database_path: product_data_dir.join("unfour.sqlite"),
        config_dir: product_data_dir.join("config"),
        cache_dir: product_data_dir.join("cache"),
        backups_dir: product_data_dir.join("backups"),
        logs_dir: product_data_dir.join("logs"),
        diagnostics_dir: product_data_dir.join("diagnostics"),
    };
    std::fs::create_dir_all(&paths.logs_dir).expect("create logs dir");
    std::fs::create_dir_all(&paths.diagnostics_dir).expect("create diagnostics dir");
    std::fs::write(paths.logs_dir.join("unfour.2026-07-03.log"), "safe log").expect("write log");
    std::fs::write(&paths.database_path, "sqlite bytes").expect("write db");

    let request = DiagnosticBundleRequest::unified_dev("0.1.0".to_string(), paths);
    let bundle = export_diagnostics_bundle(&request).expect("export bundle");
    let manifest = std::fs::read_to_string(bundle.manifest_path).expect("read manifest");

    assert!(bundle.bundle_dir.is_dir());
    assert!(manifest.contains("unfour.2026-07-03.log"));
    assert!(!manifest.contains("sqlite bytes"));
    assert!(!bundle.bundle_dir.join("unfour.sqlite").exists());
    assert!(bundle
        .bundle_dir
        .join("logs")
        .join("unfour.2026-07-03.log")
        .exists());

    let _ = std::fs::remove_dir_all(root);
}
