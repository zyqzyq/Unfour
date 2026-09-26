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
        session.data(channel, data.to_vec())?;
        session.exit_status_request(channel, if data.starts_with(b"fail ") { 1 } else { 0 })?;
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
                config_json: json!({"command":"{{MODE}} {{VERSION}} {{DEPLOY}} {{CONFIG}} {{WORKSPACE}}", "timeoutSeconds":5}),
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
        let mut definition = definition(
            &workspace,
            json!([
                {"id":"ssh","name":"ssh","kind":"action","timeoutMs":10000,"action":{"capability":"ssh","resourceId":ssh.task.id,"connectionId":connection.id,"arguments":{"workspaceDefaults":true,"inputs":{"MODE":mode,"DEPLOY":{"$ref":"/inputs/value"}}}}},
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
        let output = result.steps[0].output.as_ref().unwrap();
        let log = output["log"].as_str().unwrap();
        assert!(log.contains("v1.2.3"), "{log}");
        for secret in ["flow-private", "environment-private", "workspace-private"] {
            assert!(!log.contains(secret), "{log}");
        }
        let persisted_log = bus
            .read_ssh_task_run_log(workspace.clone(), output["runId"].as_str().unwrap().into())
            .await
            .unwrap();
        assert!(persisted_log.contains("v1.2.3"));
        for secret in ["flow-private", "environment-private", "workspace-private"] {
            assert!(!persisted_log.contains(secret), "{persisted_log}");
        }
        if mode == "fail" {
            assert_eq!(result.error.as_deref(), Some("FLOW_SSH_FAILED"));
            assert!(output["errorMessage"].is_string());
            assert!(output["logTruncated"].is_boolean());
            assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
            assert_eq!(result.steps[0].attempts[0].output.as_ref(), Some(output));
        }
    }
    task.abort();
}
