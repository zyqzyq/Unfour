//! Loopback SSH verifies the actual command-bus -> native channel/stdin path.
use super::*;
use russh::server::{Msg, Server as _, Session};
use std::sync::{Arc, Mutex};
use unfour_core::models::{SshFilePatchInput, SSH_FILE_MAX_CONTENT_BYTES};

#[derive(Default)]
struct Probe {
    commands: Vec<String>,
    requests: Vec<serde_json::Value>,
    hold: bool,
}
#[derive(Clone)]
struct Server {
    probe: Arc<Mutex<Probe>>,
    data: Vec<u8>,
}
impl russh::server::Server for Server {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        Self {
            probe: self.probe.clone(),
            data: vec![],
        }
    }
}
impl russh::server::Handler for Server {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        _: &str,
        _: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        Ok(russh::server::Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        _: russh::Channel<Msg>,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
    async fn exec_request(
        &mut self,
        channel: russh::ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.probe
            .lock()
            .unwrap()
            .commands
            .push(String::from_utf8(data.to_vec()).unwrap());
        session.channel_success(channel)?;
        Ok(())
    }
    async fn data(
        &mut self,
        _: russh::ChannelId,
        data: &[u8],
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.data.extend_from_slice(data);
        Ok(())
    }
    async fn channel_eof(
        &mut self,
        channel: russh::ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut probe = self.probe.lock().unwrap();
        probe
            .requests
            .push(serde_json::from_slice(&self.data).unwrap());
        if !probe.hold {
            session.data(
                channel,
                b"__UNFOUR_MATCH_COUNT__ 1\n__UNFOUR_PATCHED__ 1\n".to_vec(),
            )?;
            session.eof(channel)?;
            session.exit_status_request(channel, 0)?;
            session.close(channel)?;
        }
        Ok(())
    }
}

#[tokio::test]
async fn ssh_file_patch_native_stdin_limits_scoping_timeout_and_audit() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let probe = Arc::new(Mutex::new(Probe::default()));
    let mut server = Server {
        probe: probe.clone(),
        data: vec![],
    };
    let config = Arc::new(russh::server::Config {
        keys: vec![russh::keys::PrivateKey::random(
            &mut rand::rng(),
            russh::keys::Algorithm::Ed25519,
        )
        .unwrap()],
        ..Default::default()
    });
    let server_task = tokio::spawn(async move {
        server.run_on_socket(config, &socket).await.unwrap();
    });
    let connection = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "file fixture".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            username: "fixture".into(),
            auth_kind: "password".into(),
            key_path: None,
            credential_ref: None,
            secret: Some("disposable".into()),
        })
        .await
        .unwrap();
    let mut input = SshFilePatchInput {
        workspace_id: workspace,
        connection_id: connection.id,
        path: "/fixture 'quoted'.txt".into(),
        search: "sensitive-search-marker".into(),
        replace: String::new(),
        allow_multiple: false,
        timeout_ms: Some(10_000),
    };
    input.replace = "x".repeat(SSH_FILE_MAX_CONTENT_BYTES - input.search.len());
    let result = bus.patch_ssh_file(input.clone()).await.unwrap();
    assert_eq!(
        result.exit_status,
        Some(0),
        "exit status after EOF must be preserved"
    );
    {
        let probe = probe.lock().unwrap();
        assert_eq!(probe.commands.len(), 1);
        assert!(probe.commands[0].len() < 4096);
        assert!(!probe.commands[0].contains(&input.search));
        assert!(!probe.commands[0].contains(&input.path));
        assert_eq!(probe.requests[0]["search"], input.search);
        assert_eq!(probe.requests[0]["replace"], input.replace);
        assert_eq!(probe.requests[0]["path"], input.path);
    }
    let details: String = sqlx::query_scalar(
        "SELECT details_json FROM activity_events WHERE action = 'ssh.file.patch'",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert!(details.contains("searchBytes"));
    assert!(!details.contains(&input.search));
    assert!(!details.contains(&input.replace));
    let mut invalid = input.clone();
    invalid.replace.push('x');
    assert_eq!(
        bus.patch_ssh_file(invalid).await.unwrap_err().code(),
        "SSH_FILE_CONTENT_TOO_LARGE"
    );
    let mut wrong_workspace = input.clone();
    wrong_workspace.workspace_id = unfour_core::id::new_id();
    assert!(bus.patch_ssh_file(wrong_workspace).await.is_err());
    assert_eq!(probe.lock().unwrap().commands.len(), 1);
    sqlx::query("CREATE TRIGGER fail_file_activity BEFORE INSERT ON activity_events WHEN NEW.action = 'ssh.file.patch' BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;").execute(bus.db.pool()).await.unwrap();
    input.replace = "small".into();
    let result = bus.patch_ssh_file(input.clone()).await.unwrap();
    assert_eq!(result.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert_eq!(
        probe.lock().unwrap().commands.len(),
        2,
        "audit failure must not replay writes"
    );
    probe.lock().unwrap().hold = true;
    input.timeout_ms = Some(1000);
    assert!(matches!(
        bus.patch_ssh_file(input).await,
        Err(AppError::Timeout(_))
    ));
    assert_eq!(probe.lock().unwrap().commands.len(), 3);
    server_task.abort();
}
