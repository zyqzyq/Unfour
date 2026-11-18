use super::*;
use russh::server::{Msg, Server as _, Session};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod curl;
#[path = "../../examples/bastion_fixture/raw_terminal.rs"]
mod raw_terminal;
mod task;

#[derive(Clone, Default)]
struct Server {
    calls: Arc<Mutex<Vec<&'static str>>>,
    stall: Option<&'static str>,
    forward_target: Option<std::net::SocketAddr>,
    downstream: bool,
    temporary_task: bool,
    raw_terminal: Option<raw_terminal::RawTerminal>,
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
            // Cross the test-only byte limit and start KEX before enqueueing
            // the shell ACK. The server Handle queue is not consumed during
            // KEX, so receiving its ACK/marker proves setup survived a rekey.
            session.data(channel, vec![b'x'; 16 * 1024])?;
            session.flush()?;
            let handle = session.handle();
            tokio::spawn(async move {
                handle.channel_success(channel).await.unwrap();
                handle
                    .data(channel, b"setup-rekey-complete\r\n".to_vec())
                    .await
                    .unwrap();
            });
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
        if let Some(raw) = &mut self.raw_terminal {
            if raw.active() || data.starts_with(b" sh -c '") {
                return raw.feed(channel, data, session);
            }
        }
        session.data(channel, data.to_vec())?;
        if self.stall == Some("rekey-after-input") && data.len() >= 16 * 1024 {
            session.flush()?;
            let handle = session.handle();
            tokio::spawn(async move {
                handle
                    .data(channel, b"connected-rekey-complete\r\n".to_vec())
                    .await
                    .unwrap();
            });
        }
        Ok(())
    }
    async fn exec_request(
        &mut self,
        channel: russh::ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        assert!(
            self.downstream || self.temporary_task,
            "no unsolicited exec on the bastion"
        );
        assert_eq!(data, b"fixture-task");
        self.calls.lock().unwrap().push("task");
        session.channel_success(channel)?;
        session.data(channel, b"synthetic-task-output\n".to_vec())?;
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
async fn temporary_rekey_preserves_setup_terminal_and_existing_tunnel() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn output_contains(bus: &CommandBus, workspace: &str, handle: &str, expected: &str) {
        let mut output = String::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !output.contains(expected) {
                for event in bus.temporary_ssh_output(workspace, handle).unwrap() {
                    output.push_str(&event.data);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("expected output after completed rekey");
    }
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for during_setup in [true, false] {
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_address = target.local_addr().unwrap();
        let echo = tokio::spawn(async move {
            let (mut socket, _) = target.accept().await.unwrap();
            let (mut reader, mut writer) = socket.split();
            let _ = tokio::io::copy(&mut reader, &mut writer).await;
        });
        let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = socket.local_addr().unwrap().port();
        let mut server = Server {
            stall: Some(if during_setup {
                "rekey-before-shell"
            } else {
                "rekey-after-input"
            }),
            forward_target: Some(target_address),
            ..Default::default()
        };
        let calls = server.calls.clone();
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
        wait_for(&bus, &workspace, "connected").await;
        let tunnel = bus.ssh.temporary_tunnel(&workspace, &handle).unwrap();
        let lease = tunnel.http_proxy().await.unwrap();
        let mut forwarded = tunnel.connect("ssh-target.invalid", 2222).await.unwrap();
        forwarded.write_all(b"before").await.unwrap();
        let mut before = [0; 6];
        tokio::time::timeout(Duration::from_secs(2), forwarded.read_exact(&mut before))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&before, b"before");
        if !during_setup {
            bus.temporary_ssh_input(SshSessionInput {
                workspace_id: workspace.clone(),
                session_id: handle.clone(),
                data: "x".repeat(16 * 1024),
            })
            .await
            .unwrap();
        }
        output_contains(
            &bus,
            &workspace,
            &handle,
            if during_setup {
                "setup-rekey-complete"
            } else {
                "connected-rekey-complete"
            },
        )
        .await;
        let connected = wait_for(&bus, &workspace, "connected").await;
        assert!(connected.error_code.is_none());
        bus.temporary_ssh_input(SshSessionInput {
            workspace_id: workspace.clone(),
            session_id: handle.clone(),
            data: "after-rekey\n".into(),
        })
        .await
        .unwrap();
        output_contains(&bus, &workspace, &handle, "after-rekey\n").await;
        forwarded.write_all(b"after").await.unwrap();
        let mut after = [0; 5];
        tokio::time::timeout(Duration::from_secs(2), forwarded.read_exact(&mut after))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&after, b"after");
        assert!(tokio::net::TcpStream::connect(lease.address())
            .await
            .is_ok());
        {
            let calls = calls.lock().unwrap();
            for expected in ["login", "auth", "channel", "pty", "shell", "forward"] {
                assert_eq!(
                    calls.iter().filter(|&&call| call == expected).count(),
                    1,
                    "{expected}: {calls:?}"
                );
            }
        }
        bus.cancel_temporary_ssh(&workspace, &handle).unwrap();
        tokio::time::timeout(Duration::from_secs(2), tunnel.closed())
            .await
            .unwrap();
        // Wait for the proxy itself to observe parent cancellation and drop
        // its serve future. The parent's watch notification alone is earlier.
        tokio::time::timeout(Duration::from_secs(2), async {
            while lease.remote_failure_code() != Some("SSH_TUNNEL_TEMPORARY_UNAVAILABLE") {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("proxy must observe parent cancellation");
        // Windows can take just over two seconds to report WSAECONNREFUSED
        // even for a socket that was already closed before connect began.
        assert!(tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::TcpStream::connect(lease.address()),
        )
        .await
        .expect("closed proxy must finish the connection attempt")
        .is_err());
        assert!(tunnel.connect("ssh-target.invalid", 2222).await.is_err());
        task.abort();
        echo.abort();
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
    wait_for(&bus, &workspace, "connected").await;
    assert!(bus.start_temporary_ssh(&workspace, &handle).is_err());
    assert!(bus
        .temporary_ssh_probe(&workspace, &handle)
        .unwrap()
        .is_none());
    for kind in [
        TemporaryProbeKind::Exec,
        TemporaryProbeKind::Channels,
        TemporaryProbeKind::Sftp,
        TemporaryProbeKind::Terminal,
        TemporaryProbeKind::Transport,
        TemporaryProbeKind::Curl,
    ] {
        assert!(bus
            .start_temporary_ssh_probe(&workspace, &handle, kind, false)
            .is_err());
    }
    assert!(bus.temporary_ssh_probe("other-workspace", &handle).is_err());
    assert!(bus
        .start_temporary_ssh_probe("other-workspace", &handle, TemporaryProbeKind::Exec, true)
        .is_err());
    assert!(bus
        .cancel_temporary_ssh_probe("other-workspace", &handle, "unknown-run")
        .is_err());
    assert!(bus
        .temporary_ssh_probe(&workspace, &handle)
        .unwrap()
        .is_none());
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
