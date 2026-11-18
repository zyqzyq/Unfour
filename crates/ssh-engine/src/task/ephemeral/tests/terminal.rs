use super::*;
use crate::ssh::temporary::TemporaryProbeKind;

async fn terminal_fixture() -> Fixture {
    let f = fixture().await;
    f.state.lock().unwrap().terminal_enabled = true;
    f
}

async fn manual(f: &Fixture, data: &str) -> AppResult<SshSessionEvent> {
    f.service
        .temporary_input(SshSessionInput {
            workspace_id: f.workspace.clone(),
            session_id: f.handle.clone(),
            data: data.into(),
        })
        .await
}

#[tokio::test]
async fn terminal_subscription_is_bounded_and_dirty_lease_drop_quarantines_output() {
    let f = fixture().await;
    let clean = f.service.claim_terminal(&f.workspace, &f.handle).unwrap();
    assert!(manual(&f, "blocked").await.is_err());
    drop(clean);
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    let mut lease = f.service.claim_terminal(&f.workspace, &f.handle).unwrap();
    lease.mark_dirty();
    // The ordinary fixture echoes data. Leave the bounded subscription unread
    // until its byte credits run out, as a stalled consumer would.
    for _ in 0..40 {
        lease
            .writer
            .data_bytes(vec![b'x'; 16 * 1024])
            .await
            .unwrap();
    }
    until(|| *lease.overflow.borrow()).await;
    let mut bytes = 0;
    while let Ok(chunk) = lease.output.try_recv() {
        bytes += chunk.data.len();
    }
    assert!(bytes <= 512 * 1024);
    assert!(f
        .service
        .temporary_output(&f.workspace, &f.handle)
        .unwrap()
        .is_empty());
    drop(lease);
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert!(summary.terminal_recovery_required && !summary.terminal_task_busy);
    assert!(manual(&f, "still-blocked").await.is_err());
    assert_eq!(f.state.lock().unwrap().auths, 1);
}

#[tokio::test]
async fn terminal_reuses_parent_pty_without_new_auth_and_keeps_payload_private() {
    let f = terminal_fixture().await;
    let mut input = input(
        &f,
        vec![
            command("first"),
            command("{{credential}}"),
            command("exit 0"),
        ],
    )
    .await;
    input
        .inputs
        .insert("credential".into(), "private-task-secret".into());
    input.secret_input_names.push("credential".into());
    input.context = None;
    let before: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    let run = f
        .service
        .run_temporary_task_with_mode(input, &f.handle, "terminal")
        .await
        .unwrap();
    let run = finished(&f, &run).await;
    assert_eq!(run.status, "success");
    assert!(run.warnings.contains(&"SSH_TASK_TERMINAL_RUN".into()));
    let log = f
        .service
        .read_task_run_log(f.workspace.clone(), run.id)
        .await
        .unwrap();
    assert!(log.contains("output:first"));
    assert!(
        !log.contains("private-task-secret") && !log.contains("sh -c") && !log.contains("\\0160")
    );
    assert!(f
        .service
        .temporary_output(&f.workspace, &f.handle)
        .unwrap()
        .is_empty());
    {
        let state = f.state.lock().unwrap();
        assert_eq!(state.auths, 1);
        assert_eq!(state.live.len(), 1);
        assert!(state.commands.is_empty());
        assert_eq!(
            state.terminal_commands,
            ["first", "private-task-secret", "exit 0"]
        );
        assert!(!String::from_utf8_lossy(&state.terminal_wire).contains("private-task-secret"));
    }
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert!(!summary.terminal_task_busy && !summary.terminal_recovery_required);
    manual(&f, "manual-after").await.unwrap();
    until(|| {
        f.service
            .temporary_output(&f.workspace, &f.handle)
            .unwrap()
            .iter()
            .any(|e| e.data.contains("manual-after"))
    })
    .await;
    let after: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    no_run_persistence(&f).await;
}

