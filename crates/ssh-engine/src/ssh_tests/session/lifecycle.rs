#[cfg(not(feature = "ssh-native"))]
use super::super::super::*;
#[cfg(not(feature = "ssh-native"))]
use super::super::support::{password_input, service_with_workspaces};

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn ssh_session_lifecycle_supports_connect_input_resize_close_and_export() {
    let (service, workspace_id, _) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_id))
        .await
        .expect("save ssh connection");

    let session = service
        .connect(SshConnectInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            cols: Some(100),
            rows: Some(30),
            secret: None,
        })
        .await
        .expect("connect ssh session");
    assert_eq!(session.connection_id, connection.id);
    assert_eq!(session.status, "connected");
    assert_eq!(session.cols, 100);

    let output = service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "echo ok\npassword=secret\n".to_string(),
        })
        .await
        .expect("send ssh input");
    assert_eq!(output.kind, "output");

    let resize = service
        .resize(SshResizeInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            cols: 120,
            rows: 40,
        })
        .await
        .expect("resize ssh pty");
    assert_eq!(resize.kind, "resize");

    let sessions = service
        .list_sessions(workspace_id.clone())
        .await
        .expect("list sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].cols, 120);

    let closed = service
        .close_session(SshCloseInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
        })
        .await
        .expect("close session");
    assert_eq!(closed.status, "disconnected");

    let rejected = service
        .send_input(SshSessionInput {
            workspace_id: workspace_id.clone(),
            session_id: session.session_id.clone(),
            data: "whoami\n".to_string(),
        })
        .await;
    assert!(matches!(rejected, Err(AppError::Validation(_))));

    let export = service
        .export_log(SshLogExportInput {
            workspace_id,
            session_id: session.session_id,
        })
        .expect("export log");
    assert!(export.content.contains("<redacted>"));
    assert!(!export.content.contains("password=secret"));
    assert!(export.redacted);
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn deleting_ssh_connection_closes_active_sessions() {
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
        .delete_connection(workspace_id.clone(), connection.id)
        .await
        .expect("delete connection");
    let sessions = service
        .list_sessions(workspace_id)
        .await
        .expect("list sessions after delete");
    assert_eq!(sessions[0].session_id, session.session_id);
    assert_eq!(sessions[0].status, "disconnected");
}
