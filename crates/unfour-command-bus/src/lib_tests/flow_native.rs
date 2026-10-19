use super::*;
use russh::server::{Msg, Server as _, Session};

#[derive(Clone)]
pub(super) struct Server {
    pub(super) commands: Arc<Mutex<Vec<String>>>,
    pub(super) sftp_ops: Arc<Mutex<Vec<String>>>,
    pub(super) opened_sessions: Arc<Mutex<u32>>,
    sftp_channel: Arc<Mutex<Option<russh::ChannelId>>>,
    sftp_buffer: Arc<Mutex<Vec<u8>>>,
}

impl Server {
    pub(super) fn new(commands: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            commands,
            sftp_ops: Arc::new(Mutex::new(Vec::new())),
            opened_sessions: Arc::new(Mutex::new(0)),
            sftp_channel: Arc::new(Mutex::new(None)),
            sftp_buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl russh::server::Server for Server {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
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
        *self.opened_sessions.lock().unwrap() += 1;
        Ok(true)
    }
    async fn exec_request(
        &mut self,
        channel: russh::ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.commands
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(data).into_owned());
        session.channel_success(channel)?;
        if data == b"hold" {
            // A deterministic running channel for cancellation regression tests.
            return Ok(());
        }
        session.data(channel, b"flow fixture completed\n".to_vec())?;
        session.data(channel, data.to_vec())?;
        session.exit_status_request(channel, if data.starts_with(b"fail ") { 1 } else { 0 })?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: russh::ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let _ = (term, col_width, row_height, pix_width, pix_height, modes);
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: russh::ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: russh::ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            *self.sftp_channel.lock().unwrap() = Some(channel);
            self.sftp_buffer.lock().unwrap().clear();
        }
        session.channel_success(channel)?;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: russh::ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.sftp_channel.lock().unwrap().as_ref() != Some(&channel) {
            return Ok(());
        }
        let mut buffer = self.sftp_buffer.lock().unwrap();
        buffer.extend_from_slice(data);
        let replies = sftp_replies(&mut buffer, &mut self.sftp_ops.lock().unwrap());
        drop(buffer);
        for reply in replies {
            session.data(channel, reply)?;
        }
        Ok(())
    }
}

fn sftp_replies(buffer: &mut Vec<u8>, ops: &mut Vec<String>) -> Vec<Vec<u8>> {
    let mut replies = Vec::new();
    while buffer.len() >= 4 {
        let length = u32::from_be_bytes(buffer[..4].try_into().unwrap()) as usize;
        if length == 0 || buffer.len() < 4 + length {
            break;
        }
        let packet = buffer[4..4 + length].to_vec();
        buffer.drain(..4 + length);
        if let Some(payload) = sftp_reply(&packet, ops) {
            let mut framed = (payload.len() as u32).to_be_bytes().to_vec();
            framed.extend(payload);
            replies.push(framed);
        }
    }
    replies
}

fn sftp_reply(packet: &[u8], ops: &mut Vec<String>) -> Option<Vec<u8>> {
    let kind = *packet.first()?;
    if kind == 1 {
        ops.push("init".into());
        let mut payload = vec![2];
        payload.extend_from_slice(&3u32.to_be_bytes());
        return Some(payload);
    }
    let id = u32::from_be_bytes(packet.get(1..5)?.try_into().ok()?);
    ops.push(
        match kind {
            3 => "open",
            5 => "read",
            6 => "write",
            13 => "remove",
            14 => "mkdir",
            15 => "rmdir",
            16 => "realpath",
            18 => "rename",
            _ => "other",
        }
        .into(),
    );
    Some(match kind {
        3 => sftp_handle(id),
        5 => sftp_status(id, 1),
        16 => sftp_name(id, "/"),
        _ => sftp_status(id, 0),
    })
}

