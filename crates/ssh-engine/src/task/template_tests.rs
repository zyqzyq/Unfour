use super::*;

fn step(step_type: &str, config_json: serde_json::Value) -> SshTaskStep {
    SshTaskStep {
        id: "step".to_string(),
        workspace_id: "workspace".to_string(),
        task_id: "task".to_string(),
        name: "Step".to_string(),
        step_type: step_type.to_string(),
        position: 0,
        enabled: true,
        config_version: CONFIG_VERSION_V1,
        config_json,
        created_at: String::new(),
        updated_at: String::new(),
        deleted_at: None,
    }
}

#[test]
fn scans_supported_fields_and_deduplicates_in_first_seen_order() {
    let steps = vec![
        step(
            "command",
            serde_json::json!({
                "command": "docker pull {{source_image}} && echo {{source_image}} {{target_image}}",
                "workingDirectory": "/tmp/{{archive_name}}",
                "timeoutSeconds": 300,
                "continueOnError": false
            }),
        ),
        step(
            "download",
            serde_json::json!({
                "remotePath": "/tmp/{{archive_name}}.tar",
                "localPath": "{{local_output_dir}}/{{archive_name}}.tar",
                "overwrite": true
            }),
        ),
    ];

    assert_eq!(
        detected_inputs(&steps).unwrap(),
        vec![
            "source_image",
            "target_image",
            "archive_name",
            "local_output_dir"
        ]
    );
}

#[test]
fn replaces_placeholders_without_persisting_or_interpreting_values() {
    let steps = vec![step(
        "command",
        serde_json::json!({
            "command": "printf '%s' '{{value}}'",
            "workingDirectory": "{{directory}}",
            "timeoutSeconds": 30,
            "continueOnError": false
        }),
    )];
    let inputs = std::collections::BTreeMap::from([
        ("value".to_string(), "$HOME && literal".to_string()),
        ("directory".to_string(), "/tmp/work".to_string()),
    ]);

    let resolved = resolve_enabled_steps(&steps, &inputs).unwrap();
    assert_eq!(
        resolved[0].config_json["command"],
        "printf '%s' '$HOME && literal'"
    );
    assert_eq!(resolved[0].config_json["workingDirectory"], "/tmp/work");
    assert_eq!(steps[0].config_json["command"], "printf '%s' '{{value}}'");
}

#[test]
fn validates_and_redacts_declared_secret_input_values() {
    let inputs = std::collections::BTreeMap::from([
        ("token".to_string(), "long-secret-value".to_string()),
        ("empty".to_string(), String::new()),
    ]);
    let values = task_secret_values(&inputs, &["token".to_string(), "empty".to_string()]).unwrap();
    assert_eq!(values, vec!["long-secret-value"]);
    assert_eq!(
        redact_task_secret_values("echo long-secret-value", &values),
        "echo <redacted>"
    );
    assert!(task_secret_values(&inputs, &["missing".to_string()]).is_err());
    assert!(task_secret_values(&inputs, &["token".to_string(), "token".to_string()]).is_err());
}

#[test]
fn rejects_missing_invalid_nested_and_unterminated_placeholders() {
    assert!(scan_placeholders("{{valid_name}}").is_ok());
    assert!(scan_placeholders("{{bad.name}}").is_err());
    assert!(scan_placeholders("{{outer_{{inner}}}}").is_err());
    assert!(scan_placeholders("{{missing").is_err());

    let steps = vec![step(
        "upload",
        serde_json::json!({
            "localPath": "{{local_file}}",
            "remotePath": "/tmp/file",
            "overwrite": true
        }),
    )];
    assert!(resolve_enabled_steps(&steps, &std::collections::BTreeMap::new()).is_err());
}

#[test]
fn task_command_allows_newlines_and_tabs_but_rejects_other_controls() {
    assert_eq!(
        validate_task_command("echo one\necho two\r\necho three\t# tab").unwrap(),
        "echo one\necho two\r\necho three\t# tab"
    );
    assert!(validate_task_command("   ").is_err());
    assert!(validate_task_command("echo\0oops").is_err());
    assert!(validate_task_command("echo\u{0007}bell").is_err());

    let multiline = serde_json::json!({
        "command": "echo one\necho two",
        "workingDirectory": "",
        "timeoutSeconds": 30,
        "continueOnError": false
    });
    assert!(validate_step_config("command", CONFIG_VERSION_V1, &multiline).is_ok());

    let bad_cwd = serde_json::json!({
        "command": "true",
        "workingDirectory": "/tmp/bad\npath",
        "timeoutSeconds": 30,
        "continueOnError": false
    });
    let error = validate_step_config("command", CONFIG_VERSION_V1, &bad_cwd).unwrap_err();
    assert!(error
        .to_string()
        .contains("workingDirectory cannot contain control characters"));
}

#[test]
fn parses_all_version_one_configs_and_rejects_unknown_versions() {
    let command = serde_json::json!({
        "command": "true",
        "workingDirectory": "",
        "timeoutSeconds": 30,
        "continueOnError": false
    });
    let upload = serde_json::json!({
        "localPath": "{{local_file}}",
        "remotePath": "/tmp/file",
        "overwrite": true
    });
    let download = serde_json::json!({
        "remotePath": "/tmp/file",
        "localPath": "{{local_file}}",
        "overwrite": true
    });

    assert!(parse_command_config(CONFIG_VERSION_V1, &command).is_ok());
    assert!(parse_upload_config(CONFIG_VERSION_V1, &upload).is_ok());
    assert!(parse_download_config(CONFIG_VERSION_V1, &download).is_ok());
    let error = parse_command_config(99, &command).unwrap_err();
    assert!(error
        .to_string()
        .contains("unsupported SSH task command config version: 99"));
}

