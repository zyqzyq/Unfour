use super::*;

fn original_state() -> State {
    let mut state = State::default();
    state
        .files
        .insert("/fixture.txt".into(), b"original".to_vec());
    state
}

#[tokio::test]
async fn inline_sftp_all_precommit_failures_keep_original_even_if_cleanup_fails() {
    for failure in ["partial-write", "metadata", "fsync", "close", "commit"] {
        for fail_remove in [false, true] {
            let mut initial = original_state();
            match failure {
                "partial-write" => initial.fail_after_writes = Some(1),
                "metadata" => initial.fail_metadata = true,
                "fsync" => initial.fail_sync = true,
                "close" => initial.fail_close = true,
                "commit" => initial.fail_commit = true,
                _ => unreachable!(),
            }
            initial.fail_remove = fail_remove;
            let (session, state) = fixture_with(initial).await;
            let result = write_inline(
                session,
                &input(SshFileWriteMode::Overwrite, "x".repeat(100_000)),
            )
            .await;
            assert!(result.is_err(), "{failure}");
            if !fail_remove {
                cleaned(&state).await;
            }
            let state = state.lock().unwrap();
            assert_eq!(state.files["/fixture.txt"], b"original", "{failure}");
            assert!(
                state.renames.is_empty(),
                "must never move the original to a backup"
            );
            assert!(state
                .removes
                .iter()
                .all(|path| path.starts_with("/.unfour-inline-")));
            if fail_remove {
                assert_eq!(
                    state.files.len(),
                    2,
                    "leave staging, never remove the destination"
                );
            }
        }
    }
}

#[tokio::test]
async fn inline_sftp_atomic_extension_missing_or_unknown_version_fails_before_mutation() {
    for version in [None, Some("2")] {
        let mut initial = original_state();
        initial.no_atomic = version.is_none();
        initial.atomic_version = version.map(str::to_string);
        let (session, state) = fixture_with(initial).await;
        let error = write_inline(
            session.clone(),
            &input(SshFileWriteMode::Overwrite, "new".into()),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), "SSH_FILE_ATOMIC_REPLACE_UNSUPPORTED");
        {
            let state = state.lock().unwrap();
            assert_eq!(state.files.len(), 1);
            assert_eq!(state.files["/fixture.txt"], b"original");
            assert!(state.flags.is_empty());
        }
        // Unrelated operations remain supported without the atomic extension.
        write_inline(
            session.clone(),
            &input(SshFileWriteMode::Append, "+append".into()),
        )
        .await
        .unwrap();
        let mut create = input(SshFileWriteMode::Create, "new".into());
        create.path = "/new.txt".into();
        write_inline(session, &create).await.unwrap();
    }
}

#[tokio::test]
async fn inline_sftp_cancel_or_timeout_during_commit_keeps_complete_target() {
    for after_commit in [false, true] {
        for timeout in [false, true] {
            let gate = gate();
            let mut initial = original_state();
            if after_commit {
                initial.hold_commit_after = Some(gate.clone());
            } else {
                initial.hold_commit_before = Some(gate.clone());
            }
            let (session, state) = fixture_with(initial).await;
            let task = tokio::spawn(async move {
                let request = input(SshFileWriteMode::Overwrite, "complete-new".into());
                if timeout {
                    tokio::time::timeout(
                        std::time::Duration::from_millis(200),
                        write_inline(session, &request),
                    )
                    .await
                } else {
                    Ok(write_inline(session, &request).await)
                }
            });
            tokio::time::timeout(std::time::Duration::from_secs(2), gate.0.notified())
                .await
                .unwrap();
            assert_eq!(
                state.lock().unwrap().files["/fixture.txt"],
                if after_commit {
                    b"complete-new".as_slice()
                } else {
                    b"original".as_slice()
                }
            );
            if timeout {
                assert!(task.await.unwrap().is_err());
            } else {
                task.abort();
                assert!(task.await.unwrap_err().is_cancelled());
            }
            // Cancellation does not retract a request already received by the
            // server. Even a late commit must leave a complete destination.
            gate.1.notify_one();
            cleaned(&state).await;
            let state = state.lock().unwrap();
            assert_eq!(state.files["/fixture.txt"], b"complete-new");
            assert_eq!(state.commits, 1);
            assert!(state.renames.is_empty());
        }
    }
}

#[tokio::test]
async fn inline_sftp_lost_open_or_commit_reply_never_removes_destination() {
    for phase in ["open", "before-commit", "after-commit"] {
        let gate = gate();
        let mut initial = original_state();
        match phase {
            "open" => initial.hold_open_after = Some(gate.clone()),
            "before-commit" => initial.hold_commit_before = Some(gate.clone()),
            "after-commit" => initial.hold_commit_after = Some(gate.clone()),
            _ => unreachable!(),
        }
        let (session, state) = fixture_with(initial).await;
        let writer = session.clone();
        let task = tokio::spawn(async move {
            write_inline(
                writer,
                &input(SshFileWriteMode::Overwrite, "complete-new".into()),
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), gate.0.notified())
            .await
            .unwrap();
        session.raw.close_session().unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(4), task)
            .await
            .unwrap()
            .unwrap();
        assert!(result.is_err(), "{phase}");
        {
            let state = state.lock().unwrap();
            assert_eq!(
                state.files["/fixture.txt"],
                if phase == "after-commit" {
                    b"complete-new".as_slice()
                } else {
                    b"original".as_slice()
                }
            );
            assert!(state.renames.is_empty());
        }
        gate.1.notify_one();
        // No recovery rename or destination removal exists, including when the
        // server finishes its in-flight request after the connection is lost.
    }
}

#[tokio::test]
async fn inline_sftp_failed_append_preserves_original_prefix_and_is_not_replayed() {
    let mut initial = original_state();
    initial.fail_after_writes = Some(1);
    let (session, state) = fixture_with(initial).await;
    assert!(write_inline(
        session,
        &input(SshFileWriteMode::Append, "x".repeat(100_000))
    )
    .await
    .is_err());
    let state = state.lock().unwrap();
    let content = &state.files["/fixture.txt"];
    assert_eq!(&content[..8], b"original");
    assert_eq!(content.len(), 8 + 32_768);
    assert_eq!(state.writes, 2);
    assert!(state.removes.is_empty());
    assert!(state.renames.is_empty());
}

#[tokio::test]
async fn inline_sftp_cancelled_append_never_truncates_or_removes_original() {
    let gate = gate();
    let mut initial = original_state();
    initial.hold_write = Some(gate.clone());
    let (session, state) = fixture_with(initial).await;
    let task = tokio::spawn(async move {
        write_inline(
            session,
            &input(SshFileWriteMode::Append, "x".repeat(100_000)),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), gate.0.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(state.lock().unwrap().files["/fixture.txt"], b"original");
    gate.1.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while state.lock().unwrap().closes == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let state = state.lock().unwrap();
    assert_eq!(&state.files["/fixture.txt"][..8], b"original");
    assert_eq!(state.writes, 1);
    assert!(state.removes.is_empty());
    assert!(state.renames.is_empty());
}

#[test]
fn inline_sftp_runtime_shutdown_during_cleanup_is_bounded() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let state = runtime.block_on(async {
        let gate = gate();
        let mut initial = original_state();
        initial.hold_commit_before = Some(gate.clone());
        let (session, state) = fixture_with(initial).await;
        let task = tokio::spawn(async move {
            write_inline(session, &input(SshFileWriteMode::Overwrite, "new".into())).await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), gate.0.notified())
            .await
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        state
    });
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
    assert_eq!(state.lock().unwrap().files["/fixture.txt"], b"original");
}
