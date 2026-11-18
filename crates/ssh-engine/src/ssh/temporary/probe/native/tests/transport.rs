use super::*;
use russh::server::{Auth, Msg, Session};
use russh::ChannelId;
mod tcp;

#[derive(Default)]
struct State {
    auths: usize,
    live: std::collections::HashSet<ChannelId>,
    max_live: usize,
    closed: usize,
    commands: Vec<Vec<u8>>,
    hold: Option<Arc<tokio::sync::Notify>>,
    opened: Option<Arc<tokio::sync::Notify>>,
    reject_exec: bool,
    omit_exit: bool,
    sftp_requests: usize,
    direct_requests: usize,
    direct_data_bytes: usize,
    direct_channels: std::collections::HashSet<ChannelId>,
    reject_direct: bool,
    disconnect_direct: bool,
    forward_target: Option<std::net::SocketAddr>,
}
struct Server(Arc<Mutex<State>>);
impl russh::server::Handler for Server {
    type Error = russh::Error;
    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        assert_eq!((user, password), ("synthetic-user", "synthetic-password"));
        self.0.lock().unwrap().auths += 1;
        Ok(Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        let (hold, opened) = {
            let mut state = self.0.lock().unwrap();
            state.live.insert(channel.id());
            state.max_live = state.max_live.max(state.live.len());
            (state.hold.take(), state.opened.take())
        };
        if let Some(opened) = opened {
            opened.notify_one();
        }
        if let Some(hold) = hold {
            hold.notified().await;
        }
        Ok(true)
    }
    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut state = self.0.lock().unwrap();
        state.live.remove(&channel);
        state.closed += 1;
        Ok(())
    }
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        assert_eq!((host, port), ("tcp.fixture.invalid", 32123));
        let (hold, opened, reject, disconnect, target) = {
            let mut state = self.0.lock().unwrap();
            state.direct_requests += 1;
            state.direct_channels.insert(channel.id());
            if !state.reject_direct {
                state.live.insert(channel.id());
            }
            (
                state.hold.take(),
                state.opened.take(),
                state.reject_direct,
                state.disconnect_direct,
                state.forward_target,
            )
        };
        if let Some(opened) = opened {
            opened.notify_one();
        }
        if let Some(hold) = hold {
            hold.notified().await;
        }
        if disconnect {
            return Err(russh::Error::Disconnect);
        }
        if reject {
            return Ok(false);
        }
        if let Some(address) = target {
            let mut target = tokio::net::TcpStream::connect(address).await?;
            tokio::spawn(async move {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut stream, &mut target).await;
            });
        }
        Ok(true)
    }
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(name, "sftp");
        self.0.lock().unwrap().sftp_requests += 1;
        session.channel_success(channel)?;
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        {
            let mut state = self.0.lock().unwrap();
            if state.direct_channels.contains(&channel) {
                state.direct_data_bytes += data.len();
                return Ok(());
            }
        }
        session.data(channel, data.to_vec())?;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        assert!(data == EXEC_OK.as_bytes() || data == EXEC_NONZERO.as_bytes());
        let (reject, omit) = {
            let mut state = self.0.lock().unwrap();
            state.commands.push(data.to_vec());
            (state.reject_exec, state.omit_exit)
        };
        if reject {
            session.channel_failure(channel)?;
            return Ok(());
        }
        session.channel_success(channel)?;
        session.data(channel, STDOUT.to_vec())?;
        session.extended_data(channel, 1, STDERR.to_vec())?;
        session.eof(channel)?;
        if !omit {
            session.exit_status_request(
                channel,
                if data == EXEC_NONZERO.as_bytes() {
                    23
                } else {
                    0
                },
            )?;
        }
        session.close(channel)?;
        self.0.lock().unwrap().live.remove(&channel);
        Ok(())
    }
}

struct Fixture {
    service: SshService,
    workspace: String,
    handle: String,
    state: Arc<Mutex<State>>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.service.cancel_temporary(&self.workspace, &self.handle);
        self.server.abort();
    }
}

async fn fixture() -> Fixture {
    fixture_with_window(2 * 1024 * 1024).await
}

