use super::*;
use crate::ssh::temporary::{TemporaryProbeKind, TemporaryProbeReport};

fn start(f: &Fixture, kind: TemporaryProbeKind) -> TemporaryProbeReport {
    f.service
        .start_temporary_probe(&f.workspace, &f.handle, kind, true)
        .unwrap()
}

async fn finished_probe(f: &Fixture) -> TemporaryProbeReport {
    until(|| {
        f.service
            .temporary_probe(&f.workspace, &f.handle)
            .unwrap()
            .unwrap()
            .status
            != "running"
    })
    .await;
    f.service
        .temporary_probe(&f.workspace, &f.handle)
        .unwrap()
        .unwrap()
}

fn assert_no_curl(f: &Fixture) {
    assert!(f
        .state
        .lock()
        .unwrap()
        .terminal_commands
        .iter()
        .all(|command| !command.contains("curl")));
}

#[tokio::test]
async fn standalone_transport_verifies_bytes_nonzero_and_recovery_without_curl_or_new_channels() {
    let f = raw_fixture().await;
    let before: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    start(&f, TemporaryProbeKind::Transport);
    let report = finished_probe(&f).await;
    assert_eq!(report.kind, TemporaryProbeKind::Transport);
    assert_eq!(report.status, "completed");
    assert_eq!(
        report
            .checks
            .iter()
            .map(|check| check.id.as_str())
            .collect::<Vec<_>>(),
        ["transport-roundtrip", "transport-recovery"]
    );
    assert!(report
        .checks
        .iter()
        .all(|check| check.status == "pass" && check.code == "OK"));
    assert_eq!(report.checks[0].expected_exit_code, Some(23));
    assert_eq!(report.checks[0].actual_exit_code, Some(23));
    assert_eq!(report.checks[1].expected_exit_code, None);
    assert_eq!(report.checks[1].actual_exit_code, None);
    assert_no_curl(&f);
    {
        let state = f.state.lock().unwrap();
        assert_eq!(state.terminal_commands.len(), 1);
        assert!(state.terminal_commands[0].len() > 1024);
        assert_eq!(
            state.terminal_raw_input_lines.len(),
            state.terminal_commands[0].len().div_ceil(180)
        );
        assert_eq!(state.auths, 1);
        assert_eq!(state.live.len(), 1);
        assert!(state.commands.is_empty());
        assert_eq!(state.sftp_requests, 0);
    }
    assert!(f
        .service
        .temporary_output(&f.workspace, &f.handle)
        .unwrap()
        .is_empty());
    let after: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    no_run_persistence(&f).await;
}

#[tokio::test]
async fn curl_probe_stops_on_transport_byte_or_exit_mismatch_even_when_shell_restores() {
    for (corrupt, exit, expected) in [
        (true, None, "OUTPUT_MISMATCH"),
        (false, Some(0), "EXIT_STATUS_MISMATCH"),
    ] {
        let f = raw_fixture().await;
        {
            let mut state = f.state.lock().unwrap();
            state.terminal_transport_corrupt_output = corrupt;
            state.terminal_transport_code = exit;
        }
        start(&f, TemporaryProbeKind::Curl);
        let report = finished_probe(&f).await;
        assert_eq!(report.checks.len(), 2);
        assert_eq!(report.checks[0].id, "transport-roundtrip");
        assert_eq!(report.checks[0].code, expected);
        assert_eq!(report.checks[0].status, "fail");
        assert_eq!(report.checks[1].code, "OK");
        assert_no_curl(&f);
        assert_eq!(f.state.lock().unwrap().terminal_commands.len(), 1);
        assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    }
}

