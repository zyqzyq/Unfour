use super::*;

#[cfg(feature = "ssh-native")]
impl CommandMessages for tokio::sync::mpsc::Receiver<russh::ChannelMsg> {
    fn next_message(
        &mut self,
    ) -> impl std::future::Future<Output = Option<russh::ChannelMsg>> + Send {
        self.recv()
    }
}

#[cfg(feature = "ssh-native")]
#[tokio::test]
async fn queued_exit_status_wins_over_simultaneous_cancel_or_timeout() {
    for cancelled in [false, true] {
        for exit_status in [0, 7] {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(2);
            sender.try_send(russh::ChannelMsg::Eof).unwrap();
            sender
                .try_send(russh::ChannelMsg::ExitStatus { exit_status })
                .unwrap();
            // Keep the sender open. A stopped reader must not wait for channel close.
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                read_command_result(&mut receiver, std::future::ready(cancelled), &mut |_| {}),
            )
            .await
            .expect("stopped command reader waited for another SSH message");
            assert_eq!(
                result,
                command_step_outcome(Some(exit_status as i32), None, cancelled)
            );
        }
    }
}

#[cfg(feature = "ssh-native")]
#[tokio::test]
async fn continuous_output_cannot_starve_a_ready_interrupt() {
    for cancelled in [false, true] {
        let capacity = native_client_config().channel_buffer_size;
        let (sender, mut receiver) = tokio::sync::mpsc::channel(capacity + 1);
        for _ in 0..=capacity {
            sender
                .try_send(russh::ChannelMsg::Data {
                    data: b"x\n".as_slice().into(),
                })
                .unwrap();
        }
        let result =
            read_command_result(&mut receiver, std::future::ready(cancelled), &mut |_| {}).await;
        assert_eq!(result, Err(TaskStepError::OutcomeUncertain { cancelled }));
        assert_eq!(
            receiver.len(),
            1,
            "drain must be bounded even while more output is ready"
        );
    }
}

#[test]
fn cwd_failure_exits_the_step_before_the_entire_script() {
    let lines = "printf first || printf fallback\nprintf second";
    assert_eq!(task_command(lines, "   ").unwrap(), lines);
    assert_eq!(
        task_command(lines, " /tmp/user's dir ").unwrap(),
        format!("cd -- '/tmp/user'\\''s dir' || exit $?\n{lines}")
    );
}

#[test]
fn working_directory_guard_preserves_the_script_budget_and_validates_resolved_inputs() {
    let limit = super::super::MAX_TASK_COMMAND_CHARS;
    let command = "x".repeat(limit);
    assert!(task_command(&command, "/tmp/user's dir")
        .unwrap()
        .ends_with(&command));
    assert!(task_command(&"x".repeat(limit + 1), "tmp").is_err());
    assert!(task_command("pwd", &"d".repeat(limit + 1)).is_err());
    assert!(task_command("pwd", "/tmp\nunexpected").is_err());
    assert!(task_command("  ", "tmp").is_err());
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires a POSIX shell; set UNFOUR_TEST_SH to its executable"
)]
fn multiline_step_runs_only_in_its_requested_directory() {
    let shell = std::env::var_os("UNFOUR_TEST_SH").unwrap_or_else(|| "sh".into());
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("user's dir")).unwrap();
    let script = "printf 'first\\n' || printf fallback\nprintf 'second\\n'\npwd\nexit 7";
    for (directory, valid) in [("user's dir", true), ("missing", false)] {
        let result = std::process::Command::new(&shell)
            .current_dir(root.path())
            .arg("-c")
            .arg(task_command(script, directory).unwrap())
            .output()
            .unwrap();
        if valid {
            assert_eq!(result.status.code(), Some(7));
            let output = String::from_utf8(result.stdout).unwrap();
            assert!(output.starts_with("first\nsecond\n") && output.ends_with("/user's dir\n"));
        } else {
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn missing_exit_status_and_signal_leave_the_dispatched_command_outcome_uncertain() {
    for cancelled in [false, true] {
        assert_eq!(
            command_step_outcome(None, None, cancelled),
            Err(TaskStepError::OutcomeUncertain { cancelled })
        );
    }
}

#[test]
fn non_zero_exit_status_is_failure() {
    let err = command_step_outcome(Some(1), None, false).unwrap_err();
    assert!(matches!(
        err,
        TaskStepError::Failed {
            exit_code: Some(1),
            ..
        }
    ));
}

#[test]
fn zero_exit_status_is_success() {
    for cancelled in [false, true] {
        assert_eq!(
            command_step_outcome(Some(0), None, cancelled).unwrap(),
            TaskStepResult { exit_code: Some(0) }
        );
    }
}

#[test]
fn exit_signal_is_failure_even_with_zero_code() {
    let err = command_step_outcome(Some(0), Some("signal SIGKILL"), false).unwrap_err();
    assert!(matches!(err, TaskStepError::Failed { .. }));
}

#[cfg(feature = "ssh-native")]
#[tokio::test(start_paused = true)]
async fn eof_and_exit_finish_without_close_in_either_order() {
    for eof_first in [true, false] {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(3);
        sender
            .try_send(russh::ChannelMsg::Data {
                data: b"tail".as_slice().into(),
            })
            .unwrap();
        let pair = if eof_first {
            [
                russh::ChannelMsg::Eof,
                russh::ChannelMsg::ExitStatus { exit_status: 7 },
            ]
        } else {
            [
                russh::ChannelMsg::ExitStatus { exit_status: 7 },
                russh::ChannelMsg::Eof,
            ]
        };
        for item in pair {
            sender.try_send(item).unwrap();
        }
        let mut events = Vec::new();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            read_command_result(&mut receiver, std::future::pending(), &mut |event| {
                events.push(event)
            }),
        )
        .await
        .expect("must not await Close once exit and EOF arrived");
        assert_eq!(result, command_step_outcome(Some(7), None, false));
        assert_eq!(
            events,
            vec![DriverEvent::Output {
                stream: "stdout".into(),
                data: "tail".into()
            }]
        );
    }
}

#[cfg(feature = "ssh-native")]
#[tokio::test(start_paused = true)]
async fn exit_before_delayed_tail_keeps_tail_and_finishes_on_eof() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(3);
    sender
        .try_send(russh::ChannelMsg::ExitStatus { exit_status: 0 })
        .unwrap();
    let write = async {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        sender
            .send(russh::ChannelMsg::Data {
                data: b"final bytes".as_slice().into(),
            })
            .await
            .unwrap();
        sender.send(russh::ChannelMsg::Eof).await.unwrap();
    };
    let mut events = Vec::new();
    let mut emit = |event| events.push(event);
    let result = tokio::join!(
        write,
        read_command_result(&mut receiver, std::future::pending(), &mut emit)
    )
    .1;
    assert_eq!(result, Ok(TaskStepResult { exit_code: Some(0) }));
    assert_eq!(
        events,
        vec![DriverEvent::Output {
            stream: "stdout".into(),
            data: "final bytes".into()
        }]
    );
}

