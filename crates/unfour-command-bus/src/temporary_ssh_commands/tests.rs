use super::*;
use russh::server::{Msg, Server as _, Session};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
struct Server {
    calls: Arc<Mutex<Vec<&'static str>>>,
    stall: Option<&'static str>,
    forward_target: Option<std::net::SocketAddr>,
    downstream: bool,
    forwarding_channels: std::collections::HashSet<russh::ChannelId>,
    pending_open: Arc<tokio::sync::Notify>,
}
impl russh::server::Server for Server {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.calls.lock().unwrap().push("login");
        self.clone()
    }
}
impl russh::server::Handler for Server {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        if self.downstream {
            assert_eq!((user, password), ("target-user", "target-password"));
        } else {
            assert_eq!(
                (user, password),
                ("synthetic-ticket-user", "synthetic-password")
            );
        }
        self.calls.lock().unwrap().push("auth");
        if self.stall == Some("auth") {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        Ok(russh::server::Auth::Accept)
    }
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: russh::Channel<Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        let Some(address) = self.forward_target else {
            panic!("unrequested forward");
        };
        self.calls.lock().unwrap().push("forward");
        if host == "slow.fixture.invalid" {
            self.pending_open.notified().await;
            return Ok(false);
        }
        if host != "ssh-target.invalid" || port != 2222 {
            return Ok(false);
        }
        self.forwarding_channels.insert(channel.id());
        let mut target = tokio::net::TcpStream::connect(address).await?;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut target).await;
        });
        Ok(true)
    }
    async fn channel_open_session(
        &mut self,
        _: russh::Channel<Msg>,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.calls.lock().unwrap().push("channel");
        if self.stall == Some("channel") {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        Ok(true)
    }
    async fn pty_request(
        &mut self,
        channel: russh::ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.calls.lock().unwrap().push("pty");
        if self.stall == Some("reject-pty") {
            session.channel_failure(channel)?;
            return Ok(());
        }
        session.channel_success(channel)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        channel: russh::ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.calls.lock().unwrap().push("shell");
        if self.stall == Some("rekey-before-shell") {
            // Cross the server's test-only rekey threshold before acknowledging the shell.
            session.data(channel, vec![b'x'; 16 * 1024])?;
            return Ok(());
        }
        if self.stall == Some("shell") {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        if self.stall == Some("late-shell") {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if self.stall == Some("reject-shell") {
            session.channel_failure(channel)?;
            return Ok(());
        }
        session.channel_success(channel)?;
        session.data(channel, b"fixture-ready\r\n".to_vec())?;
        Ok(())
    }
    async fn data(
        &mut self,
        channel: russh::ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // russh delivers channel bytes to both the Channel reader and this
        // callback. The forwarding task consumes its own channels exclusively.
        if self.forwarding_channels.contains(&channel) {
            return Ok(());
        }
        self.calls.lock().unwrap().push("input");
        session.data(channel, data.to_vec())?;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        channel: russh::ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        assert!(self.downstream, "no exec probes on the bastion");
        assert_eq!(data, b"fixture-task");
        self.calls.lock().unwrap().push("task");
        session.channel_success(channel)?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)
    }
    async fn subsystem_request(
        &mut self,
        _: russh::ChannelId,
        _: &str,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        panic!("no SFTP probes")
    }
}

async fn wait_for(bus: &CommandBus, workspace: &str, status: &str) -> TemporarySummary {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(item) = bus
                .list_temporary_ssh(workspace)
                .unwrap()
                .into_iter()
                .find(|item| item.status == status)
            {
                break item;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "expected {status}: {:?}",
            bus.list_temporary_ssh(workspace).unwrap()
        )
    })
}

mod forwarding;

