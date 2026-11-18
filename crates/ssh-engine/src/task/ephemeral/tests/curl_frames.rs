use super::*;
use crate::ssh::temporary::TemporaryProbeKind;
use unfour_core::terminal_curl_contract::TerminalCurlRequest;

fn request(data: &str) -> TerminalCurlRequest {
    TerminalCurlRequest {
        id: uuid::Uuid::new_v4(),
        config: data.into(),
        needs_data_raw: false,
    }
}

async fn send(
    f: &Fixture,
    bytes: &str,
) -> AppResult<crate::ssh::temporary::TerminalOperationResult> {
    f.service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)?
        .run_curl(&request(bytes), Some(5000), CancellationToken::new(), 4096)
        .await
}

async fn probe_curl(f: &Fixture) {
    f.service
        .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Curl, true)
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
    assert!(f
        .service
        .temporary_probe(&f.workspace, &f.handle)
        .unwrap()
        .unwrap()
        .checks
        .iter()
        .all(|c| c.code == "OK"));
}

#[tokio::test]
async fn only_a_verified_live_context_uses_burst_and_manual_input_invalidates_it() {
    let f = raw_fixture().await;
    assert!(!send(&f, "cold").await.unwrap().timing.input_burst);
    probe_curl(&f).await;
    for config in ["first", "second"] {
        let response = send(&f, config).await.unwrap();
        assert!(response.timing.input_burst && response.terminal_restored);
        assert_eq!(response.stdout, config.as_bytes());
        assert_eq!(response.timing.input_ack_count, 1);
    }
    assert!(!send(&f, &"H".repeat(721)).await.unwrap().timing.input_burst);
    f.state.lock().unwrap().terminal_context = Some('b');
    assert!(
        !send(&f, "different shell")
            .await
            .unwrap()
            .timing
            .input_burst
    );
    probe_curl(&f).await;
    f.service
        .temporary_input(SshSessionInput {
            workspace_id: f.workspace.clone(),
            session_id: f.handle.clone(),
            data: "manual".into(),
        })
        .await
        .unwrap();
    assert!(!send(&f, "after manual").await.unwrap().timing.input_burst);
    probe_curl(&f).await;
    let mut task = input(&f, vec![command("first")]).await;
    task.context = None;
    let run = f
        .service
        .run_temporary_task_with_mode(task, &f.handle, "terminal")
        .await
        .unwrap();
    assert_eq!(finished(&f, &run).await.status, "success");
    let next = send(&f, "after task").await.unwrap();
    assert!(!next.timing.input_burst && next.terminal_restored);
    assert_eq!(f.state.lock().unwrap().auths, 1);
}

#[tokio::test]
async fn corrupted_burst_never_commits_and_next_send_uses_confirmed_lines() {
    let f = raw_fixture().await;
    probe_curl(&f).await;
    let count = f.state.lock().unwrap().terminal_commands.len();
    f.state.lock().unwrap().terminal_raw_mutate_input_line = Some(1);
    let error = send(&f, "private request").await.err().unwrap();
    assert_eq!(error.code(), "SSH_TERMINAL_PROTOCOL_ERROR");
    assert_eq!(f.state.lock().unwrap().terminal_commands.len(), count);
    f.state.lock().unwrap().terminal_raw_mutate_input_line = None;
    assert!(!send(&f, "new request").await.unwrap().timing.input_burst);
}

#[tokio::test]
async fn cancelling_during_bootstrap_recovers_from_late_load_without_dispatch() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_line_delay = Some(Duration::from_millis(250));
    let op = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap();
    let token = CancellationToken::new();
    let cancel = token.clone();
    let pending = tokio::spawn(async move {
        op.run_curl(&request("never sent"), None, cancel, 4096)
            .await
    });
    until(|| !f.state.lock().unwrap().terminal_wire.is_empty()).await;
    token.cancel();
    assert_eq!(
        pending.await.unwrap().err().unwrap().code(),
        "SSH_TERMINAL_CANCELLED"
    );
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    assert!(f.state.lock().unwrap().terminal_commands.is_empty());
}

#[tokio::test]
async fn pending_restore_holds_all_probes_and_cancel_cannot_erase_complete_result() {
    let f = raw_fixture().await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    f.state.lock().unwrap().terminal_raw_restore_gate = Some(gate.clone());
    let op = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap();
    let token = CancellationToken::new();
    let cancel = token.clone();
    let run =
        tokio::spawn(async move { op.run_curl(&request("complete"), None, cancel, 4096).await });
    until(|| f.state.lock().unwrap().terminal_raw_restores_waiting > 0).await;
    for kind in [
        TemporaryProbeKind::Exec,
        TemporaryProbeKind::Transport,
        TemporaryProbeKind::Curl,
    ] {
        assert!(f
            .service
            .start_temporary_probe(&f.workspace, &f.handle, kind, true)
            .is_err());
    }
    assert!(f
        .service
        .temporary_input(SshSessionInput {
            workspace_id: f.workspace.clone(),
            session_id: f.handle.clone(),
            data: "blocked".into(),
        })
        .await
        .is_err());
    let mut task = input(&f, vec![command("first")]).await;
    task.context = None;
    assert!(f
        .service
        .run_temporary_task_with_mode(task, &f.handle, "terminal")
        .await
        .is_err());
    token.cancel();
    gate.add_permits(1);
    let result = run.await.unwrap().unwrap();
    assert_eq!(result.stdout, b"complete");
    assert!(result.terminal_restored);
    f.state.lock().unwrap().terminal_raw_restore_gate = None;
    f.state.lock().unwrap().terminal_ignore_restore = true;
    let result = send(&f, "preserve").await.unwrap();
    assert_eq!(result.stdout, b"preserve");
    assert!(!result.terminal_restored);
    assert!(f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
}
