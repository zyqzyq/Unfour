#[cfg(not(feature = "ssh-native"))]
use super::super::super::*;
#[cfg(not(feature = "ssh-native"))]
use super::super::support::{password_input, service_with_workspaces};

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn repeated_close_does_not_panic_and_returns_stable_result() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");
    let session = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect ssh session");

    let first_close = service
        .close_session(SshCloseInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
        })
        .await
        .expect("first close");
    assert_eq!(first_close.status, "disconnected");

    let second_close = service
        .close_session(SshCloseInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
        })
        .await
        .expect("second close should not fail");
    assert_eq!(second_close.status, "disconnected");
    assert_eq!(second_close.session_id, first_close.session_id);
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn async_send_input_and_resize_work_in_simulated_path() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");
    let session = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            cols: Some(80),
            rows: Some(24),
            secret: None,
        })
        .await
        .expect("connect ssh session");

    // send_input is now async.
    let event = service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "ls -la\n".to_string(),
        })
        .await
        .expect("async send input");
    assert_eq!(event.kind, "output");

    // resize is now async.
    let resize_event = service
        .resize(SshResizeInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            cols: 200,
            rows: 50,
        })
        .await
        .expect("async resize");
    assert_eq!(resize_event.kind, "resize");
    assert!(resize_event.data.contains("200x50"));

    // Verify session dimensions were updated.
    let sessions = service
        .list_sessions(workspace_id)
        .await
        .expect("list sessions");
    assert_eq!(sessions[0].cols, 200);
    assert_eq!(sessions[0].rows, 50);
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn command_history_records_only_after_enter_and_deduplicates_repeats() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");
    let session = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect ssh session");

    service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "git ".to_string(),
        })
        .await
        .expect("send partial command");
    service
        .ingest_terminal_output(&workspace_id, &session.session_id, "git ")
        .await;
    let query = SshCommandHistoryQuery {
        workspace_id: workspace_id.clone(),
        connection_id: Some(connection.id.clone()),
        search: None,
        limit: Some(20),
        include_redacted: false,
    };
    assert!(service
        .list_command_history(query.clone())
        .await
        .expect("list before enter")
        .is_empty());

    service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "status\r".to_string(),
        })
        .await
        .expect("execute command");
    service
        .ingest_terminal_output(&workspace_id, &session.session_id, "git status\r\n")
        .await;
    service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "git status\r".to_string(),
        })
        .await
        .expect("repeat command");
    service
        .ingest_terminal_output(&workspace_id, &session.session_id, "git status\r\n")
        .await;

    let history = service
        .list_command_history(query)
        .await
        .expect("list after enter");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].command, "git status");
    assert_eq!(history[0].connection_id, connection.id);
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn command_history_drops_unechoed_secret_input() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");
    let session = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect ssh session");

    service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "hunter2\r".to_string(),
        })
        .await
        .expect("send password");
    service
        .ingest_terminal_output(
            &workspace_id,
            &session.session_id,
            "\r\nSorry, try again.\r\n",
        )
        .await;

    let query = SshCommandHistoryQuery {
        workspace_id: workspace_id.clone(),
        connection_id: Some(connection.id.clone()),
        search: None,
        limit: Some(20),
        include_redacted: true,
    };
    assert!(service
        .list_command_history(query.clone())
        .await
        .expect("list after secret")
        .is_empty());

    service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "pwd\r".to_string(),
        })
        .await
        .expect("send pwd");
    service
        .ingest_terminal_output(&workspace_id, &session.session_id, "pwd\r\n")
        .await;
    let history = service
        .list_command_history(query)
        .await
        .expect("list after echoed command");
    assert_eq!(
        history
            .iter()
            .map(|entry| entry.command.as_str())
            .collect::<Vec<_>>(),
        vec!["pwd"]
    );
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn command_history_reconnect_does_not_keep_partial_input() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");
    let session = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect ssh session");

    service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "rm -rf /tmp/foo".to_string(),
        })
        .await
        .expect("send partial command");
    {
        let mut sessions = service.sessions.lock().expect("session lock");
        sessions
            .get_mut(&session.session_id)
            .expect("session state")
            .command_line
            .reset();
    }
    service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "ls\r".to_string(),
        })
        .await
        .expect("send after reconnect");
    service
        .ingest_terminal_output(&workspace_id, &session.session_id, "ls\r\n")
        .await;

    let history = service
        .list_command_history(SshCommandHistoryQuery {
            workspace_id,
            connection_id: Some(connection.id),
            search: None,
            limit: Some(20),
            include_redacted: false,
        })
        .await
        .expect("list after reconnect");
    assert_eq!(
        history
            .iter()
            .map(|entry| entry.command.as_str())
            .collect::<Vec<_>>(),
        vec!["ls"]
    );
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn multiple_sessions_handle_concurrent_input_and_close() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");

    let session_a = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect session a");

    let session_b = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect session b");

    // Send input to both sessions concurrently.
    let (result_a, result_b) = tokio::join!(
        service.send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session_a.session_id.clone(),
            data: "echo A\n".to_string(),
        }),
        service.send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session_b.session_id.clone(),
            data: "echo B\n".to_string(),
        }),
    );
    assert!(result_a.is_ok());
    assert!(result_b.is_ok());

    // Close session a, session b remains active.
    service
        .close_session(SshCloseInput {
            workspace_id: workspace_id.clone(),
            session_id: session_a.session_id.clone(),
        })
        .await
        .expect("close session a");

    // Session b should still accept input.
    let event_b = service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session_b.session_id.clone(),
            data: "whoami\n".to_string(),
        })
        .await
        .expect("session b still active");
    assert_eq!(event_b.kind, "output");

    let sessions = service
        .list_sessions(workspace_id)
        .await
        .expect("list sessions");
    let closed = sessions
        .iter()
        .find(|s| s.session_id == session_a.session_id)
        .unwrap();
    let active = sessions
        .iter()
        .find(|s| s.session_id == session_b.session_id)
        .unwrap();
    assert_eq!(closed.status, "disconnected");
    assert_eq!(active.status, "connected");
}
