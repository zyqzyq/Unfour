use super::*;
use russh::server::Server as _;

#[tokio::test]
async fn ssh_run_handle_cancellation_and_flow_survive_activity_failure() {
    let mut bus = test_bus().await;
    let logs =
        std::env::temp_dir().join(format!("unfour-activity-ssh-{}", unfour_core::id::new_id()));
    bus.ssh = bus.ssh.with_task_log_dir(logs.clone());
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let commands = Arc::new(Mutex::new(vec![]));
    let mut server = super::native::Server {
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
    let server_task = tokio::spawn(async move {
        server.run_on_socket(config, &socket).await.unwrap();
    });
    let connection = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "isolated".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            username: "fixture".into(),
            auth_kind: "password".into(),
            key_path: None,
            credential_ref: None,
            secret: Some("fixture-password".into()),
        })
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for command in ["hold", "echo once"] {
        tasks.push(
            bus.save_ssh_task(SshTaskSaveInput {
                id: None,
                workspace_id: workspace.clone(),
                name: command.into(),
                description: String::new(),
                default_connection_id: None,
                steps: vec![SshTaskStepInput {
                    id: None,
                    name: "command".into(),
                    step_type: "command".into(),
                    position: 0,
                    enabled: true,
                    config_version: Some(1),
                    config_json: json!({"command":command,"timeoutSeconds":10}),
                }],
            })
            .await
            .unwrap(),
        );
    }
    let flow = bus.save_flow(definition(&workspace, json!([
        {"id":"ssh","name":"ssh","kind":"action","timeoutMs":10000,"action":{
            "capability":"ssh","resourceId":tasks[1].task.id,"connectionId":connection.id,"arguments":{"inputs":{}}}}
    ]))).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_ssh_activity BEFORE INSERT ON activity_events WHEN NEW.action IN ('ssh.task.run','ssh.task.cancel') BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;")
        .execute(bus.db.pool()).await.unwrap();
    let run = bus
        .run_ssh_task(SshTaskRunInput {
            workspace_id: workspace.clone(),
            task_id: tasks[0].task.id.clone(),
            connection_id: Some(connection.id),
            inputs: Default::default(),
            secret_input_names: vec![],
        })
        .await
        .unwrap();
    assert_eq!(run.warnings, ["ACTIVITY_RECORD_FAILED"]);
    let mut serialized = serde_json::to_value(&run).unwrap();
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: SshTaskRun = serde_json::from_value(serialized).unwrap();
    assert!(legacy.warnings.is_empty());
    tokio::time::timeout(Duration::from_secs(10), async {
        while commands.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let cancelled = bus
        .cancel_ssh_task_run(SshTaskCancelInput {
            workspace_id: workspace.clone(),
            run_id: run.id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(cancelled.id, run.id);
    assert_eq!(cancelled.warnings, ["ACTIVITY_RECORD_FAILED"]);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let runs = bus
                .list_ssh_task_runs(workspace.clone(), tasks[0].task.id.clone())
                .await
                .unwrap();
            assert_eq!(runs.len(), 1);
            if runs[0].status != "running" {
                assert_eq!(runs[0].status, "cancelled");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let flow_run = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    let result = finished(&bus, &flow_run).await;
    assert_eq!(result.status, FlowRunStatus::Succeeded, "{result:?}");
    assert_eq!(result.steps[0].attempts.len(), 1);
    assert_eq!(
        result.steps[0].attempts[0].output.as_ref().unwrap()["warnings"],
        json!(["ACTIVITY_RECORD_FAILED"])
    );
    assert_eq!(*commands.lock().unwrap(), ["hold", "echo once"]);
    server_task.abort();
    std::fs::remove_dir_all(logs).unwrap();
}
