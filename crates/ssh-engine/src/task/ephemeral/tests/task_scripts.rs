use super::*;

#[tokio::test]
async fn exec_and_terminal_deliver_the_guard_before_the_whole_script() {
    let f = fixture().await;
    f.state.lock().unwrap().terminal_enabled = true;
    let script = "printf first; printf second\nprintf third";
    let mut step = command(script);
    step.config_json["workingDirectory"] = "missing dir".into();
    let input = input(&f, vec![step]).await;
    for mode in ["exec", "terminal"] {
        let run = f
            .service
            .run_temporary_task_with_mode(input.clone(), &f.handle, mode)
            .await
            .unwrap();
        finished(&f, &run).await;
    }
    let state = f.state.lock().unwrap();
    assert_eq!(
        state.commands,
        [format!("cd -- 'missing dir' || exit $?\n{script}")]
    );
    assert_eq!(state.terminal_commands, state.commands);
}

#[tokio::test]
async fn terminal_record_separated_json_keeps_output_and_the_confirmed_exit_code() {
    let f = fixture().await;
    {
        let mut state = f.state.lock().unwrap();
        state.terminal_enabled = true;
        state.terminal_task_stdout = Some(b"\x1e{\"ok\":true}\n".to_vec());
    }
    for (code, status) in [(0, "success"), (23, "failed")] {
        let script = format!("printf '\\036{{\"ok\":true}}\\n'; exit {code}");
        let input = input(&f, vec![command(&script)]).await;
        let run = f
            .service
            .run_temporary_task_with_mode(input, &f.handle, "terminal")
            .await
            .unwrap();
        let run = finished(&f, &run).await;
        assert_eq!(run.status, status);
        if code != 0 {
            assert_eq!(
                run.error_message.as_deref(),
                Some("Command exited with status 23")
            );
        }
        let log = f
            .service
            .read_task_run_log(f.workspace.clone(), run.id)
            .await
            .unwrap();
        assert!(log.contains("{\"ok\":true}\n"), "{log}");
        assert!(!log.contains(":STATUS:"));
        assert!(!f.service.list_temporary(&f.workspace).unwrap()[0].terminal_recovery_required);
    }
}
