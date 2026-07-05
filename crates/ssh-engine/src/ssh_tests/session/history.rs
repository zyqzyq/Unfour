#[cfg(not(feature = "ssh-native"))]
use super::super::super::*;
#[cfg(not(feature = "ssh-native"))]
use super::super::support::{password_input, service_with_workspaces};

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn explicit_close_flushes_buffered_output_and_restore_lists_history() {
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
        .expect("connect ssh session");

    {
        let mut sessions = service.sessions.lock().expect("lock sessions");
        sessions
            .get_mut(&session.session_id)
            .expect("session")
            .pending_output
            .push_str("buffered before close\r\n");
    }
    service
        .close_session(SshCloseInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
        })
        .await
        .expect("close session");
    service.sessions.lock().expect("lock sessions").clear();

    let restored = service
        .list_sessions(workspace_id.clone())
        .await
        .expect("list persisted sessions");
    let history = service
        .session_history(SshCloseInput {
            workspace_id,
            session_id: session.session_id,
        })
        .await
        .expect("hydrate persisted history");
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].status, "disconnected");
    assert!(history[0].data.contains("buffered before close"));
    assert!(history[0].data.contains("SSH session closed."));
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn repeated_flush_without_new_output_does_not_duplicate_history() {
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
        .expect("connect ssh session");

    {
        let mut sessions = service.sessions.lock().expect("lock sessions");
        sessions
            .get_mut(&session.session_id)
            .expect("session")
            .pending_output
            .push_str("persist exactly once\r\n");
    }
    service
        .flush_session_history(&session.session_id)
        .await
        .expect("first flush");
    service
        .flush_session_history(&session.session_id)
        .await
        .expect("second flush");

    let history = service
        .session_history(SshCloseInput {
            workspace_id,
            session_id: session.session_id,
        })
        .await
        .expect("hydrate history");
    assert_eq!(history[0].data.matches("persist exactly once").count(), 1);
}