fn sftp_status(id: u32, code: u32) -> Vec<u8> {
    let mut payload = vec![101];
    payload.extend_from_slice(&id.to_be_bytes());
    payload.extend_from_slice(&code.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload
}

fn sftp_handle(id: u32) -> Vec<u8> {
    let mut payload = vec![102];
    payload.extend_from_slice(&id.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.push(b'h');
    payload
}

fn sftp_name(id: u32, path: &str) -> Vec<u8> {
    let mut payload = vec![104];
    payload.extend_from_slice(&id.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    for value in [path, path] {
        payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
        payload.extend_from_slice(value.as_bytes());
    }
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload
}

#[tokio::test]
async fn flow_real_api_condition_native_ssh_database() {
    let mut bus = test_bus().await;
    let logs = std::env::temp_dir().join(format!("unfour-flow-{}", unfour_core::id::new_id()));
    bus.ssh = bus.ssh.with_task_log_dir(logs);
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    // All endpoints and data are disposable; no user SSH server is contacted.
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let commands = Arc::new(Mutex::new(vec![]));
    let mut server = Server::new(commands.clone());
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
    let connection = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "flow fixture".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            username: "fixture".into(),
            auth_kind: "password".into(),
            key_path: None,
            credential_ref: None,
            secret: Some("disposable-fixture-password".into()),
        })
        .await
        .unwrap();
    let ssh = bus
        .save_ssh_task(SshTaskSaveInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "flow task".into(),
            description: String::new(),
            default_connection_id: None,
            steps: vec![SshTaskStepInput {
                id: None,
                name: "command".into(),
                step_type: "command".into(),
                position: 0,
                enabled: true,
                config_version: Some(1),
                config_json: json!({"command":"echo {{value}}", "timeoutSeconds":5}),
            }],
        })
        .await
        .unwrap();
    let (url, received) = spawn_api_test_server();
    let api = bus
        .save_api_request(api_script_test_input(workspace.clone(), url))
        .await
        .unwrap();
    let database = bus
        .save_database_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "flow database".into(),
            driver: "sqlite".into(),
            host: None,
            port: None,
            database: None,
            username: None,
            ssl_mode: None,
            sqlite_path: Some(":memory:".into()),
            credential_ref: None,
            read_only: true,
        })
        .await
        .unwrap();
    let flow = bus.save_flow(definition(&workspace, json!([
        action("api", "api", &api.id, json!({})),
        {"id":"condition","name":"condition","kind":"condition","timeoutMs":2000,"predicate":{"left":{"$ref":"/steps/api/status"},"op":"eq","right":200},"ifTrue":"ssh","ifFalse":"$end"},
        {"id":"ssh","name":"ssh","kind":"action","timeoutMs":10000,"action":{"capability":"ssh","resourceId":ssh.task.id,"connectionId":connection.id,"arguments":{"inputs":{"value":{"$ref":"/inputs/value"}}}}},
        action("db", "database", &database.id, json!({"sql":"SELECT '${/steps/ssh/status}' AS ssh_status"}))
    ]))).await.unwrap();
    let run = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    let result = finished(&bus, &run).await;
    assert_eq!(result.status, FlowRunStatus::Succeeded, "{:?}", result);
    assert_eq!(commands.lock().unwrap().as_slice(), ["echo 42"]);
    assert!(received.recv_timeout(Duration::from_secs(1)).is_ok());
    assert_eq!(result.steps[3].status, FlowStepRunStatus::Succeeded);
    assert!(result.steps[2].attempts[0].output.as_ref().unwrap()["log"]
        .as_str()
        .unwrap()
        .contains("flow fixture completed"));
    task.abort();
}

