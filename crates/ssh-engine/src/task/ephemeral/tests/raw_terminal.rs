use super::*;
use tokio_util::sync::CancellationToken;
#[path = "raw_terminal_failures.rs"]
mod failures;
#[path = "raw_terminal_input.rs"]
mod input_delivery;
#[path = "raw_terminal_probes.rs"]
mod probes;

#[path = "raw_terminal_latency.rs"]
mod latency;

async fn raw_fixture() -> Fixture {
    let fixture = fixture().await;
    fixture.state.lock().unwrap().terminal_enabled = true;
    fixture
}

#[tokio::test]
async fn raw_terminal_preserves_large_binary_output_nonzero_and_parent_session() {
    let f = raw_fixture().await;
    let before: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    let operation = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap();
    assert!(f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .is_err());
    let result = operation
        .run("raw-large", Some(10_000), CancellationToken::new(), 160_000)
        .await
        .unwrap();
    assert_eq!(
        result.stdout,
        (0..=255).cycle().take(150_000).collect::<Vec<_>>()
    );
    assert_eq!(result.exit_code, 23);
    assert!(result.terminal_restored);
    assert!(f
        .service
        .temporary_output(&f.workspace, &f.handle)
        .unwrap()
        .is_empty());
    let state = f.state.lock().unwrap();
    assert_eq!(state.auths, 1);
    assert_eq!(state.live.len(), 1);
    assert!(state.commands.is_empty());
    drop(state);
    let after: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    no_run_persistence(&f).await;
}

#[tokio::test]
async fn raw_terminal_streams_ten_megabytes_through_bounded_reader() {
    let f = raw_fixture().await;
    let result = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run(
            "raw-huge",
            Some(30_000),
            CancellationToken::new(),
            10 * 1024 * 1024,
        )
        .await
        .unwrap();
    assert_eq!(result.stdout.len(), 10 * 1024 * 1024);
    assert!(result.stdout.iter().all(|byte| *byte == b'x'));
    assert_eq!(result.exit_code, 0);
    assert!(result.terminal_restored);
    assert!(f
        .service
        .temporary_output(&f.workspace, &f.handle)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn raw_terminal_accepts_commands_over_task_limit_without_echo_or_history() {
    let f = raw_fixture().await;
    let command = "json-body-secret".repeat(400);
    // The full budget includes every acknowledged input line's round trip.
    let started = std::time::Instant::now();
    let result = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run(&command, Some(10_000), CancellationToken::new(), 10_000)
        .await;
    {
        let state = f.state.lock().unwrap();
        eprintln!(
            "raw input aggregate: success={} elapsed_ms={} lines={}/{} acks={} ends={} commands={} channel_messages_drained={}",
            result.is_ok(),
            started.elapsed().as_millis(),
            state.terminal_raw_input_lines.len(),
            command.len().div_ceil(180),
            state.terminal_raw_input_acks_sent,
            state.terminal_raw_input_end_received,
            state.terminal_commands.len(),
            state.terminal_channel_messages_drained,
        );
    }
    let result = result.unwrap();
    assert_eq!(result.stdout, command.as_bytes());
    assert!(result.terminal_restored);
    assert!(
        !String::from_utf8_lossy(&f.state.lock().unwrap().terminal_wire)
            .contains("json-body-secret")
    );
    assert!(f
        .service
        .temporary_output(&f.workspace, &f.handle)
        .unwrap()
        .is_empty());
    no_run_persistence(&f).await;
}

#[tokio::test]
async fn raw_terminal_cancel_recovers_and_restoration_failure_quarantines() {
    for ignore in [false, true] {
        let f = raw_fixture().await;
        f.state.lock().unwrap().terminal_ignore_interrupt = ignore;
        let operation = f
            .service
            .claim_temporary_terminal_operation(&f.workspace, &f.handle)
            .unwrap();
        let cancellation = CancellationToken::new();
        let token = cancellation.clone();
        let run =
            tokio::spawn(async move { operation.run("raw-hold", Some(10_000), token, 1000).await });
        until(|| !f.state.lock().unwrap().terminal_commands.is_empty()).await;
        assert!(f
            .service
            .temporary_input(SshSessionInput {
                workspace_id: f.workspace.clone(),
                session_id: f.handle.clone(),
                data: "blocked".into()
            })
            .await
            .is_err());
        cancellation.cancel();
        let error = run.await.unwrap().err().unwrap();
        assert!(error.to_string().contains("SSH_TERMINAL_CANCELLED"));
        let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
        assert_eq!(summary.terminal_recovery_required, ignore);
        assert!(!summary.terminal_task_busy);
        assert_eq!(
            f.service
                .claim_temporary_terminal_operation(&f.workspace, &f.handle)
                .is_err(),
            ignore
        );
    }
}

#[tokio::test]
async fn raw_terminal_full_response_survives_failed_restore() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_ignore_restore = true;
    let result = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run("response", Some(10_000), CancellationToken::new(), 1000)
        .await
        .unwrap();
    assert_eq!(result.stdout, b"response");
    assert!(!result.terminal_restored);
    assert!(f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
}

#[tokio::test]
async fn raw_terminal_limits_preflight_and_missing_end_cannot_report_success() {
    let f = raw_fixture().await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let cancelled = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run("must-not-run", Some(1000), cancel, 100)
        .await
        .err()
        .unwrap();
    assert!(cancelled.to_string().contains("SSH_TERMINAL_CANCELLED"));
    assert!(f.state.lock().unwrap().terminal_wire.is_empty());
    assert!(f
        .service
        .claim_temporary_terminal_operation("another-workspace", &f.handle)
        .is_err());
    let error = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run("raw-large", Some(10_000), CancellationToken::new(), 100)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("SSH_TERMINAL_OUTPUT_LIMIT"));
    let error = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run("raw-status-only", Some(150), CancellationToken::new(), 100)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("SSH_TERMINAL_END_TIMEOUT"));
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
}

