use super::*;

const TEMPLATE_FIELDS: &[(&str, &[&str])] = &[
    ("command", &["command", "workingDirectory"]),
    ("upload", &["localPath", "remotePath"]),
    ("download", &["remotePath", "localPath"]),
];
const CONFIG_VERSION_V1: i64 = 1;

pub(super) fn detected_inputs(steps: &[SshTaskStep]) -> AppResult<Vec<String>> {
    let mut variables = Vec::new();
    for step in steps {
        for value in template_values(step)? {
            for variable in scan_placeholders(value)? {
                if !variables.contains(&variable) {
                    variables.push(variable);
                }
            }
        }
    }
    Ok(variables)
}

pub(super) fn resolve_enabled_steps(
    steps: &[SshTaskStep],
    inputs: &std::collections::BTreeMap<String, String>,
) -> AppResult<Vec<SshTaskStep>> {
    let enabled = steps
        .iter()
        .filter(|step| step.enabled)
        .cloned()
        .collect::<Vec<_>>();
    let required = detected_inputs(&enabled)?;
    let missing = required
        .iter()
        .filter(|name| {
            inputs
                .get(*name)
                .is_none_or(|value| value.trim().is_empty())
        })
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "missing SSH task inputs: {}",
            missing.join(", ")
        )));
    }

    enabled
        .into_iter()
        .map(|mut step| {
            let fields = fields_for_type(&step.step_type)?;
            let object = step.config_json.as_object_mut().ok_or_else(|| {
                AppError::Validation(format!(
                    "SSH task step '{}' config must be a JSON object",
                    step.name
                ))
            })?;
            for field in fields {
                if let Some(value) = object.get_mut(*field) {
                    let string = value.as_str().ok_or_else(|| {
                        AppError::Validation(format!(
                            "SSH task step '{}' field '{}' must be a string",
                            step.name, field
                        ))
                    })?;
                    *value = serde_json::Value::String(replace_placeholders(string, inputs)?);
                }
            }
            Ok(step)
        })
        .collect()
}

pub(super) fn validate_step_config(
    step_type: &str,
    config_version: i64,
    config: &serde_json::Value,
) -> AppResult<()> {
    let object = config.as_object().ok_or_else(|| {
        AppError::Validation("SSH task step config must be a JSON object".to_string())
    })?;
    if ["version", "configVersion", "config_version"]
        .iter()
        .any(|field| object.contains_key(*field))
    {
        return Err(AppError::Validation(
            "SSH task step config version must be stored in config_version, not config_json"
                .to_string(),
        ));
    }
    match step_type {
        "command" => {
            let parsed = parse_command_config(config_version, config)?;
            if parsed.command.trim().is_empty() {
                return Err(AppError::Validation(
                    "Command step requires a command".to_string(),
                ));
            }
            if !(1..=3_600).contains(&parsed.timeout_seconds) {
                return Err(AppError::Validation(
                    "Command timeoutSeconds must be between 1 and 3600".to_string(),
                ));
            }
        }
        "upload" => {
            let parsed = parse_upload_config(config_version, config)?;
            validate_transfer_paths(&parsed.local_path, &parsed.remote_path)?;
        }
        "download" => {
            let parsed = parse_download_config(config_version, config)?;
            validate_transfer_paths(&parsed.local_path, &parsed.remote_path)?;
        }
        _ => {
            return Err(AppError::Validation(format!(
                "unsupported SSH task step type: {step_type}"
            )));
        }
    }
    for value in config
        .as_object()
        .into_iter()
        .flat_map(|object| object.values())
    {
        if let Some(value) = value.as_str() {
            scan_placeholders(value)?;
        }
    }
    Ok(())
}

pub(super) fn parse_command_config(
    config_version: i64,
    config: &serde_json::Value,
) -> AppResult<SshTaskCommandConfig> {
    require_config_version("command", config_version)?;
    serde_json::from_value(config.clone())
        .map_err(|error| AppError::Validation(format!("invalid Command step config: {error}")))
}

pub(super) fn parse_upload_config(
    config_version: i64,
    config: &serde_json::Value,
) -> AppResult<SshTaskUploadConfig> {
    require_config_version("upload", config_version)?;
    serde_json::from_value(config.clone())
        .map_err(|error| AppError::Validation(format!("invalid Upload step config: {error}")))
}

pub(super) fn parse_download_config(
    config_version: i64,
    config: &serde_json::Value,
) -> AppResult<SshTaskDownloadConfig> {
    require_config_version("download", config_version)?;
    serde_json::from_value(config.clone())
        .map_err(|error| AppError::Validation(format!("invalid Download step config: {error}")))
}