#[tokio::test]
async fn flow_native_ssh_secret_inputs_and_failed_history() {
    let mut bus = test_bus().await;
    let logs = std::env::temp_dir().join(format!("unfour-flow-{}", unfour_core::id::new_id()));
    bus.ssh = bus.ssh.with_task_log_dir(logs);
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    // All endpoints and data are disposable; no user SSH server is contacted.
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let commands = Arc::new(Mutex::new(vec![]));
    let mut server = Server::new(commands.clone());
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
    let connection = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "flow fixture".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            username: "fixture".into(),
            auth_kind: "password".into(),
            key_path: None,
            credential_ref: None,
            secret: Some("disposable-fixture-password".into()),
        })
        .await
        .unwrap();
    let ssh = bus
        .save_ssh_task(SshTaskSaveInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "flow task".into(),
            description: String::new(),
            default_connection_id: None,
            steps: vec![SshTaskStepInput {
                id: None,
                name: "command".into(),
                step_type: "command".into(),
                position: 0,
                enabled: true,
                config_version: Some(1),
                config_json: json!({"command":"{{MODE}} {{VERSION}} {{DEPLOY}} {{CONFIG}} {{WORKSPACE}} {{UPSTREAM}} {{PUBLIC}}", "timeoutSeconds":5}),
            }],
        })
        .await
        .unwrap();

    let variable = |key: &str, value: &str, secret| {
        serde_json::from_value::<WorkspaceVariableInput>(
            json!({"key":key,"value":value,"isSecret":secret}),
        )
        .unwrap()
    };
    bus.workspace_variables_replace(
        workspace.clone(),
        vec![
            variable("VERSION", "v1.2.3", false),
            variable("CONFIG", "workspace-old", false),
            variable("WORKSPACE", "workspace-private", true),
        ],
    )
    .await
    .unwrap();
    let env = bus
        .workspace_environment_create(workspace.clone(), "selected".into())
        .await
        .unwrap();
    bus.workspace_environment_update(
        workspace.clone(),
        env.id.clone(),
        env.name,
        vec![variable("CONFIG", "environment-private", true)],
    )
    .await
    .unwrap();
    for mode in ["echo", "fail"] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let http = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 8192];
            socket.read(&mut bytes).unwrap();
            let body = r#"{"token":"upstream-private","version":"upstream-visible"}"#;
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let api = bus
            .save_api_request(api_script_test_input(workspace.clone(), url))
            .await
            .unwrap();
        let mut definition = definition(
            &workspace,
            json!([
                {"id":"ssh","name":"ssh","kind":"action","timeoutMs":10000,"action":{"capability":"ssh","resourceId":ssh.task.id,"connectionId":connection.id,"arguments":{"workspaceDefaults":true,"inputs":{"MODE":mode,"DEPLOY":{"$ref":"/inputs/value"},"UPSTREAM":"${/steps/login/body/token}","PUBLIC":{"$ref":"/steps/login/body/version"}}}}},
                action("later","ssh",&ssh.task.id,json!({}))
            ]),
        );
        // A wait makes the fail-fast assertion independent of another SSH fixture.
        definition.steps[1] = serde_json::from_value(
            json!({"id":"later","name":"later","kind":"wait","timeoutMs":1000,"durationMs":1}),
        )
        .unwrap();
        definition.inputs =
            serde_json::from_value(json!([{"name":"value","type":"string","secret":true}]))
                .unwrap();
        definition.steps.insert(
            0,
            serde_json::from_value(action("login", "api", &api.id, json!({}))).unwrap(),
        );
        let flow = bus.save_flow(definition).await.unwrap();
        let mut input = request(&workspace, &flow.id);
        input.inputs = json!({"value":"flow-private"});
        input.environment_id = Some(env.id.clone());
        let run = bus.run_flow(input).await.unwrap();
        let result = finished(&bus, &run).await;
        assert_eq!(
            result.status,
            if mode == "fail" {
                FlowRunStatus::Failed
            } else {
                FlowRunStatus::Succeeded
            },
            "{result:?}"
        );
        let output = result.steps[1].output.as_ref().unwrap();
        let log = output["log"].as_str().unwrap();
        assert!(log.contains("v1.2.3"), "{log}");
        for secret in [
            "flow-private",
            "environment-private",
            "workspace-private",
            "upstream-private",
        ] {
            assert!(!log.contains(secret), "{log}");
        }
        let persisted_log = bus
            .read_ssh_task_run_log(workspace.clone(), output["runId"].as_str().unwrap().into())
            .await
            .unwrap();
        assert!(persisted_log.contains("v1.2.3"));
        assert!(persisted_log.contains("upstream-visible"));
        assert!(log.contains("upstream-visible"));
        http.join().unwrap();
        for secret in [
            "flow-private",
            "environment-private",
            "workspace-private",
            "upstream-private",
        ] {
            assert!(!persisted_log.contains(secret), "{persisted_log}");
        }
        if mode == "fail" {
            assert_eq!(result.error.as_deref(), Some("FLOW_SSH_FAILED"));
            assert!(output["errorMessage"].is_string());
            assert!(output["logTruncated"].is_boolean());
            assert_eq!(result.steps[2].status, FlowStepRunStatus::Skipped);
            assert_eq!(result.steps[1].attempts[0].output.as_ref(), Some(output));
        }
    }
    task.abort();
}