#[tokio::test]
async fn terminal_cancel_blocks_manual_input_and_does_not_replay_later_steps() {
    let f = terminal_fixture().await;
    let input = input(&f, vec![command("hold"), command("must-not-run")]).await;
    let run = f
        .service
        .run_temporary_task_with_mode(input.clone(), &f.handle, "terminal")
        .await
        .unwrap();
    until(|| f.state.lock().unwrap().terminal_commands.len() == 1).await;
    assert!(manual(&f, "must-not-be-input")
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_TERMINAL_BUSY"));
    assert!(f
        .service
        .run_temporary_task_with_mode(input, &f.handle, "terminal")
        .await
        .is_err());
    assert!(f
        .service
        .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Terminal, true)
        .is_err());
    f.service
        .cancel_task_run(SshTaskCancelInput {
            workspace_id: f.workspace.clone(),
            run_id: run.id.clone(),
        })
        .await
        .unwrap();
    let run = finished(&f, &run).await;
    assert_eq!(run.status, "cancelled");
    assert_eq!(
        run.error_message.as_deref(),
        Some("SSH_TASK_OUTCOME_UNCERTAIN")
    );
    {
        let state = f.state.lock().unwrap();
        assert_eq!(state.terminal_commands, ["hold"]);
        assert!(!String::from_utf8_lossy(&state.terminal_wire).contains("must-not-be-input"));
    }
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert_eq!(summary.status, "connected");
    assert!(!summary.terminal_task_busy && !summary.terminal_recovery_required);
    let next = super::fixture::input(&f, vec![command("next")]).await;
    let next = f
        .service
        .run_temporary_task_with_mode(next, &f.handle, "terminal")
        .await
        .unwrap();
    assert_eq!(finished(&f, &next).await.status, "success");
    no_run_persistence(&f).await;
}

