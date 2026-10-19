use unfour_core::{AppError, AppResult};

#[path = "diagnostic_arguments.rs"]
mod arguments;

/// Utilities permitted in read-only diagnostic forms. Mixed-purpose utilities
/// also require argument restrictions below and in `diagnostic_arguments.rs`.
const SSH_DIAGNOSTIC_ALLOWED_COMMANDS: &[&str] = &[
    "df",
    "du",
    "free",
    "uptime",
    "uname",
    "hostname",
    "whoami",
    "id",
    "date",
    "ps",
    "ss",
    "netstat",
    "ip",
    "ifconfig",
    "vmstat",
    "iostat",
    "mount",
    "stat",
    "wc",
    "ls",
    "cat",
    "tail",
    "head",
    "grep",
    "systemctl",
    "journalctl",
    "docker",
    "podman",
    "kubectl",
];

/// Read-only `docker`/`podman` subcommands. Anything that can create, mutate,
/// remove, or exec into containers/images is excluded.
const CONTAINER_READONLY_SUBCOMMANDS: &[&str] = &[
    "ps", "logs", "inspect", "images", "version", "info", "stats", "top", "port", "diff",
];

/// Read-only `kubectl` subcommands. Mutating verbs (apply, delete, edit, scale,
/// exec, cp, port-forward, drain, ...) and `config` (which can expose
/// credentials) are excluded.
const KUBECTL_READONLY_SUBCOMMANDS: &[&str] = &[
    "get",
    "describe",
    "logs",
    "top",
    "version",
    "api-resources",
    "explain",
    "cluster-info",
];

/// Validate a one-shot SSH diagnostic command. Returns the trimmed command on
/// success. Enforces: non-empty, length bound, no control characters, no shell
/// metacharacters (so no chaining/piping/redirection/subshells), a bare
/// allowlisted leading utility, and read-only subcommands/flags for utilities
/// that could otherwise mutate state.
pub fn validate_diagnostic_command(command: &str) -> AppResult<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "ssh diagnostic command cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 512 {
        return Err(AppError::Validation(
            "ssh diagnostic command must be 512 characters or fewer".to_string(),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "ssh diagnostic command cannot contain control characters".to_string(),
        ));
    }
    const FORBIDDEN: &[char] = &[
        ';', '|', '&', '$', '`', '>', '<', '(', ')', '{', '}', '\\', '*', '?', '~', '!', '#', '\'',
        '"',
    ];
    if let Some(found) = trimmed.chars().find(|c| FORBIDDEN.contains(c)) {
        return Err(AppError::Validation(format!(
            "ssh diagnostic command cannot contain the shell metacharacter `{}`",
            found
        )));
    }

    let mut tokens = trimmed.split_whitespace();
    let head = tokens.next().unwrap_or_default();
    if head.contains('/') {
        return Err(AppError::Validation(
            "ssh diagnostic command must be a bare allowlisted utility (no path)".to_string(),
        ));
    }
    if !SSH_DIAGNOSTIC_ALLOWED_COMMANDS.contains(&head) {
        return Err(AppError::Validation(format!(
            "`{}` is not an allowed read-only diagnostic command",
            head
        )));
    }

    if !arguments::readonly_arguments(head, &tokens.clone().collect::<Vec<_>>()) {
        return Err(AppError::Validation(format!(
            "`{head}` arguments are not an allowed read-only diagnostic form"
        )));
    }
    match head {
        "systemctl" => {
            const SYSTEMCTL_READONLY: &[&str] = &[
                "status",
                "is-active",
                "is-enabled",
                "is-failed",
                "show",
                "cat",
                "list-units",
                "list-unit-files",
                "list-timers",
                "list-sockets",
                "get-default",
            ];
            let sub = tokens.next().unwrap_or("");
            if !SYSTEMCTL_READONLY.contains(&sub) {
                return Err(AppError::Validation(
                    "systemctl diagnostics require a read-only subcommand placed first (status, is-active, show, list-units, ...)".to_string(),
                ));
            }
        }
        // For container CLIs the subcommand must come first (e.g. `docker logs
        // -n 100 web`, `kubectl get pods -n prod`). Global flags before the
        // subcommand are rejected so a value-taking flag (`-n ns`, `-H host`)
        // cannot disguise a mutating verb as its argument.
        "docker" | "podman" => {
            let sub = tokens.next().unwrap_or("");
            if !CONTAINER_READONLY_SUBCOMMANDS.contains(&sub) {
                return Err(AppError::Validation(
                    "docker/podman diagnostics are limited to read-only subcommands placed first (ps, logs, inspect, images, stats, ...)".to_string(),
                ));
            }
        }
        "kubectl" => {
            let sub = tokens.next().unwrap_or("");
            if !KUBECTL_READONLY_SUBCOMMANDS.contains(&sub)
                || (sub == "cluster-info" && tokens.next().is_some())
            {
                return Err(AppError::Validation(
                    "kubectl diagnostics are limited to read-only subcommands placed first (get, describe, logs, top, ...)".to_string(),
                ));
            }
        }
        _ => {}
    }

    Ok(trimmed.to_string())
}

/// Validate a broader one-shot SSH command. The MCP layer applies workspace
/// policy and high-risk confirmation before this reaches the engine; the engine
/// still enforces basic shape limits before opening a remote channel.
pub(crate) fn validate_one_shot_command(command: &str) -> AppResult<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "ssh command cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 4096 {
        return Err(AppError::Validation(
            "ssh command must be 4096 characters or fewer".to_string(),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "ssh command cannot contain control characters".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}
