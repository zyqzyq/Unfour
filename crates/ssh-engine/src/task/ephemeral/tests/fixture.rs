use super::*;
#[path = "sftp.rs"]
mod sftp;
#[path = "terminal_fixture.rs"]
mod terminal_fixture;
#[derive(Default)]
pub(super) struct State {
    pub(super) auths: usize,
    pub(super) live: HashSet<ChannelId>,
    pub(super) closed: usize,
    pub(super) commands: Vec<String>,
    pub(super) sftp_requests: usize,
    pub(super) files: HashMap<String, Vec<u8>>,
    pub(super) hold_open: Option<Arc<tokio::sync::Notify>>,
    pub(super) opened: Option<Arc<tokio::sync::Notify>>,
    pub(super) terminal_enabled: bool,
    pub(super) terminal_commands: Vec<String>,
    pub(super) terminal_wire: Vec<u8>,
    pub(super) terminal_ignore_interrupt: bool,
    pub(super) terminal_ignore_restore: bool,
    pub(super) terminal_task_drop_line: Option<usize>,
    pub(super) terminal_task_stall: Option<&'static str>,
    pub(super) terminal_task_stdout: Option<Vec<u8>>,
    pub(super) terminal_raw_missing_tool: bool,
    pub(super) terminal_raw_printf_external: bool,
    pub(super) terminal_octal: bool,
    pub(super) terminal_line_delay: Option<Duration>,
    pub(super) terminal_raw_stall: Option<&'static str>,
    pub(super) terminal_raw_foreign_restore: bool,
    pub(super) terminal_raw_early_restore: bool,
    pub(super) terminal_raw_auto_restore: bool,
    pub(super) terminal_raw_ack_gate: Option<Arc<tokio::sync::Semaphore>>,
    pub(super) terminal_raw_input_lines: Vec<usize>,
    pub(super) terminal_raw_pending_input_acks: usize,
    pub(super) terminal_raw_input_acks_sent: usize,
    pub(super) terminal_raw_pipelined_input: bool,
    pub(super) terminal_raw_input_end_received: usize,
    pub(super) terminal_raw_truncate_input_line: Option<usize>,
    pub(super) terminal_raw_mutate_input_line: Option<usize>,
    pub(super) terminal_raw_wrong_input_index: bool,
    pub(super) terminal_raw_corrupt_end_total: bool,
    pub(super) terminal_raw_corrupt_end_ack: bool,
    pub(super) terminal_channel_messages_drained: usize,
    pub(super) terminal_no_digest: bool,
    pub(super) terminal_context: Option<char>,
    pub(super) terminal_curl_code: Option<i32>,
    pub(super) terminal_transport_code: Option<i32>,
    pub(super) terminal_transport_corrupt_output: bool,
    pub(super) terminal_raw_restore_gate: Option<Arc<tokio::sync::Semaphore>>,
    pub(super) terminal_raw_restores_waiting: usize,
    pub(super) terminal_raw_restores_sent: usize,
    pub(super) terminal_login_prompt: Option<&'static str>,
}
struct Server {
    pub(super) state: Arc<Mutex<State>>,
    channels: HashMap<ChannelId, Channel<Msg>>,
    terminal: terminal_fixture::Terminal,
}
impl russh::server::Handler for Server {
    type Error = russh::Error;
    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        assert_eq!((user, password), ("synthetic-user", "synthetic-password"));
        self.state.lock().unwrap().auths += 1;
        Ok(Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        let (hold, opened) = {
            let mut state = self.state.lock().unwrap();
            state.live.insert(channel.id());
            (state.hold_open.take(), state.opened.take())
        };
        self.channels.insert(channel.id(), channel);
        if let Some(opened) = opened {
            opened.notify_one();
        }
        if let Some(hold) = hold {
            hold.notified().await;
        }
        Ok(true)
    }
    async fn channel_close(&mut self, id: ChannelId, _: &mut Session) -> Result<(), Self::Error> {
        let mut state = self.state.lock().unwrap();
        state.live.remove(&id);
        state.closed += 1;
        self.channels.remove(&id);
        Ok(())
    }
    async fn pty_request(
        &mut self,
        id: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        id: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        Ok(())
    }
    async fn data(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.state.lock().unwrap().terminal_enabled {
            // russh duplicates incoming messages into the Channel receiver
            // before invoking this handler. This fixture executes via the
            // handler, so consume that duplicate queue instead of blocking
            // the SSH session when its bounded 100-message buffer fills.
            if let Some(channel) = self.channels.get_mut(&id) {
                while matches!(
                    tokio::time::timeout(Duration::ZERO, channel.wait()).await,
                    Ok(Some(_))
                ) {
                    self.state.lock().unwrap().terminal_channel_messages_drained += 1;
                }
            }
            let delay = self.state.lock().unwrap().terminal_line_delay;
            if let Some(delay) = delay {
                for part in data.split_inclusive(|byte| *byte == b'\n') {
                    if part.ends_with(b"\n") {
                        tokio::time::sleep(delay).await;
                    }
                    self.terminal.feed(id, part, session, &self.state)?;
                }
                return Ok(());
            }
            return self.terminal.feed(id, data, session, &self.state);
        }
        if self.channels.contains_key(&id) {
            session.data(id, data.to_vec())?;
        }
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(name, "sftp");
        self.state.lock().unwrap().sftp_requests += 1;
        session.channel_success(id)?;
        let channel = self.channels.remove(&id).unwrap();
        russh_sftp::server::run(channel.into_stream(), sftp::Server(self.state.clone())).await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8(data.to_vec()).unwrap();
        self.state.lock().unwrap().commands.push(command.clone());
        if command == "reject" {
            session.channel_failure(id)?;
            return Ok(());
        }
        session.channel_success(id)?;
        session.data(id, format!("output:{command}\n").into_bytes())?;
        session.extended_data(id, 1, b"stderr-marker\n".to_vec())?;
        if command == "hold" {
            return Ok(());
        }
        session.eof(id)?;
        if command != "unknown" {
            session.exit_status_request(id, if command == "fail" { 23 } else { 0 })?;
        }
        session.close(id)?;
        self.state.lock().unwrap().live.remove(&id);
        self.channels.remove(&id);
        Ok(())
    }
}
pub(super) struct Fixture {
    pub(super) service: SshService,
    pub(super) workspace: String,
    pub(super) handle: String,
    pub(super) state: Arc<Mutex<State>>,
    pub(super) server: tokio::task::JoinHandle<()>,
    pub(super) _dir: tempfile::TempDir,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(gate) = &self.state.lock().unwrap().terminal_raw_ack_gate {
            gate.close();
        }
        let _ = self.service.cancel_temporary(&self.workspace, &self.handle);
        self.server.abort();
    }
}
pub(super) async fn fixture() -> Fixture {
    fixture_with_window(2 * 1024 * 1024).await
}
pub(super) async fn fixture_with_window(window_size: u32) -> Fixture {
    let state = Arc::new(Mutex::new(State::default()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let key = ssh_key::private::Ed25519Keypair::from_seed(&[43; 32]).into();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::ZERO,
        window_size,
        ..Default::default()
    });
    let server_state = state.clone();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let session = russh::server::run_stream(
            config,
            stream,
            Server {
                state: server_state,
                channels: HashMap::new(),
                terminal: terminal_fixture::Terminal::default(),
            },
        )
        .await
        .unwrap();
        let _ = session.await;
    });
    let db = LocalDb::connect_ephemeral().await.unwrap();
    db.migrate().await.unwrap();
    let workspace = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO workspaces (id,name,is_default,created_at,updated_at,revision,sync_status) VALUES (?,'task fixture',0,'now','now',1,'local')").bind(&workspace).execute(db.pool()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let service = SshService::new(db, SecretStore::in_memory("temporary-task-test"))
        .with_task_log_dir(dir.path().join("logs"));
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
    until(|| service.list_temporary(&workspace).unwrap()[0].status == "connected").await;
    Fixture {
        service,
        workspace,
        handle,
        state,
        server,
        _dir: dir,
    }
}