async fn fixture_with_window(window_size: u32) -> Fixture {
    let state = Arc::new(Mutex::new(State::default()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let key = ssh_key::private::Ed25519Keypair::from_seed(&[42; 32]).into();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::ZERO,
        window_size,
        ..Default::default()
    });
    let server_state = state.clone();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let session = russh::server::run_stream(config, stream, Server(server_state))
            .await
            .expect("fixture SSH handshake");
        // Parent cancellation intentionally aborts this socket; either clean
        // EOF or a platform connection-aborted error is expected on teardown.
        let _ = session.await;
    });
    let db = LocalDb::connect_ephemeral().await.unwrap();
    db.migrate().await.unwrap();
    let workspace = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO workspaces (id,name,is_default,created_at,updated_at,revision,sync_status) VALUES (?,'probe fixture',0,'now','now',1,'local')").bind(&workspace).execute(db.pool()).await.unwrap();
    let service = SshService::new(db, SecretStore::in_memory("temporary-probe-test"));
    let request = TemporaryRequest::password(
        "127.0.0.1".into(),
        port,
        "synthetic-user".into(),
        "synthetic-password".into(),
    )
    .unwrap();
    let handle = service
        .receive_temporary(&LaunchReceipt::default(), request)
        .unwrap();
    service.claim_temporary(&workspace, &handle).await.unwrap();
    service.start_temporary(&workspace, &handle).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let summary = service.list_temporary(&workspace).unwrap().remove(0);
            if summary.status == "connected" {
                break;
            }
            assert!(
                summary.error_code.is_none(),
                "fixture setup: {:?}",
                summary.error_code
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    Fixture {
        service,
        workspace,
        handle,
        state,
        server,
    }
}