#[tokio::test]
async fn curl_probe_uses_existing_terminal_and_has_no_output_or_network_request() {
    let f = raw_fixture().await;
    f.service
        .start_temporary_probe(
            &f.workspace,
            &f.handle,
            crate::ssh::temporary::TemporaryProbeKind::Curl,
            true,
        )
        .unwrap();
    until(|| {
        f.service
            .temporary_probe(&f.workspace, &f.handle)
            .unwrap()
            .unwrap()
            .status
            != "running"
    })
    .await;
    let report = f
        .service
        .temporary_probe(&f.workspace, &f.handle)
        .unwrap()
        .unwrap();
    assert_eq!(report.checks.len(), 4);
    assert!(report.checks.iter().all(|check| check.code == "OK"));
    assert_eq!(
        report
            .checks
            .iter()
            .map(|check| check.id.as_str())
            .collect::<Vec<_>>(),
        [
            "transport-roundtrip",
            "transport-recovery",
            "curl-tools",
            "curl-recovery"
        ]
    );
    assert!(f
        .service
        .temporary_output(&f.workspace, &f.handle)
        .unwrap()
        .is_empty());
    assert_eq!(f.state.lock().unwrap().auths, 1);
}

#[tokio::test]
async fn curl_probe_distinguishes_missing_tools_and_options_without_request_dispatch() {
    for (exit, expected) in [
        (127, "CURL_MISSING"),
        (126, "TERMINAL_TOOLS_MISSING"),
        (125, "CURL_OPTIONS_UNSUPPORTED"),
    ] {
        let f = raw_fixture().await;
        f.state.lock().unwrap().terminal_curl_code = Some(exit);
        f.service
            .start_temporary_probe(
                &f.workspace,
                &f.handle,
                crate::ssh::temporary::TemporaryProbeKind::Curl,
                true,
            )
            .unwrap();
        until(|| {
            f.service
                .temporary_probe(&f.workspace, &f.handle)
                .unwrap()
                .unwrap()
                .status
                != "running"
        })
        .await;
        let report = f
            .service
            .temporary_probe(&f.workspace, &f.handle)
            .unwrap()
            .unwrap();
        assert_eq!(report.checks.len(), 4);
        assert_eq!(report.checks[2].code, expected);
        assert_eq!(report.checks[2].status, "unsupported");
        assert_eq!(report.checks[3].code, "OK");
    }
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_raw_missing_tool = true;
    let error = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run(
            "private-request-must-not-dispatch",
            Some(5000),
            CancellationToken::new(),
            1000,
        )
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("SSH_TERMINAL_BASE64_MISSING"));
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    let state = f.state.lock().unwrap();
    assert!(state.terminal_commands.is_empty());
    assert!(!String::from_utf8_lossy(&state.terminal_wire).contains("private-request"));
    assert!(!state.terminal_wire.contains(&3));
}