#[tokio::test]
async fn curl_probe_requires_confirmed_transport_recovery_before_tools() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_ignore_restore = true;
    start(&f, TemporaryProbeKind::Curl);
    let report = finished_probe(&f).await;
    assert_eq!(report.checks.len(), 2);
    assert_eq!(report.checks[0].code, "OK");
    assert_eq!(report.checks[1].id, "transport-recovery");
    assert_eq!(report.checks[1].code, "SSH_TERMINAL_RESTORE_TIMEOUT");
    assert_eq!(report.checks[1].status, "inconclusive");
    assert_no_curl(&f);
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert_eq!(summary.status, "connected");
    assert!(summary.terminal_recovery_required);
}

#[tokio::test]
async fn both_probe_stages_keep_the_terminal_exclusively_claimed_until_final_restore() {
    let f = raw_fixture().await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    f.state.lock().unwrap().terminal_raw_restore_gate = Some(gate.clone());
    start(&f, TemporaryProbeKind::Curl);
    for count in 1..=2 {
        until(|| {
            let state = f.state.lock().unwrap();
            state.terminal_commands.len() == count && state.terminal_raw_restores_waiting == 1
        })
        .await;
        assert!(f.service.list_temporary(&f.workspace).unwrap()[0].terminal_task_busy);
        assert!(f
            .service
            .claim_temporary_terminal_operation(&f.workspace, &f.handle)
            .is_err());
        assert!(f
            .service
            .temporary_input(SshSessionInput {
                workspace_id: f.workspace.clone(),
                session_id: f.handle.clone(),
                data: "must-not-interleave".into()
            })
            .await
            .is_err());
        assert_eq!(
            f.state.lock().unwrap().terminal_raw_restores_sent,
            count - 1
        );
        gate.add_permits(1);
    }
    let report = finished_probe(&f).await;
    assert_eq!(report.checks.len(), 4);
    assert!(report.checks.iter().all(|check| check.status == "pass"));
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert!(!summary.terminal_task_busy && !summary.terminal_recovery_required);
    assert_eq!(f.state.lock().unwrap().auths, 1);
    f.service
        .temporary_input(SshSessionInput {
            workspace_id: f.workspace.clone(),
            session_id: f.handle.clone(),
            data: "parent-still-usable".into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn cancellation_while_transport_restores_does_not_start_curl_or_release_dirty() {
    let f = raw_fixture().await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    f.state.lock().unwrap().terminal_raw_restore_gate = Some(gate.clone());
    let run = start(&f, TemporaryProbeKind::Curl);
    until(|| f.state.lock().unwrap().terminal_raw_restores_waiting == 1).await;
    f.service
        .cancel_temporary_probe(&f.workspace, &f.handle, &run.run_id)
        .unwrap();
    assert!(f.service.list_temporary(&f.workspace).unwrap()[0].terminal_task_busy);
    gate.add_permits(1);
    let report = finished_probe(&f).await;
    assert_eq!(report.status, "cancelled");
    assert_eq!(report.checks.len(), 2);
    assert!(report.checks.iter().all(|check| check.code == "OK"));
    assert_no_curl(&f);
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert!(!summary.terminal_task_busy && !summary.terminal_recovery_required);
    assert_eq!(f.state.lock().unwrap().terminal_commands.len(), 1);
}

#[tokio::test]
async fn cancellation_during_transport_input_never_dispatches_transport_or_curl() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_raw_ack_gate = Some(Arc::new(tokio::sync::Semaphore::new(0)));
    let run = start(&f, TemporaryProbeKind::Curl);
    until(|| f.state.lock().unwrap().terminal_raw_pending_input_acks == 1).await;
    f.service
        .cancel_temporary_probe(&f.workspace, &f.handle, &run.run_id)
        .unwrap();
    let report = finished_probe(&f).await;
    assert_eq!(report.status, "cancelled");
    assert_eq!(report.checks.len(), 2);
    assert_eq!(report.checks[0].code, "CANCELLED");
    assert_eq!(report.checks[1].code, "OK");
    assert!(f.state.lock().unwrap().terminal_commands.is_empty());
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert!(!summary.terminal_task_busy && !summary.terminal_recovery_required);
}
