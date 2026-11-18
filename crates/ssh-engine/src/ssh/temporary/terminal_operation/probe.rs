use super::*;
use tokio::sync::watch;
use unfour_core::terminal_curl_contract::TerminalCurlRequest;

const TRANSPORT_SUFFIX: &[u8] = b"\n\0\x01\x1e\x1f\x7f\xff";

fn transport_payload() -> (String, Vec<u8>) {
    // More than one kilobyte of deterministic input exercises many chunks.
    // printf emits that exact input plus binary bytes; no curl or HTTP is used.
    let text = format!(
        "UNFOUR_TRANSPORT_ROUNDTRIP:{}",
        "0123456789abcdef".repeat(60)
    );
    let command = format!(
        "command printf '%s' '{text}'; command printf '\\n\\000\\001\\036\\037\\177\\377'; exit 23"
    );
    let mut expected = text.into_bytes();
    expected.extend_from_slice(TRANSPORT_SUFFIX);
    (command, expected)
}

pub(in crate::ssh) async fn run(
    mut lease: terminal::TerminalLease,
    cancelled: watch::Receiver<bool>,
    kind: TemporaryProbeKind,
) -> Vec<TemporaryProbeCheck> {
    let cancellation = CancellationToken::new();
    if *cancelled.borrow() || cancelled.has_changed().is_err() {
        cancellation.cancel();
    }
    let token = cancellation.clone();
    let mut observed = cancelled.clone();
    let monitor = tokio::spawn(async move {
        let _ = observed.wait_for(|value| *value).await;
        token.cancel();
    });
    let mut checks = Vec::new();
    let (command, expected) = transport_payload();
    let (passed, mut restored) = observe(
        &mut lease,
        Operation::Command(&command),
        &expected,
        23,
        "transport-roundtrip",
        "transport-recovery",
        cancellation.clone(),
        &mut checks,
    )
    .await;
    // The lease stays exclusively owned across both stages. A successful byte
    // roundtrip alone is not enough: recovery must also be confirmed first.
    if kind == TemporaryProbeKind::Curl
        && passed
        && restored
        && !cancellation.is_cancelled()
        && !*cancelled.borrow()
        && cancelled.has_changed().is_ok()
    {
        // One fixed capacity sample. Echo only synthetic bytes and never run HTTP.
        let sample = TerminalCurlRequest {
            id: Uuid::new_v4(),
            config: (0..driver::SAMPLE_BYTES)
                .map(|i| char::from(b'A' + ((i + i / 83) % 26) as u8))
                .collect(),
            needs_data_raw: false,
        };
        let (matched, curl_restored) = observe(
            &mut lease,
            Operation::Probe(&sample),
            sample.config.as_bytes(),
            0,
            "curl-tools",
            "curl-recovery",
            cancellation,
            &mut checks,
        )
        .await;
        if !matched || !curl_restored {
            lease.curl_context = None;
        }
        restored = curl_restored;
    }
    lease.release(restored);
    monitor.abort();
    checks
}

async fn observe(
    lease: &mut terminal::TerminalLease,
    operation: Operation<'_>,
    expected: &[u8],
    expected_exit: u32,
    id: &str,
    recovery_id: &str,
    cancellation: CancellationToken,
    checks: &mut Vec<TemporaryProbeCheck>,
) -> (bool, bool) {
    let (result, restoration, _) =
        driver::run_report_borrowed(lease, operation, Some(30_000), cancellation, 4096).await;
    let (code, actual) = match result {
        Ok((stdout, exit_code)) => {
            let code = match (id, exit_code) {
                (_, code) if code == expected_exit as i32 && stdout == expected => "OK",
                (_, code) if code == expected_exit as i32 => "OUTPUT_MISMATCH",
                ("curl-tools", 127) => "CURL_MISSING",
                ("curl-tools", 126) => "TERMINAL_TOOLS_MISSING",
                ("curl-tools", 125) => "CURL_OPTIONS_UNSUPPORTED",
                _ => "EXIT_STATUS_MISMATCH",
            };
            (code, Some(exit_code as u32))
        }
        Err(code) => (
            match code {
                "SSH_TERMINAL_BASE64_MISSING" | "SSH_TERMINAL_PRINTF_UNSUPPORTED" => {
                    "TERMINAL_TOOLS_MISSING"
                }
                "SSH_TERMINAL_HTTP_CURL_UNAVAILABLE" => "CURL_MISSING",
                "SSH_TERMINAL_HTTP_CURL_UNSUPPORTED" => "CURL_OPTIONS_UNSUPPORTED",
                "SSH_TERMINAL_CANCELLED" => "CANCELLED",
                "SSH_TERMINAL_TIMEOUT" => "TIMEOUT",
                "SSH_TERMINAL_STARTUP_TIMEOUT"
                | "SSH_TERMINAL_READY_TIMEOUT"
                | "SSH_TERMINAL_ACCEPT_TIMEOUT"
                | "SSH_TERMINAL_INPUT_ACK_TIMEOUT"
                | "SSH_TERMINAL_BEGIN_TIMEOUT"
                | "SSH_TERMINAL_END_TIMEOUT" => code,
                "SSH_TERMINAL_LOGIN_REQUIRED" => "INTERACTIVE_LOGIN_PROMPT",
                "SSH_TERMINAL_OUTPUT_LIMIT" => "OUTPUT_LIMIT",
                "SSH_TERMINAL_PROTOCOL_ERROR" => "TERMINAL_PROTOCOL_ERROR",
                _ => "TERMINAL_NOT_READY",
            },
            None,
        ),
    };
    checks.push(TemporaryProbeCheck {
        id: id.into(),
        status: match code {
            "OK" => "pass",
            "CURL_MISSING" | "TERMINAL_TOOLS_MISSING" | "CURL_OPTIONS_UNSUPPORTED" => "unsupported",
            "CANCELLED" => "cancelled",
            "OUTPUT_MISMATCH" | "EXIT_STATUS_MISMATCH" => "fail",
            _ => "inconclusive",
        }
        .into(),
        code: code.into(),
        expected_exit_code: Some(expected_exit),
        actual_exit_code: actual,
    });
    checks.push(TemporaryProbeCheck {
        id: recovery_id.into(),
        status: if restoration.is_ok() {
            "pass"
        } else {
            "inconclusive"
        }
        .into(),
        code: restoration.err().unwrap_or("OK").into(),
        expected_exit_code: None,
        actual_exit_code: None,
    });
    (code == "OK", restoration.is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_probe_is_fixed_large_and_independent_of_curl_or_network() {
        let (command, expected) = transport_payload();
        assert!((1024..1200).contains(&command.len()));
        assert!(command.ends_with("exit 23"));
        assert!(!command.contains("curl") && !command.contains("http"));
        assert!(expected.ends_with(TRANSPORT_SUFFIX));
        assert!(expected.len() > 960);
    }
}