#[test]
fn rejects_config_versions_embedded_inside_config_json() {
    let config = serde_json::json!({
        "command": "true",
        "workingDirectory": "",
        "timeoutSeconds": 30,
        "continueOnError": false,
        "version": 1
    });
    let error = validate_step_config("command", CONFIG_VERSION_V1, &config).unwrap_err();
    assert!(error
        .to_string()
        .contains("must be stored in config_version"));
}

#[test]
fn transfer_local_paths_allow_literals_and_placeholder_templates() {
    for local_path in [
        "/Users/alice/archive.tar",
        r"C:\Users\alice\archive.tar",
        "relative/archive.tar",
        "/tmp/{{archive_name}}.tar",
        "{{local_output_dir}}/{{archive_name}}.tar",
    ] {
        let config = serde_json::json!({
            "remotePath": "/tmp/archive.tar",
            "localPath": local_path,
            "overwrite": true
        });
        assert!(
            validate_step_config("download", CONFIG_VERSION_V1, &config).is_ok(),
            "expected localPath {local_path:?} to be accepted"
        );
    }

    let empty = serde_json::json!({
        "remotePath": "/tmp/archive.tar",
        "localPath": "   ",
        "overwrite": true
    });
    let error = validate_step_config("download", CONFIG_VERSION_V1, &empty).unwrap_err();
    assert!(error.to_string().contains("paths cannot be empty"));
}

#[test]
fn canonical_transfer_config_replaces_device_absolute_local_paths() {
    for local_path in [
        "/Users/alice/archive.tar",
        r"C:\Users\alice\archive.tar",
        r"\\server\share\archive.tar",
        "/tmp/{{archive_name}}.tar",
    ] {
        let config = serde_json::json!({
            "remotePath": "/tmp/archive.tar",
            "localPath": local_path,
            "overwrite": true
        });
        let canonical =
            canonical_step_config("step", "download", CONFIG_VERSION_V1, &config).unwrap();
        assert_eq!(
            canonical["localPath"],
            canonical_local_path_placeholder("step")
        );
        assert!(!canonical.to_string().contains(local_path));
    }
    let portable = serde_json::json!({
        "remotePath": "/tmp/archive.tar",
        "localPath": "{{local_output_dir}}/archive.tar",
        "overwrite": true
    });
    assert_eq!(
        canonical_step_config("step", "download", CONFIG_VERSION_V1, &portable).unwrap(),
        portable
    );
}

#[test]
fn canonical_transfer_configs_use_distinct_step_inputs() {
    let config = serde_json::json!({
        "remotePath": "/tmp/archive.tar",
        "localPath": "/Users/alice/archive.tar",
        "overwrite": true
    });
    let first = canonical_step_config("step-a", "upload", CONFIG_VERSION_V1, &config).unwrap();
    let second = canonical_step_config("step-b", "upload", CONFIG_VERSION_V1, &config).unwrap();
    assert_ne!(first["localPath"], second["localPath"]);
    assert_eq!(
        scan_placeholders(first["localPath"].as_str().unwrap())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        scan_placeholders(second["localPath"].as_str().unwrap())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn normalizes_legacy_unknown_fields_but_keeps_new_writes_strict() {
    let config = serde_json::json!({
        "command": "echo ok",
        "workingDirectory": "",
        "timeoutSeconds": 30,
        "continueOnError": false,
        "legacyExtension": "old-value"
    });
    assert!(validate_step_config("command", CONFIG_VERSION_V1, &config).is_err());
    let normalized = normalized_step_config("command", CONFIG_VERSION_V1, &config).unwrap();
    assert_eq!(normalized["command"], "echo ok");
    assert!(normalized.get("legacyExtension").is_none());
}

#[test]
fn restores_current_device_path_from_canonical_placeholder() {
    let incoming = serde_json::json!({
        "remotePath": "/tmp/archive.tar",
        "localPath": canonical_local_path_placeholder("step"),
        "overwrite": true
    });
    let current = serde_json::json!({
        "remotePath": "/tmp/archive.tar",
        "localPath": r"C:\Users\alice\archive.tar",
        "overwrite": true,
        "legacyExtension": "ignored"
    });
    let restored = restore_device_local_step_config(
        "step",
        "download",
        CONFIG_VERSION_V1,
        &incoming,
        "download",
        CONFIG_VERSION_V1,
        &current,
    )
    .unwrap();
    assert_eq!(restored["localPath"], r"C:\Users\alice\archive.tar");
    assert!(restored.get("legacyExtension").is_none());
}

#[test]
fn rejects_unknown_config_fields_that_could_hide_local_or_secret_state() {
    for field in [
        "password",
        "privateKey",
        "credentialRef",
        "connectionId",
        "runtimeInputValue",
        "executionResult",
    ] {
        let mut config = serde_json::json!({
            "command": "echo ok",
            "workingDirectory": "",
            "timeoutSeconds": 30,
            "continueOnError": false
        });
        config[field] = serde_json::Value::String("device-local-value".to_string());
        let error = validate_step_config("command", CONFIG_VERSION_V1, &config).unwrap_err();
        assert!(error.to_string().contains("unsupported"));
    }
}