#[tokio::test]
async fn temporary_rekey_requires_a_new_launch_during_setup_and_after_connecting() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for during_setup in [true, false] {
        let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = socket.local_addr().unwrap().port();
        let mut server = Server {
            stall: during_setup.then_some("rekey-before-shell"),
            ..Default::default()
        };
        let config = Arc::new(russh::server::Config {
            keys: vec![russh::keys::PrivateKey::random(
                &mut rand::rng(),
                russh::keys::Algorithm::Ed25519,
            )
            .unwrap()],
            limits: russh::Limits {
                rekey_write_limit: 8 * 1024,
                ..Default::default()
            },
            ..Default::default()
        });
        let task =
            tokio::spawn(async move { server.run_on_socket(config, &socket).await.unwrap() });
        let request = TemporaryRequest::password(
            "127.0.0.1".into(),
            port,
            "synthetic-ticket-user".into(),
            "synthetic-password".into(),
        )
        .unwrap();
        let handle = bus
            .receive_temporary_ssh(&LaunchReceipt::default(), request)
            .unwrap();
        bus.claim_temporary_ssh(&workspace, &handle).await.unwrap();
        bus.start_temporary_ssh(&workspace, &handle).unwrap();
        let trust = wait_for(&bus, &workspace, "awaiting-trust").await;
        bus.trust_temporary_ssh(&workspace, &handle, trust.fingerprint.as_deref().unwrap())
            .unwrap();
        let mut forwarding = None;
        if !during_setup {
            wait_for(&bus, &workspace, "connected").await;
            let tunnel = bus.ssh.temporary_tunnel(&workspace, &handle).unwrap();
            forwarding = Some((tunnel.http_proxy().await.unwrap(), tunnel));
            bus.temporary_ssh_input(SshSessionInput {
                workspace_id: workspace.clone(),
                session_id: handle.clone(),
                data: "x".repeat(16 * 1024),
            })
            .await
            .unwrap();
        }
        let ended = wait_for(&bus, &workspace, "disconnected").await;
        assert_eq!(
            ended.error_code.as_deref(),
            Some("TEMPORARY_REKEY_REQUIRES_RELAUNCH")
        );
        if let Some((lease, tunnel)) = forwarding {
            tokio::time::timeout(Duration::from_secs(2), tunnel.closed())
                .await
                .unwrap();
            assert!(tokio::net::TcpStream::connect(lease.address())
                .await
                .is_err());
            assert!(tunnel.connect("ssh-target.invalid", 2222).await.is_err());
        }
        bus.cancel_temporary_ssh(&workspace, &handle).unwrap();
        task.abort();
    }
}

#[tokio::test]
async fn temporary_loopback_terminal_single_login_channel_and_zero_database_writes() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let before: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let mut server = Server::default();
    let calls = server.calls.clone();
    let config = Arc::new(russh::server::Config {
        keys: vec![russh::keys::PrivateKey::random(
            &mut rand::rng(),
            russh::keys::Algorithm::Ed25519,
        )
        .unwrap()],
        ..Default::default()
    });
    let task = tokio::spawn(async move {
        server.run_on_socket(config, &socket).await.unwrap();
    });
    let receipt = LaunchReceipt::default();
    let make_request = || {
        TemporaryRequest::password(
            "127.0.0.1".into(),
            port,
            "synthetic-ticket-user".into(),
            "synthetic-password".into(),
        )
        .unwrap()
    };
    let handle = bus.receive_temporary_ssh(&receipt, make_request()).unwrap();
    assert_eq!(
        bus.receive_temporary_ssh(&receipt, make_request()).unwrap(),
        handle
    );
    bus.claim_temporary_ssh(&workspace, &handle).await.unwrap();
    bus.start_temporary_ssh(&workspace, &handle).unwrap();
    let trust = wait_for(&bus, &workspace, "awaiting-trust").await;
    assert_eq!(calls.lock().unwrap().as_slice(), ["login"]);
    bus.trust_temporary_ssh(&workspace, &handle, trust.fingerprint.as_deref().unwrap())
        .unwrap();
    wait_for(&bus, &workspace, "connected").await;
    assert!(bus.start_temporary_ssh(&workspace, &handle).is_err());
    bus.temporary_ssh_input(SshSessionInput {
        workspace_id: workspace.clone(),
        session_id: handle.clone(),
        data: "fixture-one\r\n".into(),
    })
    .await
    .unwrap();
    bus.temporary_ssh_resize(SshResizeInput {
        workspace_id: workspace.clone(),
        session_id: handle.clone(),
        cols: 80,
        rows: 24,
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut output = String::new();
        loop {
            for event in bus.temporary_ssh_output(&workspace, &handle).unwrap() {
                output.push_str(&event.data);
            }
            if output.contains("fixture-one") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(output.contains("fixture-ready"));
    })
    .await
    .unwrap();
    assert!(bus
        .list_ssh_connections(workspace.clone())
        .await
        .unwrap()
        .is_empty());
    assert!(bus
        .ssh
        .export_log(SshLogExportInput {
            workspace_id: workspace.clone(),
            session_id: handle.clone()
        })
        .await
        .is_err());
    bus.close_temporary_ssh_workspace(&workspace).unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(bus.list_temporary_ssh(&workspace).unwrap().is_empty());
    assert!(bus.temporary_ssh_output(&workspace, &handle).is_err());
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["login", "auth", "channel", "pty", "shell", "input"]
    );
    let after: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "no history, host trust, activity, connection, sync or credential writes"
    );
    task.abort();
}

