use super::*;
use russh::server::Server as _;

fn version(detail: &SshTaskDetail) -> String {
    std::iter::once(detail.task.updated_at.clone())
        .chain(
            detail
                .steps
                .iter()
                .map(|step| format!("{}:{}", step.id, step.updated_at)),
        )
        .collect::<Vec<_>>()
        .join("|")
}

async fn done(bus: &CommandBus, run: &SshTaskRun) -> SshTaskRun {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let runs = bus
                .list_ssh_task_runs(run.workspace_id.clone(), run.task_id.clone())
                .await
                .unwrap();
            let current = runs.into_iter().find(|r| r.id == run.id).unwrap();
            if current.status != "running" {
                break current;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn ssh_rerun_executes_current_definition_from_start_and_keeps_local_context() {
    let mut bus = test_bus().await;
    let logs = std::env::temp_dir().join(format!("unfour-rerun-{}", unfour_core::id::new_id()));
    bus.ssh = bus.ssh.with_task_log_dir(logs.clone());
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let commands = Arc::new(Mutex::new(vec![]));
    let mut server = super::native::Server::new(commands.clone());
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
    let mut task_input = SshTaskSaveInput {
        id: None,
        workspace_id: workspace.clone(),
        name: "Rerun".into(),
        description: String::new(),
        default_connection_id: None,
        steps: [
            "echo disabled",
            "echo first",
            "{{MODE}} {{CUSTOM}}",
            "echo {{LABEL}}",
        ]
        .into_iter()
        .enumerate()
        .map(|(position, command)| SshTaskStepInput {
            id: None,
            name: format!("step {position}"),
            step_type: "command".into(),
            position: position as i64,
            enabled: position != 0,
            config_version: Some(1),
            config_json: json!({"command":command,"timeoutSeconds":5}),
        })
        .collect(),
    };
    let detail = bus.save_ssh_task(task_input.clone()).await.unwrap();
    task_input.id = Some(detail.task.id.clone());
    let mut input = SshTaskRunInput {
        workspace_id: workspace.clone(),
        task_id: detail.task.id.clone(),
        connection_id: Some(connection.id),
        inputs: [
            ("MODE", "fail"),
            ("CUSTOM", "private-fixture-value"),
            ("LABEL", "recall-only-marker"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
        secret_input_names: vec!["CUSTOM".into()],
        context: Some(SshTaskRunContextInput {
            environment_id: None,
            task_version: version(&detail),
            environment_updated_at: None,
            input_sources: ["MODE", "CUSTOM", "LABEL"]
                .into_iter()
                .map(|key| (key.into(), SshTaskInputSource::Manual))
                .collect(),
        }),
    };
    let first = bus.run_ssh_task(input.clone()).await.unwrap();
    let first = done(&bus, &first).await;
    assert_eq!(first.status, "failed");
    let original_log = std::fs::read_to_string(&first.log_path).unwrap();
    assert!(!original_log.contains("private-fixture-value"));
    assert_eq!(
        *commands.lock().unwrap(),
        ["echo first", "fail private-fixture-value"]
    );
    // Change the current definition. Old preflight metadata cannot execute it silently.
    task_input.steps[1].config_json["command"] = json!("echo current-first");
    let updated = bus.save_ssh_task(task_input.clone()).await.unwrap();
    assert!(bus
        .run_ssh_task(input.clone())
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_DEFINITION_CHANGED"));
    input.context.as_mut().unwrap().task_version = version(&updated);
    input.inputs.insert("MODE".into(), "echo".into());
    let second = bus.run_ssh_task(input.clone()).await.unwrap();
    let second = done(&bus, &second).await;
    assert_eq!(second.status, "success");
    assert_ne!(first.id, second.id);
    assert_ne!(first.log_path, second.log_path);
    assert_eq!(
        std::fs::read_to_string(&first.log_path).unwrap(),
        original_log
    );
    assert_eq!(
        &commands.lock().unwrap()[2..],
        [
            "echo current-first",
            "echo private-fixture-value",
            "echo recall-only-marker"
        ]
    );
    let snapshot = second.context.as_ref().unwrap();
    assert!(snapshot.inputs["CUSTOM"].value.is_none());
    assert_eq!(
        snapshot.inputs["LABEL"].value.as_deref(),
        Some("recall-only-marker")
    );
    let exported = bus
        .workspace_bundle_export(workspace.clone())
        .await
        .unwrap();
    assert!(!exported.contains("recall-only-marker"));
    assert!(!exported.contains("contextJson"));
    let key = unfour_core::domain::DomainEntityKey::new(
        unfour_core::domain::DomainEntityType::SshTask,
        &workspace,
        &detail.task.id,
    );
    let domain = bus.ssh.read_task_domain_snapshot(&key).await.unwrap();
    assert!(!serde_json::to_string(&domain)
        .unwrap()
        .contains("recall-only-marker"));
    // Cancellation retains its own run. A later run starts from the beginning again.
    task_input.steps[2].config_json["command"] = json!("hold");
    let held = bus.save_ssh_task(task_input.clone()).await.unwrap();
    input.context.as_mut().unwrap().task_version = version(&held);
    let cancelled = bus.run_ssh_task(input.clone()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while commands
            .lock()
            .unwrap()
            .last()
            .is_none_or(|command| command != "hold")
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    bus.cancel_ssh_task_run(SshTaskCancelInput {
        workspace_id: workspace.clone(),
        run_id: cancelled.id.clone(),
    })
    .await
    .unwrap();
    assert_eq!(done(&bus, &cancelled).await.status, "cancelled");
    task_input.steps[2].config_json["command"] = json!("echo after-cancel");
    let after_cancel = bus.save_ssh_task(task_input).await.unwrap();
    input.context.as_mut().unwrap().task_version = version(&after_cancel);
    let final_run = bus.run_ssh_task(input).await.unwrap();
    assert_eq!(done(&bus, &final_run).await.status, "success");
    assert_eq!(
        bus.list_ssh_task_runs(workspace, detail.task.id)
            .await
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        std::fs::read_to_string(&first.log_path).unwrap(),
        original_log
    );
    server_task.abort();
    std::fs::remove_dir_all(logs).unwrap();
}
