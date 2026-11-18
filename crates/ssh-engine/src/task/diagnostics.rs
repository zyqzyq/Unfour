use super::*;

fn owned_code(value: &str) -> bool {
    matches!(
        value,
        SSH_TASK_OUTCOME_UNCERTAIN
            | "SSH_TASK_INTERACTIVE_LOGIN_PROMPT"
            | "SSH_TASK_OUTPUT_INCOMPLETE"
            | "SSH_TASK_TERMINAL_RECOVERY_REQUIRED"
            | "SSH_TASK_TERMINAL_NOT_READY"
            | "SSH_TASK_TERMINAL_INPUT_UNCONFIRMED"
            | "SSH_TASK_TERMINAL_COMMAND_ONLY"
            | "SSH_TASK_TERMINAL_OUTPUT_OVERFLOW"
            | "SSH_TASK_EXEC_QUEUE_TIMEOUT"
    )
}

/// Only exact locally emitted identifiers bypass value redaction. Remote
/// stdout/stderr, or arbitrary text in a diagnostic event, is still redacted.
pub(super) fn redact_driver_output(stream: &str, data: String, secrets: &[String]) -> String {
    if stream == "diagnostic" && owned_code(data.strip_suffix('\n').unwrap_or(&data)) {
        data
    } else {
        redact_task_secret_values(&data, secrets)
    }
}

pub(super) fn redact_task_error(value: String, secrets: &[String]) -> String {
    if owned_code(&value) {
        value
    } else {
        redact_task_secret_values(&value, secrets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_diagnostics_survive_short_secret_values_but_remote_text_does_not() {
        let secrets = vec!["SSH".into(), "private-value".into()];
        for code in [
            "SSH_TASK_INTERACTIVE_LOGIN_PROMPT",
            "SSH_TASK_OUTPUT_INCOMPLETE",
            "SSH_TASK_TERMINAL_RECOVERY_REQUIRED",
        ] {
            let data = format!("{code}\n");
            assert_eq!(
                redact_driver_output("diagnostic", data.clone(), &secrets),
                data
            );
            assert_eq!(redact_task_error(code.into(), &secrets), code);
            assert!(!redact_driver_output("stdout", data, &secrets).contains("SSH"));
        }
        for text in [
            "SSH_TASK_INTERACTIVE_LOGIN_PROMPT private-value",
            "SSH_TASK_INTERACTIVE_LOGIN_PROMPT\nprivate-value",
            "SSH_TASK_UNKNOWN private-value",
        ] {
            assert!(!redact_driver_output("diagnostic", text.into(), &secrets)
                .contains("private-value"));
            assert!(!redact_task_error(text.into(), &secrets).contains("private-value"));
        }
    }
}