#[cfg(feature = "ssh-native")]
#[tokio::test(start_paused = true)]
async fn missing_eof_after_exit_has_bounded_drain_and_explicit_output_warning() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    sender
        .try_send(russh::ChannelMsg::ExitStatus { exit_status: 0 })
        .unwrap();
    let mut events = Vec::new();
    let start = tokio::time::Instant::now();
    let grace = std::time::Duration::from_millis(50);
    let result = read_command_result_with_grace(
        &mut receiver,
        std::future::pending(),
        &mut |event| events.push(event),
        grace,
    )
    .await;
    assert_eq!(tokio::time::Instant::now() - start, grace);
    assert_eq!(result, Ok(TaskStepResult { exit_code: Some(0) }));
    assert_eq!(
        events,
        vec![DriverEvent::Output {
            stream: "diagnostic".into(),
            data: "SSH_TASK_OUTPUT_INCOMPLETE\n".into()
        }]
    );
}

#[cfg(feature = "ssh-native")]
#[tokio::test(start_paused = true)]
async fn login_prompt_is_reported_before_timeout_but_never_claims_execution_failed() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    sender
        .try_send(russh::ChannelMsg::Data {
            data: b"\x1b]0;private-user@host\x07login: ".as_slice().into(),
        })
        .unwrap();
    let interrupt = async {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        false
    };
    let start = tokio::time::Instant::now();
    let mut events = Vec::new();
    let result = read_command_result(&mut receiver, interrupt, &mut |event| {
        events.push((tokio::time::Instant::now() - start, event))
    })
    .await;
    assert_eq!(
        result,
        Err(TaskStepError::OutcomeUncertain { cancelled: false })
    );
    assert_eq!(
        events[0],
        (
            std::time::Duration::from_millis(250),
            DriverEvent::Output {
                stream: "diagnostic".into(),
                data: "SSH_TASK_INTERACTIVE_LOGIN_PROMPT\n".into()
            }
        )
    );
    assert_eq!(
        events[1].1,
        DriverEvent::Output {
            stream: "stdout".into(),
            data: "login: ".into()
        }
    );
}

#[cfg(feature = "ssh-native")]
#[tokio::test(start_paused = true)]
async fn packet_boundary_does_not_turn_a_business_line_into_a_prompt() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(4);
    sender
        .try_send(russh::ChannelMsg::Data {
            data: b"login:".as_slice().into(),
        })
        .unwrap();
    let write = async {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        sender
            .send(russh::ChannelMsg::Data {
                data: b" previous authentication succeeded\n".as_slice().into(),
            })
            .await
            .unwrap();
        sender
            .send(russh::ChannelMsg::ExitStatus { exit_status: 0 })
            .await
            .unwrap();
        sender.send(russh::ChannelMsg::Eof).await.unwrap();
    };
    let mut events = Vec::new();
    let mut emit = |event| events.push(event);
    let result = tokio::join!(
        write,
        read_command_result(&mut receiver, std::future::pending(), &mut emit)
    )
    .1;
    assert_eq!(result, Ok(TaskStepResult { exit_code: Some(0) }));
    assert_eq!(
        events,
        vec![DriverEvent::Output {
            stream: "stdout".into(),
            data: "login: previous authentication succeeded\n".into()
        }]
    );
}

#[cfg(feature = "ssh-native")]
#[tokio::test(start_paused = true)]
async fn blocked_enqueue_obeys_deadline_and_cancel_without_dispatch() {
    let (cancel, mut cancel_rx) = tokio::sync::watch::channel(false);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    assert_eq!(
        send_exec_request(std::future::pending(), &mut cancel_rx, deadline).await,
        Err(TaskStepError::Failed {
            message: "SSH_TASK_EXEC_QUEUE_TIMEOUT".into(),
            exit_code: None
        })
    );
    cancel.send_replace(true);
    assert_eq!(
        send_exec_request(std::future::pending(), &mut cancel_rx, deadline).await,
        Err(TaskStepError::Cancelled)
    );
}
