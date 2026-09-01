//! Build-time release identity for the unified desktop binary.
//!
//! `scripts/release-channel.mjs` is the single policy resolver. It bakes the
//! release channel, distribution, Account origins, updater authority, storage
//! profile, and build commit into the executable. Store builds therefore have
//! no runtime switch that can re-enable the Standard updater.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let profile = resolve_build_profile();
    bake_build_profile(&profile);
    resolve_build_commit();
    tauri_build::build();

    #[cfg(target_os = "windows")]
    {
        let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        println!("cargo:rustc-link-search=native={}", out_dir.display());
    }
}

#[derive(Debug)]
struct ResolvedBuildProfile {
    kind: String,
    release_channel: String,
    distribution: String,
    account_api_url: String,
    account_web_url: String,
    telemetry_endpoint: String,
    updater_enabled: String,
    updater_endpoint: String,
    allow_loopback_http: String,
    default_storage_profile: String,
}

fn resolve_build_profile() -> ResolvedBuildProfile {
    println!("cargo:rerun-if-env-changed=UNFOUR_RELEASE_CHANNEL");
    println!("cargo:rerun-if-env-changed=UNFOUR_DISTRIBUTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let explicit_channel = std::env::var("UNFOUR_RELEASE_CHANNEL")
        .ok()
        .filter(|value| !value.is_empty());
    let explicit_distribution = std::env::var("UNFOUR_DISTRIBUTION")
        .ok()
        .filter(|value| !value.is_empty());
    let script = repo_root().join("scripts").join("release-channel.mjs");
    println!("cargo:rerun-if-changed={}", script.display());

    let mut command = Command::new("node");
    command
        .arg(&script)
        .args(["--version", &version, "--format", "lines"]);
    if let Some(channel) = explicit_channel.as_deref() {
        command.args(["--expected-channel", channel]);
    }
    if let Some(distribution) = explicit_distribution.as_deref() {
        command.args(["--distribution", distribution]);
    }
    let output = command.output().unwrap_or_else(|error| {
        panic!(
            "failed to run build profile resolver {}: {error}. Node.js is required for Unfour workspace builds",
            script.display()
        )
    });
    if !output.status.success() {
        panic!(
            "build profile resolution failed for version {version}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let values = parse_profile_lines(&String::from_utf8_lossy(&output.stdout));
    let profile = ResolvedBuildProfile {
        kind: required_profile_value(&values, "profile_kind"),
        release_channel: required_profile_value(&values, "release_channel"),
        distribution: required_profile_value(&values, "distribution"),
        account_api_url: required_profile_value(&values, "account_api_url"),
        account_web_url: required_profile_value(&values, "account_web_url"),
        telemetry_endpoint: values
            .get("telemetry_endpoint")
            .cloned()
            .unwrap_or_default(),
        updater_enabled: required_profile_value(&values, "updater_enabled"),
        updater_endpoint: values.get("updater_endpoint").cloned().unwrap_or_default(),
        allow_loopback_http: required_profile_value(&values, "allow_loopback_http"),
        default_storage_profile: required_profile_value(&values, "default_storage_profile"),
    };

    match (
        profile.distribution.as_str(),
        profile.updater_enabled.as_str(),
        profile.updater_endpoint.is_empty(),
    ) {
        ("standard", "1", false) | ("microsoft-store", "0", true) => {}
        _ => panic!(
            "invalid distribution/updater build profile: distribution={}, updater_enabled={}, updater_endpoint={:?}",
            profile.distribution, profile.updater_enabled, profile.updater_endpoint
        ),
    }
    match (
        profile.release_channel.as_str(),
        profile.telemetry_endpoint.is_empty(),
    ) {
        ("test", true) | ("stable", false) => {}
        _ => panic!(
            "invalid telemetry build profile: channel={}, endpoint={:?}",
            profile.release_channel, profile.telemetry_endpoint
        ),
    }
    profile
}

fn parse_profile_lines(output: &str) -> BTreeMap<String, String> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (key, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("invalid build profile output line: {line:?}"));
            (key.to_string(), value.to_string())
        })
        .collect()
}

fn required_profile_value(values: &BTreeMap<String, String>, key: &str) -> String {
    values
        .get(key)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("build profile resolver omitted {key}"))
        .clone()
}

fn bake_build_profile(profile: &ResolvedBuildProfile) {
    println!("cargo:rustc-env=UNFOUR_BUILD_PROFILE={}", profile.kind);
    println!(
        "cargo:rustc-env=UNFOUR_RELEASE_CHANNEL={}",
        profile.release_channel
    );
    println!(
        "cargo:rustc-env=UNFOUR_DISTRIBUTION={}",
        profile.distribution
    );
    println!(
        "cargo:rustc-env=UNFOUR_UPDATER_ENABLED={}",
        profile.updater_enabled
    );
    println!(
        "cargo:rustc-env=UNFOUR_UPDATE_ENDPOINT={}",
        profile.updater_endpoint
    );
    println!(
        "cargo:rustc-env=UNFOUR_ACCOUNT_API_URL={}",
        profile.account_api_url
    );
    println!(
        "cargo:rustc-env=UNFOUR_ACCOUNT_WEB_URL={}",
        profile.account_web_url
    );
    println!(
        "cargo:rustc-env=UNFOUR_TELEMETRY_ENDPOINT={}",
        profile.telemetry_endpoint
    );
    println!(
        "cargo:rustc-env=UNFOUR_ACCOUNT_ALLOW_LOOPBACK_HTTP={}",
        profile.allow_loopback_http
    );
    println!(
        "cargo:rustc-env=UNFOUR_DEFAULT_STORAGE_PROFILE={}",
        profile.default_storage_profile
    );
}

fn resolve_build_commit() {
    println!("cargo:rerun-if-env-changed=UNFOUR_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");

    let commit = if let Ok(explicit) = std::env::var("UNFOUR_BUILD_COMMIT") {
        nonempty_or_unknown(&explicit)
    } else if let Ok(github_sha) = std::env::var("GITHUB_SHA") {
        nonempty_or_unknown(&github_sha)
    } else {
        resolve_git_head().unwrap_or_else(|| "unknown".to_string())
    };
    println!("cargo:rustc-env=UNFOUR_BUILD_COMMIT={commit}");

    if let Some(git_dir) = locate_git_dir() {
        for relative in ["HEAD", "packed-refs"] {
            let path = git_dir.join(relative);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

fn nonempty_or_unknown(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

fn resolve_git_head() -> Option<String> {
    let sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|sha| !sha.is_empty())?;
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false);
    Some(if dirty { format!("{sha}-dirty") } else { sha })
}

fn locate_git_dir() -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn repo_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("Cargo always provides CARGO_MANIFEST_DIR to build scripts");
    let mut path = PathBuf::from(manifest_dir);
    for _ in 0..3 {
        path.pop();
    }
    path
}
