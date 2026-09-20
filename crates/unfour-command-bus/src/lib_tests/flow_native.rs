use super::*;
use russh::server::{Msg, Server as _, Session};

#[derive(Clone)]
struct Server {
    commands: Arc<Mutex<Vec<String>>>,
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
        session.data(channel, b"flow fixture completed\n".to_vec())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
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
    let mut server = Server {
        commands: commands.clone(),
    };
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
