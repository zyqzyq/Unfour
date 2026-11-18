use super::*;

#[tokio::test]
async fn missing_prerequisite_restores_without_interrupt_or_payload_and_parent_is_reusable() {
    for (printf, expected) in [
        (false, "SSH_TERMINAL_BASE64_MISSING"),
        (true, "SSH_TERMINAL_PRINTF_UNSUPPORTED"),
    ] {
        let f = raw_fixture().await;
        {
            let mut state = f.state.lock().unwrap();
            state.terminal_raw_missing_tool = !printf;
            state.terminal_raw_printf_external = printf;
        }
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
        assert!(error.to_string().contains(expected), "{error}");
        let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
        assert_eq!(summary.status, "connected");
        assert!(!summary.terminal_recovery_required && !summary.terminal_task_busy);
        {
            let mut state = f.state.lock().unwrap();
            assert!(state.terminal_commands.is_empty());
            assert!(!state.terminal_wire.contains(&3));
            assert_eq!(state.auths, 1);
            assert_eq!(state.live.len(), 1);
            state.terminal_raw_missing_tool = false;
            state.terminal_raw_printf_external = false;
        }
        let response = f
            .service
            .claim_temporary_terminal_operation(&f.workspace, &f.handle)
            .unwrap()
            .run("after-failure", Some(5000), CancellationToken::new(), 1000)
            .await
            .unwrap();
        assert_eq!(response.stdout, b"after-failure");
        assert!(response.terminal_restored);
        assert_eq!(f.state.lock().unwrap().auths, 1);
    }
}

#[tokio::test]
async fn native_stalls_keep_the_exact_phase_and_only_interrupt_a_confirmed_child() {
    for stage in ["STARTUP", "READY", "ACCEPT", "INPUT_ACK", "BEGIN", "END"] {
        let f = raw_fixture().await;
        f.state.lock().unwrap().terminal_raw_stall = Some(stage);
        // Multiple lines verify that an unacknowledged input cannot advance.
        let command = "secret-json".repeat(40);
        let error = f
            .service
            .claim_temporary_terminal_operation(&f.workspace, &f.handle)
            .unwrap()
            .run(&command, Some(300), CancellationToken::new(), 2000)
            .await
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains(&format!("SSH_TERMINAL_{stage}_TIMEOUT")),
            "{stage}: {error}"
        );
        let unknown_child = matches!(stage, "STARTUP" | "READY");
        let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
        assert_eq!(summary.status, "connected", "{stage}");
        assert_eq!(summary.terminal_recovery_required, unknown_child, "{stage}");
        assert!(!summary.terminal_task_busy);
        let state = f.state.lock().unwrap();
        assert_eq!(state.terminal_wire.contains(&3), !unknown_child, "{stage}");
        assert_eq!(state.auths, 1);
        assert_eq!(state.live.len(), 1);
        if matches!(stage, "STARTUP" | "READY" | "ACCEPT" | "INPUT_ACK") {
            assert!(state.terminal_commands.is_empty());
        }
    }
}

#[tokio::test]
async fn unrelated_restore_never_unlocks_or_sends_input_to_an_unknown_parent() {
    let f = raw_fixture().await;
    {
        let mut state = f.state.lock().unwrap();
        state.terminal_raw_missing_tool = true;
        state.terminal_raw_foreign_restore = true;
    }
    let error = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run(
            "private-request",
            Some(5000),
            CancellationToken::new(),
            1000,
        )
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("SSH_TERMINAL_BASE64_MISSING"));
    assert!(f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    assert!(!f.state.lock().unwrap().terminal_wire.contains(&3));
    assert!(f.state.lock().unwrap().terminal_commands.is_empty());
}

#[tokio::test]
async fn correlated_early_restore_is_remembered_and_never_followed_by_parent_input() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_raw_early_restore = true;
    let error = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run(
            "private-request",
            Some(5000),
            CancellationToken::new(),
            1000,
        )
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("SSH_TERMINAL_NOT_READY"));
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    let state = f.state.lock().unwrap();
    assert!(!state.terminal_wire.contains(&3));
    assert!(state.terminal_commands.is_empty());
    assert!(!state.terminal_wire.windows(2).any(|part| part == b"Q\n"));
}

#[tokio::test]
async fn complete_end_and_restored_in_same_packet_keep_result_without_parent_input() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_raw_auto_restore = true;
    let response = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run(
            "complete-response",
            Some(5000),
            CancellationToken::new(),
            1000,
        )
        .await
        .unwrap();
    assert_eq!(response.stdout, b"complete-response");
    assert!(response.terminal_restored);
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    let state = f.state.lock().unwrap();
    assert!(!state.terminal_wire.contains(&3));
    assert!(!state.terminal_wire.windows(2).any(|part| part == b"Q\n"));
}
