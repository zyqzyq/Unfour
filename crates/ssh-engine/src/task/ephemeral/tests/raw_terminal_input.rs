use super::*;

#[tokio::test]
async fn every_payload_line_waits_for_its_ack_before_more_input_or_completion() {
    let f = raw_fixture().await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    f.state.lock().unwrap().terminal_raw_ack_gate = Some(gate.clone());
    let command = "unfour-input".repeat(43);
    let expected = command.clone();
    let lines = command.len().div_ceil(180);
    let operation = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap();
    let run = tokio::spawn(async move {
        operation
            .run(&command, Some(5000), CancellationToken::new(), 1000)
            .await
    });

    for index in 1..=lines {
        until(|| f.state.lock().unwrap().terminal_raw_input_lines.len() >= index).await;
        // The peer continues processing SSH input while the ACK is withheld.
        // A pipelining driver would deliver the next D/E during this delay.
        tokio::time::sleep(Duration::from_millis(60)).await;
        {
            let state = f.state.lock().unwrap();
            assert_eq!(state.terminal_raw_input_lines.len(), index);
            assert_eq!(state.terminal_raw_pending_input_acks, 1);
            assert_eq!(state.terminal_raw_input_acks_sent, index - 1);
            assert_eq!(state.terminal_raw_input_end_received, 0);
            assert!(!state.terminal_raw_pipelined_input);
            assert!(state.terminal_commands.is_empty());
        }
        gate.add_permits(1);
    }

    let response = run.await.unwrap().unwrap();
    assert_eq!(response.timing.input_ack_count, lines + 1);
    assert!(response.timing.input_ack_wait_ms >= 60 * lines as u128);
    assert!(response.timing.input_ack_wait_ms <= response.timing.input_ms);
    assert_eq!(response.stdout, expected.as_bytes());
    assert!(response.terminal_restored);
    let state = f.state.lock().unwrap();
    assert_eq!(state.terminal_raw_input_acks_sent, lines);
    assert_eq!(state.terminal_raw_input_end_received, 1);
    assert!(state.terminal_raw_input_lines.iter().all(|len| *len <= 242));
    assert!(!state.terminal_raw_pipelined_input);
    assert_eq!(state.terminal_commands, [expected]);
    assert_eq!(state.auths, 1);
}

#[tokio::test]
async fn truncated_payload_or_wrong_ack_index_never_advances_or_executes() {
    for (truncate, wrong_index, delivered_lines) in
        [(Some(1), false, 1), (Some(3), false, 3), (None, true, 1)]
    {
        let f = raw_fixture().await;
        {
            let mut state = f.state.lock().unwrap();
            state.terminal_raw_truncate_input_line = truncate;
            state.terminal_raw_wrong_input_index = wrong_index;
        }
        let error = f
            .service
            .claim_temporary_terminal_operation(&f.workspace, &f.handle)
            .unwrap()
            .run(
                &"private-request".repeat(30),
                Some(5000),
                CancellationToken::new(),
                1000,
            )
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("SSH_TERMINAL_PROTOCOL_ERROR"));
        let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
        assert_eq!(summary.status, "connected");
        assert!(!summary.terminal_recovery_required && !summary.terminal_task_busy);
        let state = f.state.lock().unwrap();
        assert_eq!(state.terminal_raw_input_lines.len(), delivered_lines);
        assert_eq!(state.terminal_raw_input_end_received, 0);
        assert!(state.terminal_commands.is_empty());
        assert_eq!(state.auths, 1);
    }
}

#[tokio::test]
async fn missing_line_ack_or_completion_ack_remains_input_timeout_without_begin() {
    for (stall, delivered_lines, completions) in [("INPUT_ACK", 1, 0), ("INPUT_END", 3, 1)] {
        let f = raw_fixture().await;
        f.state.lock().unwrap().terminal_raw_stall = Some(stall);
        let error = f
            .service
            .claim_temporary_terminal_operation(&f.workspace, &f.handle)
            .unwrap()
            .run(
                &"private-request".repeat(30),
                Some(1000),
                CancellationToken::new(),
                1000,
            )
            .await
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("SSH_TERMINAL_INPUT_ACK_TIMEOUT"),
            "{stall}: {error}"
        );
        assert!(!error.to_string().contains("SSH_TERMINAL_BEGIN_TIMEOUT"));
        let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
        assert_eq!(summary.status, "connected");
        assert!(!summary.terminal_recovery_required && !summary.terminal_task_busy);
        let state = f.state.lock().unwrap();
        assert_eq!(state.terminal_raw_input_lines.len(), delivered_lines);
        assert_eq!(state.terminal_raw_input_end_received, completions);
        assert!(state.terminal_commands.is_empty());
        assert_eq!(state.auths, 1);
    }
}

#[tokio::test]
async fn invalid_completion_totals_restore_without_dispatch_or_parent_interrupt() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_raw_corrupt_end_total = true;
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
    assert!(error.to_string().contains("SSH_TERMINAL_PROTOCOL_ERROR"));
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    let state = f.state.lock().unwrap();
    assert_eq!(state.terminal_raw_input_end_received, 1);
    assert!(state.terminal_commands.is_empty());
    assert!(!state.terminal_wire.contains(&3));
    assert!(!state.terminal_wire.windows(2).any(|part| part == b"Q\n"));
}

#[tokio::test]
async fn same_length_payload_corruption_is_rejected_before_completion_or_execution() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_raw_mutate_input_line = Some(2);
    let error = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run(
            &"private-request".repeat(30),
            Some(5000),
            CancellationToken::new(),
            1000,
        )
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("SSH_TERMINAL_PROTOCOL_ERROR"));
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    let state = f.state.lock().unwrap();
    assert_eq!(state.terminal_raw_input_lines.len(), 2);
    assert_eq!(state.terminal_raw_input_end_received, 0);
    assert!(state.terminal_commands.is_empty());
}

#[tokio::test]
async fn encoder_only_peer_keeps_confirmed_octal_delivery() {
    let f = raw_fixture().await;
    f.state.lock().unwrap().terminal_octal = true;
    let command = "synthetic中文".repeat(40);
    let result = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap()
        .run(&command, None, CancellationToken::new(), 1000)
        .await
        .unwrap();
    assert_eq!(result.stdout, command.as_bytes());
    assert_eq!(
        result.timing.input_ack_count,
        command.len().div_ceil(48) + 1
    );
    assert!(result.terminal_restored);
}

#[tokio::test]
async fn lost_or_corrupt_final_ack_after_commit_never_replays_the_request() {
    for (corrupt, expected) in [
        (false, "SSH_TERMINAL_INPUT_ACK_TIMEOUT"),
        (true, "SSH_TERMINAL_PROTOCOL_ERROR"),
    ] {
        let f = raw_fixture().await;
        {
            let mut state = f.state.lock().unwrap();
            state.terminal_raw_corrupt_end_ack = corrupt;
            if !corrupt {
                state.terminal_raw_stall = Some("INPUT_END_ACK");
            }
        }
        let error = f
            .service
            .claim_temporary_terminal_operation(&f.workspace, &f.handle)
            .unwrap()
            .run("committed-once", Some(1000), CancellationToken::new(), 1000)
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains(expected));
        assert_eq!(
            f.state.lock().unwrap().terminal_commands,
            ["committed-once"]
        );
        assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    }
}