async fn stalled_session(stage: &'static str, cancel: bool) {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let mut server = Server {
        stall: Some(stage),
        ..Default::default()
    };
    let calls = server.calls.clone();
    let config = Arc::new(russh::server::Config {
        keys: vec![russh::keys::PrivateKey::random(
            &mut rand::rng(),
            russh::keys::Algorithm::Ed25519,
        )
        .unwrap()],
        ..Default::default()
    });
    let task = tokio::spawn(async move {
        server.run_on_socket(config, &socket).await.unwrap();
    });
    let handle = bus
        .receive_temporary_ssh(
            &LaunchReceipt::default(),
            TemporaryRequest::password(
                "127.0.0.1".into(),
                port,
                "synthetic-ticket-user".into(),
                "synthetic-password".into(),
            )
            .unwrap(),
        )
        .unwrap();
    bus.claim_temporary_ssh(&workspace, &handle).await.unwrap();
    bus.start_temporary_ssh(&workspace, &handle).unwrap();
    let trust = wait_for(&bus, &workspace, "awaiting-trust").await;
    bus.trust_temporary_ssh(&workspace, &handle, trust.fingerprint.as_deref().unwrap())
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !calls.lock().unwrap().contains(&match stage {
            "late-shell" | "reject-shell" => "shell",
            "reject-pty" => "pty",
            _ => stage,
        }) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    if cancel {
        bus.cancel_temporary_ssh(&workspace, &handle).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(bus.list_temporary_ssh(&workspace).unwrap().is_empty());
    } else {
        let result = tokio::time::timeout(Duration::from_secs(35), async {
            loop {
                let entry = bus.list_temporary_ssh(&workspace).unwrap().remove(0);
                if entry.status == "disconnected" {
                    break entry;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            result.error_code.as_deref(),
            Some(if stage.starts_with("reject-") {
                "TEMPORARY_CHANNEL_FAILED"
            } else if stage == "auth" {
                "TEMPORARY_AUTH_TIMEOUT"
            } else {
                "TEMPORARY_CHANNEL_TIMEOUT"
            })
        );
        bus.cancel_temporary_ssh(&workspace, &handle).unwrap();
    }
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|item| **item == "login")
            .count(),
        1
    );
    if matches!(stage, "auth" | "channel" | "reject-pty") {
        assert!(!calls.lock().unwrap().contains(&"shell"));
    }
    task.abort();
}

#[tokio::test]
async fn temporary_auth_and_channel_timeouts() {
    tokio::join!(
        stalled_session("auth", false),
        stalled_session("channel", false),
        stalled_session("shell", false)
    );
}
#[tokio::test]
async fn temporary_cancel_during_auth_and_channel_setup() {
    tokio::join!(
        stalled_session("auth", true),
        stalled_session("channel", true),
        stalled_session("late-shell", true)
    );
}

#[tokio::test]
async fn temporary_pty_and_shell_rejection_never_report_connected() {
    tokio::join!(
        stalled_session("reject-pty", false),
        stalled_session("reject-shell", false)
    );
}