async fn finished(f: &Fixture) -> TemporaryProbeReport {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let report = f
                .service
                .temporary_probe(&f.workspace, &f.handle)
                .unwrap()
                .unwrap();
            if report.status != "running" {
                return report;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn probes_reuse_one_auth_preserve_pty_and_make_no_database_changes() {
    let f = fixture().await;
    let before: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    for kind in [
        TemporaryProbeKind::Exec,
        TemporaryProbeKind::Channels,
        TemporaryProbeKind::Sftp,
    ] {
        assert!(f
            .service
            .start_temporary_probe(&f.workspace, &f.handle, kind, false)
            .is_err());
    }
    assert!(f.service.temporary_probe("wrong", &f.handle).is_err());
    for kind in [TemporaryProbeKind::Exec, TemporaryProbeKind::Channels] {
        let initial = f
            .service
            .start_temporary_probe(&f.workspace, &f.handle, kind, true)
            .unwrap();
        assert_eq!(initial.status, "running");
        assert!(f
            .service
            .start_temporary_probe(&f.workspace, &f.handle, kind, true)
            .is_err());
        assert!(f
            .service
            .cancel_temporary_probe("wrong", &f.handle, &initial.run_id)
            .is_err());
        let report = finished(&f).await;
        assert_eq!(report.status, "completed");
        assert!(
            report.checks.iter().all(|check| check.status == "pass"),
            "{report:?}"
        );
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("synthetic-") && !serialized.contains("UNFOUR_A4_"));
    }
    let state = f.state.lock().unwrap();
    assert_eq!(state.auths, 1);
    assert_eq!(state.max_live, 3, "original PTY plus two extra channels");
    assert_eq!(state.live.len(), 1);
    assert_eq!(state.commands.len(), 4);
    drop(state);
    f.service
        .temporary_input(SshSessionInput {
            workspace_id: f.workspace.clone(),
            session_id: f.handle.clone(),
            data: "still-alive".into(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if f.service
                .temporary_output(&f.workspace, &f.handle)
                .unwrap()
                .iter()
                .any(|event| event.data.contains("still-alive"))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let after: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn late_channel_after_cancel_or_timeout_is_closed_without_parent_loss() {
    for cancel in [true, false] {
        let f = fixture().await;
        let hold = Arc::new(tokio::sync::Notify::new());
        let opened = Arc::new(tokio::sync::Notify::new());
        {
            let mut state = f.state.lock().unwrap();
            state.hold = Some(hold.clone());
            state.opened = Some(opened.clone());
        }
        let initial = f
            .service
            .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Exec, true)
            .unwrap();
        opened.notified().await;
        if cancel {
            f.service
                .cancel_temporary_probe(&f.workspace, &f.handle, &initial.run_id)
                .unwrap();
        } else {
            tokio::time::pause();
            tokio::time::advance(STEP_TIMEOUT + Duration::from_secs(1)).await;
            tokio::time::resume();
        }
        let report = finished(&f).await;
        assert_eq!(
            report.checks[0].code,
            if cancel { "CANCELLED" } else { "TIMEOUT" }
        );
        hold.notify_one();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if f.state.lock().unwrap().closed > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(f.state.lock().unwrap().live.len(), 1);
        f.service
            .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Exec, true)
            .unwrap();
        f.service
            .cancel_temporary_probe(&f.workspace, &f.handle, &initial.run_id)
            .unwrap();
        assert!(finished(&f)
            .await
            .checks
            .iter()
            .all(|check| check.status == "pass"));
        assert_eq!(f.state.lock().unwrap().auths, 1);
    }
}

#[tokio::test]
async fn explicit_refusal_and_missing_exit_are_not_false_passes() {
    for omit in [true, false] {
        let f = fixture().await;
        {
            let mut state = f.state.lock().unwrap();
            state.omit_exit = omit;
            state.reject_exec = !omit;
        }
        f.service
            .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Exec, true)
            .unwrap();
        let report = finished(&f).await;
        assert_eq!(
            report.checks[0].code,
            if omit {
                "EXIT_STATUS_MISSING"
            } else {
                "EXEC_REJECTED"
            }
        );
        assert_eq!(
            report.checks[0].status,
            if omit { "inconclusive" } else { "unsupported" }
        );
    }
}

#[tokio::test]
async fn parent_cancellation_preserves_report_until_explicit_close_without_resurrection() {
    let f = fixture().await;
    let hold = Arc::new(tokio::sync::Notify::new());
    let opened = Arc::new(tokio::sync::Notify::new());
    {
        let mut state = f.state.lock().unwrap();
        state.hold = Some(hold.clone());
        state.opened = Some(opened.clone());
    }
    f.service
        .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Exec, true)
        .unwrap();
    opened.notified().await;
    {
        let mut queue = f.service.temporary.0.lock().unwrap();
        queue
            .owned(&f.workspace, &f.handle)
            .unwrap()
            .cancel
            .as_ref()
            .unwrap()
            .send_replace(true);
    }
    let report = finished(&f).await;
    assert_eq!(report.status, "cancelled");
    hold.notify_one();
    {
        let mut queue = f.service.temporary.0.lock().unwrap();
        let entry = queue.owned(&f.workspace, &f.handle).unwrap();
        entry.summary.status = "disconnected".into();
        entry.deadline = Instant::now() - Duration::from_secs(1);
        queue.sweep(Instant::now());
        assert!(queue
            .owned(&f.workspace, &f.handle)
            .unwrap()
            .probe
            .is_some());
    }
    assert_eq!(
        f.service
            .temporary_probe(&f.workspace, &f.handle)
            .unwrap()
            .unwrap()
            .run_id,
        report.run_id
    );
    f.service.close_temporary_workspace(&f.workspace).unwrap();
    tokio::task::yield_now().await;
    assert!(f.service.temporary_probe(&f.workspace, &f.handle).is_err());
    assert!(f.service.temporary.0.lock().unwrap().entries.is_empty());
}

#[tokio::test]
async fn blocked_sftp_writer_is_closed_on_cancel_and_timeout_without_closing_pty() {
    for cancel in [true, false] {
        let f = fixture_with_window(0).await;
        let report = f
            .service
            .start_temporary_probe(&f.workspace, &f.handle, TemporaryProbeKind::Sftp, true)
            .unwrap();
        assert_eq!(report.cleanup, None);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if f.state.lock().unwrap().sftp_requests == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        if cancel {
            f.service
                .cancel_temporary_probe(&f.workspace, &f.handle, &report.run_id)
                .unwrap();
        } else {
            tokio::time::pause();
            tokio::time::advance(STEP_TIMEOUT + Duration::from_secs(1)).await;
            tokio::time::resume();
        }
        let report = finished(&f).await;
        assert_eq!(
            report.checks[0].code,
            if cancel { "CANCELLED" } else { "TIMEOUT" }
        );
        assert_eq!(report.cleanup.as_deref(), Some("not-created"));
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if f.state.lock().unwrap().closed == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            f.state.lock().unwrap().live.len(),
            1,
            "only original PTY stays open"
        );
        assert_eq!(f.state.lock().unwrap().auths, 1);
        assert_eq!(
            f.service.list_temporary(&f.workspace).unwrap()[0].status,
            "connected"
        );
    }
}
