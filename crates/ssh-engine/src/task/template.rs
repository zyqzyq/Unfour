use super::*;

const TEMPLATE_FIELDS: &[(&str, &[&str])] = &[
    ("command", &["command", "workingDirectory"]),
    ("upload", &["localPath", "remotePath"]),
    ("download", &["remotePath", "localPath"]),
];
const CONFIG_FIELDS: &[(&str, &[&str])] = &[
    (
        "command",
        &[
            "command",
            "workingDirectory",
            "timeoutSeconds",
            "continueOnError",
        ],
    ),
    ("upload", &["localPath", "remotePath", "overwrite"]),
    ("download", &["remotePath", "localPath", "overwrite"]),
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

pub(super) fn task_secret_values(
    inputs: &std::collections::BTreeMap<String, String>,
    secret_input_names: &[String],
) -> AppResult<Vec<String>> {
    let mut names = std::collections::HashSet::new();
    let mut values = Vec::new();
    for name in secret_input_names {
        if !names.insert(name.as_str()) {
            return Err(AppError::Validation(
                "SSH task secret input names cannot contain duplicates".to_string(),
            ));
        }
        if !valid_variable_name(name) || !inputs.contains_key(name) {
            return Err(AppError::Validation(format!(
                "SSH task secret input name is not present in inputs: {name}"
            )));
        }
        let value = inputs[name].as_str();
        if !value.is_empty() && !values.iter().any(|item| item == value) {
            values.push(value.to_string());
        }
    }
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    Ok(values)
}

#[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
pub(super) fn redact_task_secret_values(value: &str, secret_values: &[String]) -> String {
    secret_values
        .iter()
        .fold(value.to_string(), |redacted, secret| {
            redacted.replace(secret, unfour_core::redaction::REDACTED_VALUE)
        })
}

pub(super) fn validate_step_config(
    step_type: &str,
    config_version: i64,
    config: &serde_json::Value,
) -> AppResult<()> {
    let object = step_config_object(config)?;
    let allowed_fields = config_fields_for_type(step_type)?;
    if let Some(field) = object
        .keys()
        .find(|field| !allowed_fields.contains(&field.as_str()))
    {
        return Err(AppError::Validation(format!(
            "unsupported SSH task {step_type} config field: {field}"
        )));
    }
    normalized_step_config(step_type, config_version, config).map(|_| ())
}

/// Parse and validate the known versioned fields while dropping unknown
/// legacy keys. Reads use this compatibility path so configs accepted by an
/// older release remain usable; all new writes still call
/// [`validate_step_config`] first and therefore reject unknown fields.
pub(super) fn normalized_step_config(
    step_type: &str,
    config_version: i64,
    config: &serde_json::Value,
) -> AppResult<serde_json::Value> {
    let _ = step_config_object(config)?;
    let normalized = match step_type {
        "command" => {
            let parsed = parse_command_config(config_version, config)?;
            validate_task_command(&parsed.command)?;
            if !parsed.working_directory.is_empty()
                && parsed.working_directory.chars().any(char::is_control)
            {
                return Err(AppError::Validation(
                    "Command workingDirectory cannot contain control characters".to_string(),
                ));
            }
            if !(1..=3_600).contains(&parsed.timeout_seconds) {
                return Err(AppError::Validation(
                    "Command timeoutSeconds must be between 1 and 3600".to_string(),
                ));
            }
            serde_json::to_value(parsed)?
        }
        "upload" => {
            let parsed = parse_upload_config(config_version, config)?;
            validate_transfer_paths(&parsed.local_path, &parsed.remote_path)?;
            serde_json::to_value(parsed)?
        }
        "download" => {
            let parsed = parse_download_config(config_version, config)?;
            validate_transfer_paths(&parsed.local_path, &parsed.remote_path)?;
            serde_json::to_value(parsed)?
        }
        _ => {
            return Err(AppError::Validation(format!(
                "unsupported SSH task step type: {step_type}"
            )));
        }
    };
    for value in normalized
        .as_object()
        .into_iter()
        .flat_map(|object| object.values())
    {
        if let Some(value) = value.as_str() {
            scan_placeholders(value)?;
        }
    }
    Ok(normalized)
}

fn step_config_object(
    config: &serde_json::Value,
) -> AppResult<&serde_json::Map<String, serde_json::Value>> {
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
    Ok(object)
}

/// Validate an SSH task command step. Unlike one-shot SSH exec validation,
/// task scripts may include newlines and tabs; other control characters (NUL,
/// bell, etc.) remain rejected.
pub(super) fn validate_task_command(command: &str) -> AppResult<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "Command step requires a command".to_string(),
        ));
    }
    if trimmed.chars().count() > 4096 {
        return Err(AppError::Validation(
            "ssh task command must be 4096 characters or fewer".to_string(),
        ));
    }
    if trimmed
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return Err(AppError::Validation(
            "ssh task command cannot contain control characters other than newlines and tabs"
                .to_string(),
        ));
    }
    Ok(trimmed.to_string())
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
    let _ = scan_placeholders(local_path.trim())?;
    let _ = scan_placeholders(remote_path.trim())?;
    Ok(())
}

