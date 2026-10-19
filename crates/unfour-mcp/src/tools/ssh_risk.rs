use serde_json::{Map, Value};

use super::ToolCallError;

pub(super) fn redact_command_display(command: &str) -> String {
    let lower = command.to_ascii_lowercase();
    let sensitive_terms = [
        "authorization",
        "cookie",
        "proxy-authorization",
        "x-api-key",
        "x-auth-token",
        "password",
        "passwd",
        "pwd=",
        "token",
        "api_key",
        "apikey",
        "secret",
        "private_key",
        "credential_ref",
        "connection_string",
        "database_url",
    ];

    if sensitive_terms.iter().any(|term| lower.contains(term)) {
        "[redacted command]".to_string()
    } else {
        command.to_string()
    }
}

pub(super) fn parse_optional_u64(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<u64>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::Number(n)) => {
            let value = n.as_u64().ok_or_else(|| {
                ToolCallError::InvalidArguments(format!(
                    "argument `{}` must be a positive integer",
                    key
                ))
            })?;
            Ok(Some(value))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a number",
            key
        ))),
    }
}

pub(super) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn shell_env_key(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>()
}

pub(super) fn build_ssh_exec_command(
    command: &str,
    cwd: Option<&str>,
    env: Option<&Map<String, Value>>,
) -> String {
    let mut command = command.to_string();
    if let Some(cwd) = cwd {
        command = format!("cd {} && {}", shell_quote(cwd), command);
    }
    if let Some(env) = env {
        let prefix = env
            .iter()
            .map(|(key, value)| {
                let raw = value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string());
                format!("{}={}", shell_env_key(key), shell_quote(&raw))
            })
            .collect::<Vec<_>>()
            .join(" ");
        if !prefix.is_empty() {
            command = format!("{prefix} {command}");
        }
    }
    command
}

fn shell_words(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|part| {
            part.trim_matches(|c: char| {
                matches!(c, '\'' | '"' | ';' | '|' | '&' | '(' | ')' | '{' | '}')
            })
            .to_ascii_lowercase()
        })
        .filter(|part| !part.is_empty())
        .collect()
}

pub(super) fn classify_high_risk_command(command: &str) -> Option<(&'static str, &'static str)> {
    let lower = command.to_ascii_lowercase();
    let words = shell_words(command);
    if lower.contains("rm -rf") || words.first().is_some_and(|word| word == "rm") {
        return Some(("SSH_DELETE_COMMAND", "rm/rm -rf can delete remote files."));
    }
    if lower.contains("curl ") && (lower.contains("| sh") || lower.contains("| bash")) {
        return Some((
            "SSH_CURL_PIPE_SHELL",
            "Piping downloaded scripts into a shell is dangerous.",
        ));
    }
    if words.iter().any(|word| {
        matches!(
            word.as_str(),
            "reboot" | "shutdown" | "halt" | "poweroff" | "kill" | "killall" | "pkill"
        )
    }) {
        return Some((
            "SSH_PROCESS_OR_POWER_CONTROL",
            "Process termination or power-control commands require confirmation.",
        ));
    }
    if lower.contains("systemctl restart")
        || lower.contains("systemctl stop")
        || lower.contains("service restart")
        || lower.contains("service stop")
    {
        return Some((
            "SSH_SERVICE_CONTROL",
            "Service restart/stop commands can disrupt a remote host.",
        ));
    }
    if lower.contains("docker rm")
        || lower.contains("docker compose down")
        || lower.contains("podman rm")
        || lower.contains("kubectl delete")
    {
        return Some((
            "SSH_ORCHESTRATOR_DELETE",
            "Container or Kubernetes delete/down commands require confirmation.",
        ));
    }
    if lower.contains(" > /etc/")
        || lower.contains(" >/etc/")
        || lower.contains(" > /usr/")
        || lower.contains(" >/usr/")
        || lower.contains(" > c:\\windows")
    {
        return Some((
            "SSH_SYSTEM_PATH_WRITE",
            "Writing to system directories requires confirmation.",
        ));
    }
    None
}

pub(super) fn is_readonly_ssh_command(command: &str) -> bool {
    unfour_command_bus::validate_diagnostic_command(command).is_ok()
}

pub(super) fn is_sensitive_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    lower.starts_with("/etc/")
        || lower.starts_with("/usr/")
        || lower.starts_with("/bin/")
        || lower.starts_with("/sbin/")
        || lower.starts_with("/boot/")
        || lower.starts_with("c:/windows/")
        || lower.ends_with("/.ssh/authorized_keys")
}