fn require_config_version(step_type: &str, config_version: i64) -> AppResult<()> {
    if config_version != CONFIG_VERSION_V1 {
        return Err(AppError::Validation(format!(
            "unsupported SSH task {step_type} config version: {config_version}"
        )));
    }
    Ok(())
}

fn validate_transfer_paths(local_path: &str, remote_path: &str) -> AppResult<()> {
    if local_path.trim().is_empty() || remote_path.trim().is_empty() {
        return Err(AppError::Validation(
            "Upload and Download paths cannot be empty".to_string(),
        ));
    }
    validate_local_template_path(local_path)?;
    Ok(())
}

fn validate_local_template_path(local_path: &str) -> AppResult<()> {
    let local_path = local_path.trim();
    let first_variable = scan_placeholders(local_path)?
        .into_iter()
        .next()
        .ok_or_else(|| {
            AppError::Validation(
                "Upload and Download localPath must begin with a runtime placeholder".to_string(),
            )
        })?;
    if !local_path.starts_with(&format!("{{{{{first_variable}}}}}")) {
        return Err(AppError::Validation(
            "Upload and Download localPath must begin with a runtime placeholder".to_string(),
        ));
    }
    Ok(())
}

fn template_values(step: &SshTaskStep) -> AppResult<Vec<&str>> {
    let fields = fields_for_type(&step.step_type)?;
    let object = step.config_json.as_object().ok_or_else(|| {
        AppError::Validation(format!(
            "SSH task step '{}' config must be a JSON object",
            step.name
        ))
    })?;
    fields
        .iter()
        .filter_map(|field| object.get(*field).map(|value| (*field, value)))
        .map(|(field, value)| {
            value.as_str().ok_or_else(|| {
                AppError::Validation(format!(
                    "SSH task step '{}' field '{}' must be a string",
                    step.name, field
                ))
            })
        })
        .collect()
}

fn fields_for_type(step_type: &str) -> AppResult<&'static [&'static str]> {
    TEMPLATE_FIELDS
        .iter()
        .find(|(kind, _)| *kind == step_type)
        .map(|(_, fields)| *fields)
        .ok_or_else(|| AppError::Validation(format!("unsupported SSH task step type: {step_type}")))
}

pub(super) fn scan_placeholders(value: &str) -> AppResult<Vec<String>> {
    let mut variables = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = value[cursor..].find("{{") {
        let start = cursor + relative_start;
        let content_start = start + 2;
        let Some(relative_end) = value[content_start..].find("}}") else {
            return Err(AppError::Validation(
                "unterminated SSH task placeholder".to_string(),
            ));
        };
        let end = content_start + relative_end;
        let variable = &value[content_start..end];
        if !valid_variable_name(variable) {
            return Err(AppError::Validation(format!(
                "invalid SSH task placeholder: {{{{{variable}}}}}"
            )));
        }
        if !variables.iter().any(|item| item == variable) {
            variables.push(variable.to_string());
        }
        cursor = end + 2;
    }
    Ok(variables)
}

fn replace_placeholders(
    value: &str,
    inputs: &std::collections::BTreeMap<String, String>,
) -> AppResult<String> {
    let mut result = value.to_string();
    for variable in scan_placeholders(value)? {
        let replacement = inputs
            .get(&variable)
            .ok_or_else(|| AppError::Validation(format!("missing SSH task input: {variable}")))?;
        result = result.replace(&format!("{{{{{variable}}}}}"), replacement);
    }
    Ok(result)
}

fn valid_variable_name(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
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
    fn transfer_local_paths_must_be_placeholder_led_templates() {
        for local_path in [
            "/Users/alice/archive.tar",
            r"C:\Users\alice\archive.tar",
            "relative/archive.tar",
            "/tmp/{{archive_name}}.tar",
        ] {
            let config = serde_json::json!({
                "remotePath": "/tmp/archive.tar",
                "localPath": local_path,
                "overwrite": true
            });
            let error = validate_step_config("download", CONFIG_VERSION_V1, &config).unwrap_err();
            assert!(error
                .to_string()
                .contains("localPath must begin with a runtime placeholder"));
        }

        let portable = serde_json::json!({
            "remotePath": "/tmp/{{archive_name}}.tar",
            "localPath": "{{local_output_dir}}/{{archive_name}}.tar",
            "overwrite": true
        });
        assert!(validate_step_config("download", CONFIG_VERSION_V1, &portable).is_ok());
    }
}