pub(super) fn canonical_step_config(
    step_id: &str,
    step_type: &str,
    config_version: i64,
    config: &serde_json::Value,
) -> AppResult<serde_json::Value> {
    let mut normalized = normalized_step_config(step_type, config_version, config)?;
    if matches!(step_type, "upload" | "download") {
        let local_path = normalized
            .get("localPath")
            .and_then(serde_json::Value::as_str)
            .expect("normalized transfer config has localPath");
        if is_device_absolute_path(local_path.trim()) {
            normalized["localPath"] =
                serde_json::Value::String(canonical_local_path_placeholder(step_id));
        }
    }
    Ok(normalized)
}

/// Merge an incoming sync-safe config with the current device's local transfer
/// path. A canonical placeholder signals that the sender intentionally omitted
/// its device path; it must never erase a value already configured here.
pub(super) fn restore_device_local_step_config(
    step_id: &str,
    incoming_step_type: &str,
    incoming_config_version: i64,
    incoming: &serde_json::Value,
    current_step_type: &str,
    current_config_version: i64,
    current: &serde_json::Value,
) -> AppResult<serde_json::Value> {
    let mut restored =
        normalized_step_config(incoming_step_type, incoming_config_version, incoming)?;
    if !matches!(incoming_step_type, "upload" | "download") {
        return Ok(restored);
    }
    let incoming_local_path = restored
        .get("localPath")
        .and_then(serde_json::Value::as_str)
        .expect("normalized transfer config has localPath");
    if !is_canonical_local_path_placeholder(step_id, incoming_local_path) {
        return Ok(restored);
    }
    if !matches!(current_step_type, "upload" | "download") {
        return Ok(restored);
    }
    let current = normalized_step_config(current_step_type, current_config_version, current)?;
    let current_local_path = current
        .get("localPath")
        .and_then(serde_json::Value::as_str)
        .expect("normalized transfer config has localPath");
    if !is_canonical_local_path_placeholder(step_id, current_local_path) {
        restored["localPath"] = serde_json::Value::String(current_local_path.to_string());
    }
    Ok(restored)
}

pub(super) fn canonical_local_path_placeholder(step_id: &str) -> String {
    use std::fmt::Write;

    let mut placeholder = String::with_capacity(15 + step_id.len() * 2);
    placeholder.push_str("{{local_path_");
    for byte in step_id.as_bytes() {
        write!(&mut placeholder, "{byte:02x}").expect("writing to a String cannot fail");
    }
    placeholder.push_str("}}");
    placeholder
}

fn is_canonical_local_path_placeholder(step_id: &str, value: &str) -> bool {
    value == "{{local_path}}" || value == canonical_local_path_placeholder(step_id)
}

fn is_device_absolute_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('\\')
        || value
            .as_bytes()
            .get(1)
            .is_some_and(|separator| *separator == b':')
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

fn config_fields_for_type(step_type: &str) -> AppResult<&'static [&'static str]> {
    CONFIG_FIELDS
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
#[path = "template_tests.rs"]
mod template_tests;
