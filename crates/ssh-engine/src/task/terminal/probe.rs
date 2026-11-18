use super::*;
use crate::ssh::temporary::TemporaryProbeCheck;

/// This proves only current-terminal shell execution, never target identity.
pub(in crate::ssh) async fn run_probe(
    lease: TerminalLease,
    mut cancel: watch::Receiver<bool>,
) -> Vec<TemporaryProbeCheck> {
    let mut driver = TerminalTaskDriver::new(lease);
    let mut checks = Vec::new();
    for (id, expected) in [("terminal-success", 0), ("terminal-nonzero", 23)] {
        let step = SshTaskStep {
            id: id.into(),
            workspace_id: String::new(),
            task_id: String::new(),
            name: id.into(),
            step_type: "command".into(),
            position: 0,
            enabled: true,
            config_version: 1,
            config_json: serde_json::json!({"command":format!("printf 'UNFOUR_TERMINAL_PROBE\\n'; exit {expected}"), "workingDirectory":"", "timeoutSeconds":5, "continueOnError":false}),
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        };
        let mut output = String::new();
        let result = driver
            .execute_step(&step, &mut cancel, &mut |event| {
                if let DriverEvent::Output { stream, data } = event {
                    if stream == "stdout" && output.len() < 2048 {
                        output.push_str(&data);
                    }
                }
            })
            .await;
        let actual = match &result {
            Ok(result) => result.exit_code,
            Err(TaskStepError::Failed { exit_code, .. }) => *exit_code,
            _ => None,
        };
        let code = if actual == Some(expected) {
            if output.replace('\r', "") == "UNFOUR_TERMINAL_PROBE\n" {
                "OK"
            } else {
                "OUTPUT_MISMATCH"
            }
        } else {
            match result {
                Err(TaskStepError::Cancelled)
                | Err(TaskStepError::OutcomeUncertain { cancelled: true }) => "CANCELLED",
                Err(TaskStepError::OutcomeUncertain { cancelled: false }) => "TIMEOUT",
                Err(TaskStepError::InputUnconfirmed) => "TERMINAL_PROTOCOL_ERROR",
                Err(TaskStepError::Failed { ref message, .. })
                    if message == "SSH_TASK_TERMINAL_NOT_READY" =>
                {
                    "TERMINAL_NOT_READY"
                }
                Err(TaskStepError::Failed { ref message, .. })
                    if message == "SSH_TASK_INTERACTIVE_LOGIN_PROMPT" =>
                {
                    "INTERACTIVE_LOGIN_PROMPT"
                }
                Err(TaskStepError::Failed { ref message, .. })
                    if message == "SSH_TASK_TERMINAL_OUTPUT_OVERFLOW" =>
                {
                    "OUTPUT_LIMIT"
                }
                _ if actual.is_some() => "EXIT_STATUS_MISMATCH",
                _ => "TRANSPORT_CLOSED",
            }
        };
        checks.push(TemporaryProbeCheck {
            id: id.into(),
            status: if code == "OK" { "pass" } else { "inconclusive" }.into(),
            code: code.into(),
            expected_exit_code: Some(expected as u32),
            actual_exit_code: actual.map(|code| code as u32),
        });
        if code != "OK" {
            break;
        }
    }
    if !driver.finish().await {
        checks.push(TemporaryProbeCheck {
            id: "terminal-recovery".into(),
            status: "inconclusive".into(),
            code: "TERMINAL_RECOVERY_REQUIRED".into(),
            expected_exit_code: None,
            actual_exit_code: None,
        });
    }
    checks
}
