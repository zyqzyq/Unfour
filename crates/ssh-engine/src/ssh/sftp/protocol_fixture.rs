use super::*;

pub(super) async fn fixture(
    server: impl russh_sftp::server::Handler + Send + 'static,
) -> (SshService, SftpPathInput, tokio::task::JoinHandle<()>) {
    let (client, mut client_proxy) = tokio::io::duplex(128 * 1024);
    let (mut server_proxy, stream) = tokio::io::duplex(128 * 1024);
    russh_sftp::server::run(stream, server).await;
    // The server helper spawns internally. Own the proxy task so abort really
    // closes the transport instead of aborting an already-finished wrapper.
    let task = tokio::spawn(async move {
        let _ = tokio::io::copy_bidirectional(&mut client_proxy, &mut server_proxy).await;
    });
    let sftp = SftpSession::new(client).await.unwrap();
    sftp.set_timeout(1);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_lazy("sqlite::memory:")
        .unwrap();
    // No schema: any accidental persistence operation would fail this fixture.
    let service = SshService::new(
        LocalDb::from_pool(pool),
        SecretStore::in_memory("sftp-test"),
    );
    let input = SftpPathInput {
        workspace_id: Uuid::new_v4().to_string(),
        session_id: Uuid::new_v4().to_string(),
        path: "/fixture.txt".into(),
    };
    let summary: SshSessionSummary = serde_json::from_value(serde_json::json!({
        "workspaceId": input.workspace_id, "sessionId": input.session_id,
        "connectionId": Uuid::new_v4().to_string(), "status": "connected", "reconnectAttempt": 0,
        "authKind": "password", "host": "fixture", "username": "fixture", "cols": 80, "rows": 24,
        "createdAt": "", "updatedAt": ""
    }))
    .unwrap();
    service.sessions.lock().unwrap().insert(
        input.session_id.clone(),
        SshSessionState {
            summary,
            events: vec![],
            command_line: SshCommandLineTracker::new(),
            pending_output: String::new(),
            history_flush_running: false,
            intentional_close: false,
            native_handle: None,
            cancel_tx: None,
            sftp: Some(SftpChannelState {
                session: Arc::new(sftp),
                home_path: "/".into(),
                generation: 1,
            }),
            sftp_generation: 1,
        },
    );
    (service, input, task)
}