pub(super) async fn login_prompt_at(f: &Fixture, stage: &'static str) {
    if stage == "SPLIT_STARTUP" {
        f.service
            .temporary_input(SshSessionInput {
                workspace_id: f.workspace.clone(),
                session_id: f.handle.clone(),
                data: "Pass".into(),
            })
            .await
            .unwrap();
        let mut displayed = String::new();
        until(|| {
            for event in f.service.temporary_output(&f.workspace, &f.handle).unwrap() {
                displayed.push_str(&event.data);
            }
            displayed.ends_with("Pass")
        })
        .await;
        f.state.lock().unwrap().terminal_wire.clear();
    }
    f.state.lock().unwrap().terminal_login_prompt = Some(stage);
}

pub(super) async fn until(mut check: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(8), async {
        while !check() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
pub(super) fn step(kind: &str, config_json: serde_json::Value) -> SshTaskStepInput {
    SshTaskStepInput {
        id: None,
        name: kind.into(),
        step_type: kind.into(),
        position: 0,
        enabled: true,
        config_version: None,
        config_json,
    }
}
pub(super) fn command(command: &str) -> SshTaskStepInput {
    step(
        "command",
        serde_json::json!({"command":command,"workingDirectory":"","timeoutSeconds":30,"continueOnError":false}),
    )
}
pub(super) async fn input(f: &Fixture, mut steps: Vec<SshTaskStepInput>) -> SshTaskRunInput {
    for (position, step) in steps.iter_mut().enumerate() {
        step.position = position as i64;
    }
    let detail = f
        .service
        .save_task(SshTaskSaveInput {
            id: None,
            workspace_id: f.workspace.clone(),
            name: "Saved task without saved connection".into(),
            description: String::new(),
            default_connection_id: None,
            steps,
        })
        .await
        .unwrap();
    SshTaskRunInput {
        workspace_id: f.workspace.clone(),
        task_id: detail.task.id.clone(),
        connection_id: None,
        inputs: Default::default(),
        secret_input_names: vec![],
        context: Some(unfour_core::models::SshTaskRunContextInput {
            environment_id: None,
            task_version: context::task_version(&detail),
            environment_updated_at: None,
            input_sources: Default::default(),
            input_versions: None,
        }),
    }
}
pub(super) async fn finished(f: &Fixture, run: &SshTaskRun) -> SshTaskRun {
    until(|| {
        f.service
            .temporary_task_run(&f.workspace, &run.id)
            .unwrap()
            .is_some_and(|r| r.status != "running")
    })
    .await;
    f.service.get_task_run(&f.workspace, &run.id).await.unwrap()
}
pub(super) async fn no_run_persistence(f: &Fixture) {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_run")
        .fetch_one(f.service.db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ssh_task_local_binding WHERE last_used_connection_id IS NOT NULL",
    )
    .fetch_one(f.service.db.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    assert!(!f.service.task_log_dir.exists());
}
