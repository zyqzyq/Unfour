use super::super::*;
use super::support::{password_input, service_with_workspaces};

async fn assert_workspace_scoped_teardown(cancel_reconnect: bool) {
    let (service, owner, other) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&owner))
        .await
        .unwrap();
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let summary = SshSessionSummary {
        warnings: vec![],
        session_id: session_id.clone(),
        workspace_id: owner.clone(),
        connection_id: connection.id.clone(),
        status: "connected".into(),
        reconnect_attempt: 0,
        auth_kind: "password".into(),
        host: "fixture".into(),
        username: "fixture".into(),
        cols: 80,
        rows: 24,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    service
        .terminal_history
        .save_session(&summary)
        .await
        .unwrap();
    let (session_cancel, session_cancelled) = tokio::sync::watch::channel(false);
    service.sessions.lock().unwrap().insert(
        session_id.clone(),
        SshSessionState {
            summary,
            events: vec![],
            command_line: SshCommandLineTracker::new(),
            pending_output: String::new(),
            history_flush_running: false,
            intentional_close: false,
            native_handle: None,
            cancel_tx: Some(session_cancel),
            sftp: None,
            sftp_generation: 7,
        },
    );
    let transfer_id = Uuid::new_v4().to_string();
    let (cancel_tx, transfer_cancelled) = tokio::sync::watch::channel(false);
    service.transfers.lock().unwrap().insert(
        transfer_id.clone(),
        SftpTransferRuntime {
            state: SftpTransferState {
                warnings: vec![],
                transfer_id,
                workspace_id: owner.clone(),
                session_id: session_id.clone(),
                connection_id: connection.id,
                direction: "download".into(),
                local_path: "fixture".into(),
                remote_path: "/fixture".into(),
                transferred_bytes: 0,
                total_bytes: 1,
                bytes_per_second: 0,
                status: "running".into(),
                error: None,
                started_at: now,
                finished_at: None,
            },
            cancel_tx,
        },
    );

    let rejected = if cancel_reconnect {
        service
            .cancel_reconnect(SshReconnectCancelInput {
                workspace_id: other,
                session_id: session_id.clone(),
            })
            .await
    } else {
        service
            .close_session(SshCloseInput {
                workspace_id: other,
                session_id: session_id.clone(),
            })
            .await
    };
    assert!(matches!(rejected, Err(AppError::NotFound(_))));
    assert!(
        !*transfer_cancelled.borrow(),
        "foreign workspace must not cancel a transfer"
    );
    assert!(
        !*session_cancelled.borrow(),
        "foreign workspace must not cancel the supervisor"
    );
    {
        let sessions = service.sessions.lock().unwrap();
        let state = sessions.get(&session_id).unwrap();
        assert!(!state.intentional_close);
        assert_eq!(state.sftp_generation, 7);
        assert_eq!(state.summary.status, "connected");
    }

    let closed = if cancel_reconnect {
        service
            .cancel_reconnect(SshReconnectCancelInput {
                workspace_id: owner.clone(),
                session_id: session_id.clone(),
            })
            .await
    } else {
        service
            .close_session(SshCloseInput {
                workspace_id: owner.clone(),
                session_id: session_id.clone(),
            })
            .await
    }
    .unwrap();
    assert_eq!(closed.status, "disconnected");
    assert!(*transfer_cancelled.borrow());
    assert!(*session_cancelled.borrow());
    assert!(!service.sessions.lock().unwrap().contains_key(&session_id));
    assert_eq!(
        service
            .terminal_history
            .get_session(&owner, &session_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "disconnected"
    );
}

#[tokio::test]
async fn close_checks_workspace_before_cancelling_native_resources() {
    assert_workspace_scoped_teardown(false).await;
}

#[tokio::test]
async fn reconnect_cancel_checks_workspace_before_cancelling_native_resources() {
    assert_workspace_scoped_teardown(true).await;
}

#[tokio::test]
async fn foreign_history_read_does_not_flush_session_output() {
    let (service, owner, other) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&owner))
        .await
        .unwrap();
    let session_id = Uuid::new_v4().to_string();
    let summary: SshSessionSummary = serde_json::from_value(serde_json::json!({
        "workspaceId": owner, "sessionId": session_id, "connectionId": connection.id,
        "status": "connected", "reconnectAttempt": 0, "authKind": "password",
        "host": "fixture", "username": "fixture", "cols": 80, "rows": 24,
        "createdAt": "", "updatedAt": ""
    }))
    .unwrap();
    service
        .terminal_history
        .save_session(&summary)
        .await
        .unwrap();
    service.sessions.lock().unwrap().insert(
        session_id.clone(),
        SshSessionState {
            summary,
            events: vec![],
            command_line: SshCommandLineTracker::new(),
            pending_output: "owner output".into(),
            history_flush_running: false,
            intentional_close: false,
            native_handle: None,
            cancel_tx: None,
            sftp: None,
            sftp_generation: 0,
        },
    );

    assert!(service
        .session_history(SshCloseInput {
            workspace_id: other,
            session_id: session_id.clone(),
        })
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        service
            .sessions
            .lock()
            .unwrap()
            .get(&session_id)
            .unwrap()
            .pending_output,
        "owner output"
    );
    let history = service
        .session_history(SshCloseInput {
            workspace_id: owner,
            session_id: session_id.clone(),
        })
        .await
        .unwrap();
    assert!(history
        .iter()
        .any(|event| event.data.contains("owner output")));
    assert!(service
        .sessions
        .lock()
        .unwrap()
        .get(&session_id)
        .unwrap()
        .pending_output
        .is_empty());
}