#[tokio::test]
async fn terminal_status_without_pipe_end_is_unknown_and_failed_restore_quarantines() {
    let f = terminal_fixture().await;
    f.state.lock().unwrap().terminal_ignore_interrupt = true;
    let mut first = command("status-only");
    first.config_json["timeoutSeconds"] = 1.into();
    first.config_json["continueOnError"] = true.into();
    let input = input(&f, vec![first, command("must-not-run")]).await;
    let run = f
        .service
        .run_temporary_task_with_mode(input.clone(), &f.handle, "terminal")
        .await
        .unwrap();
    let run = finished(&f, &run).await;
    assert_eq!(run.status, "failed");
    assert_eq!(
        run.error_message.as_deref(),
        Some("SSH_TASK_OUTCOME_UNCERTAIN")
    );
    let log = f
        .service
        .read_task_run_log(f.workspace.clone(), run.id)
        .await
        .unwrap();
    assert!(log.contains("SSH_TASK_TERMINAL_RECOVERY_REQUIRED"));
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert_eq!(summary.status, "connected");
    assert!(!summary.terminal_task_busy && summary.terminal_recovery_required);
    assert!(manual(&f, "manual")
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_TERMINAL_RECOVERY_REQUIRED"));
    assert!(f
        .service
        .run_temporary_task_with_mode(input, &f.handle, "terminal")
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_TERMINAL_RECOVERY_REQUIRED"));
    let state = f.state.lock().unwrap();
    assert_eq!(state.terminal_commands, ["status-only"]);
    assert_eq!(state.terminal_wire.iter().filter(|b| **b == 3).count(), 1);
    assert_eq!(state.live.len(), 1);
}

#[tokio::test]
async fn terminal_missing_or_altered_middle_line_stops_before_dispatch() {
    for dropped in [true, false] {
        let f = terminal_fixture().await;
        {
            let mut state = f.state.lock().unwrap();
            if dropped {
                state.terminal_task_drop_line = Some(2);
            } else {
                state.terminal_raw_mutate_input_line = Some(2);
            }
        }
        let mut first = command(&"x".repeat(140));
        first.config_json["timeoutSeconds"] = 2.into();
        first.config_json["continueOnError"] = true.into();
        let input = input(&f, vec![first, command("must-not-run")]).await;
        let run = f
            .service
            .run_temporary_task_with_mode(input, &f.handle, "terminal")
            .await
            .unwrap();
        let run = finished(&f, &run).await;
        assert_eq!(run.status, "failed");
        assert_eq!(
            run.error_message.as_deref(),
            Some("SSH_TASK_TERMINAL_INPUT_UNCONFIRMED")
        );
        let wire = f.state.lock().unwrap().terminal_wire.clone();
        let mut lines = wire.split(|byte| *byte == b'\n');
        assert_eq!(
            lines.clone().filter(|line| line.starts_with(b"D")).count(),
            2
        );
        assert!(!lines.any(|line| line.starts_with(b"E:")));
        assert!(f.state.lock().unwrap().terminal_commands.is_empty());
        assert_eq!(wire.iter().filter(|byte| **byte == 3).count(), 1);
        manual(&f, "manual-after").await.unwrap();
    }
}

#[tokio::test]
async fn terminal_cancel_before_ready_never_writes_to_unconfirmed_parent() {
    for stage in ["STARTUP", "READY"] {
        let f = terminal_fixture().await;
        f.state.lock().unwrap().terminal_task_stall = Some(stage);
        let input = input(&f, vec![command("must-not-run")]).await;
        let run = f
            .service
            .run_temporary_task_with_mode(input, &f.handle, "terminal")
            .await
            .unwrap();
        let expected_lines = if stage == "STARTUP" { 1 } else { 2 };
        until(|| {
            f.state
                .lock()
                .unwrap()
                .terminal_wire
                .iter()
                .filter(|b| **b == b'\n')
                .count()
                == expected_lines
        })
        .await;
        let before = f.state.lock().unwrap().terminal_wire.clone();
        f.service
            .cancel_task_run(SshTaskCancelInput {
                workspace_id: f.workspace.clone(),
                run_id: run.id.clone(),
            })
            .await
            .unwrap();
        let run = finished(&f, &run).await;
        assert_eq!(run.status, "cancelled");
        assert_eq!(
            run.error_message.as_deref(),
            Some("SSH_TASK_TERMINAL_RECOVERY_REQUIRED")
        );
        assert!(f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
        assert!(manual(&f, "blocked").await.is_err());
        let state = f.state.lock().unwrap();
        assert!(state.terminal_commands.is_empty());
        assert_eq!(state.terminal_wire, before);
    }
}

#[tokio::test]
async fn displayed_password_prompt_rejects_task_and_api_before_any_automatic_input() {
    let f = terminal_fixture().await;
    // This fixture echoes manual input. Drain the echoed prompt exactly as the UI does.
    manual(&f, "Pass").await.unwrap();
    manual(&f, "word: ").await.unwrap();
    manual(&f, "***").await.unwrap();
    let mut displayed = String::new();
    until(|| {
        for event in f.service.temporary_output(&f.workspace, &f.handle).unwrap() {
            displayed.push_str(&event.data);
        }
        displayed.ends_with("Password: ***")
    })
    .await;
    let before = f.state.lock().unwrap().terminal_wire.clone();

    let task = input(&f, vec![command("private-command")]).await;
    let run = f
        .service
        .run_temporary_task_with_mode(task.clone(), &f.handle, "terminal")
        .await
        .unwrap();
    let run = finished(&f, &run).await;
    assert_eq!(
        run.error_message.as_deref(),
        Some("SSH_TASK_INTERACTIVE_LOGIN_PROMPT")
    );
    let api = f
        .service
        .claim_temporary_terminal_operation(&f.workspace, &f.handle)
        .unwrap();
    let error = api
        .run(
            "private-command",
            None,
            tokio_util::sync::CancellationToken::new(),
            1024,
        )
        .await
        .err()
        .unwrap();
    assert_eq!(error.code(), "SSH_TERMINAL_LOGIN_REQUIRED");
    assert_eq!(f.state.lock().unwrap().terminal_wire, before);
    let summary = &f.service.list_temporary(&f.workspace).unwrap()[0];
    assert!(!summary.terminal_recovery_required && !summary.terminal_task_busy);

    manual(&f, "\r\n$ ").await.unwrap();
    until(|| {
        !f.service
            .temporary_output(&f.workspace, &f.handle)
            .unwrap()
            .is_empty()
    })
    .await;
    let run = f
        .service
        .run_temporary_task_with_mode(task, &f.handle, "terminal")
        .await
        .unwrap();
    assert_eq!(finished(&f, &run).await.status, "success");
}

#[tokio::test]
async fn terminal_login_prompt_never_receives_user_payload_or_leaks_bootstrap() {
    let f = terminal_fixture().await;
    f.state.lock().unwrap().terminal_login_prompt = true;
    let input = input(&f, vec![command("private-command")]).await;
    let run = f
        .service
        .run_temporary_task_with_mode(input, &f.handle, "terminal")
        .await
        .unwrap();
    let run = finished(&f, &run).await;
    assert_eq!(
        run.error_message.as_deref(),
        Some("SSH_TASK_INTERACTIVE_LOGIN_PROMPT")
    );
    let log = f
        .service
        .read_task_run_log(f.workspace.clone(), run.id)
        .await
        .unwrap();
    assert!(
        !log.contains("private-login-title")
            && !log.contains("private-command")
            && !log.contains("sh -c")
    );
    assert!(f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    assert!(f
        .service
        .temporary_output(&f.workspace, &f.handle)
        .unwrap()
        .is_empty());
    let state = f.state.lock().unwrap();
    assert!(state.terminal_commands.is_empty());
    assert!(!state.terminal_wire.contains(&3));
    assert!(!String::from_utf8_lossy(&state.terminal_wire).contains("private-command"));
}

#[tokio::test]
async fn terminal_rejects_transfer_and_wrong_workspace_before_touching_parent() {
    let f = terminal_fixture().await;
    let upload = input(&f, vec![step("upload", serde_json::json!({"localPath":"missing","remotePath":"/fixture.txt","overwrite":false}))]).await;
    assert!(f
        .service
        .run_temporary_task_with_mode(upload, &f.handle, "terminal")
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_TERMINAL_COMMAND_ONLY"));
    let valid = input(&f, vec![command("first")]).await;
    assert!(f
        .service
        .run_temporary_task_with_mode(valid.clone(), &f.handle, "fallback")
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_EXECUTION_MODE_INVALID"));
    let mut foreign = valid;
    foreign.workspace_id = uuid::Uuid::new_v4().to_string();
    assert!(f
        .service
        .run_temporary_task_with_mode(foreign, &f.handle, "terminal")
        .await
        .is_err());
    assert!(f.state.lock().unwrap().terminal_wire.is_empty());
    assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_task_busy);
}

#[tokio::test]
async fn terminal_probe_checks_zero_nonzero_on_parent_pty_without_database_writes() {
    let f = terminal_fixture().await;
    let before: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    f.service
        .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Terminal, true)
        .unwrap();
    until(|| {
        f.service
            .temporary_probe(&f.workspace, &f.handle)
            .unwrap()
            .is_some_and(|p| p.status != "running")
    })
    .await;
    let report = f
        .service
        .temporary_probe(&f.workspace, &f.handle)
        .unwrap()
        .unwrap();
    assert_eq!(report.checks.len(), 2);
    assert!(report.checks.iter().all(|c| c.status == "pass"));
    assert_eq!(report.checks[0].actual_exit_code, Some(0));
    assert_eq!(report.checks[1].actual_exit_code, Some(23));
    let after: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    let state = f.state.lock().unwrap();
    assert_eq!(state.auths, 1);
    assert_eq!(state.live.len(), 1);
    assert!(state.commands.is_empty());
}

#[tokio::test]
async fn closing_parent_during_terminal_run_never_runs_following_command() {
    let f = terminal_fixture().await;
    let input = input(&f, vec![command("hold"), command("must-not-run")]).await;
    let run = f
        .service
        .run_temporary_task_with_mode(input, &f.handle, "terminal")
        .await
        .unwrap();
    until(|| f.state.lock().unwrap().terminal_commands.len() == 1).await;
    f.service.close_temporary_workspace(&f.workspace).unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(f
        .service
        .temporary_task_run(&f.workspace, &run.id)
        .unwrap()
        .is_none());
    assert_eq!(f.state.lock().unwrap().terminal_commands, ["hold"]);
    no_run_persistence(&f).await;
}
