use super::super::super::*;
#[cfg(not(feature = "ssh-native"))]
use super::super::support::{password_input, service_with_workspaces};

#[test]
fn reconnect_policy_is_bounded_to_three_attempts() {
    assert_eq!(RECONNECT_BACKOFF_SECS, [1, 2, 4]);
    assert_eq!(RECONNECT_BACKOFF_SECS.len(), 3);
    assert_eq!(RECONNECT_BACKOFF_SECS.iter().sum::<u64>(), 7);
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn explicit_close_disables_reconnect() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");
    let session = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id,
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect session");

    service
        .close_session(SshCloseInput {
            workspace_id,
            session_id: session.session_id.clone(),
        })
        .await
        .expect("close session");

    // The in-memory entry must be dropped on close so the session map cannot
    // grow without bound (issue #4). The session survives only in the
    // terminal-history store.
    let sessions = service.sessions.lock().expect("session lock");
    assert!(sessions.get(&session.session_id).is_none());
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn cancel_reconnect_marks_session_disconnected() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");
    let session = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id,
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect session");

    let cancelled = service
        .cancel_reconnect(SshReconnectCancelInput {
            workspace_id,
            session_id: session.session_id.clone(),
        })
        .await
        .expect("cancel reconnect");

    assert_eq!(cancelled.status, "disconnected");
    assert_eq!(cancelled.reconnect_attempt, 0);
    // cancel_reconnect drops the in-memory entry (issue #4); verify it is gone.
    let sessions = service.sessions.lock().expect("session lock");
    assert!(sessions.get(&session.session_id).is_none());
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn dropped_and_failed_states_stop_after_cleanup() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");
    let session = service
        .connect(SshConnectInput {
            workspace_id,
            connection_id: connection.id,
            cols: None,
            rows: None,
            secret: None,
        })
        .await
        .expect("connect session");

    let mut sessions = service.sessions.lock().expect("session lock");
    let state = sessions
        .get_mut(&session.session_id)
        .expect("session state");
    state.summary.status = "degraded".to_string();
    assert!(should_reconnect(state));
    state.summary.status = "reconnecting".to_string();
    state.summary.reconnect_attempt = 3;
    assert!(should_reconnect(state));
    state.summary.status = "failed".to_string();
    assert!(!should_reconnect(state));
}
