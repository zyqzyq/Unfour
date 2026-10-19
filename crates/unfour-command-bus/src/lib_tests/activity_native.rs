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

#[tokio::test]
async fn ssh_command_and_sftp_outcomes_survive_activity_failure() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let commands = Arc::new(Mutex::new(vec![]));
    let mut server = super::native::Server::new(commands.clone());
    let sftp_ops = server.sftp_ops.clone();
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
            name: "activity".into(),
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
    let diagnostic = |command: &str| SshDiagnosticInput {
        workspace_id: workspace.clone(),
        connection_id: connection.id.clone(),
        command: command.into(),
        timeout_ms: Some(5_000),
    };
    assert!(bus
        .run_ssh_diagnostic(diagnostic("rm -rf /"))
        .await
        .is_err());
    assert!(commands.lock().unwrap().is_empty());
    sqlx::query("CREATE TRIGGER fail_completion_activity BEFORE INSERT ON activity_events WHEN NEW.action IN ('ssh.diagnostic','ssh.command','ssh.sftp.mkdir','ssh.sftp.rename','ssh.sftp.delete','ssh.sftp.transfer.start') BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;")
        .execute(bus.db.pool()).await.unwrap();
    assert!(bus
        .run_ssh_command(SshDiagnosticInput {
            workspace_id: workspace.clone(),
            connection_id: "missing-connection".into(),
            command: "echo should-not-run".into(),
            timeout_ms: Some(5_000),
        })
        .await
        .is_err());
    let diagnosed = bus
        .run_ssh_diagnostic(diagnostic("uptime"))
        .await
        .expect("diagnostic result must survive activity failure");
    assert_eq!(diagnosed.exit_status, Some(0));
    assert!(diagnosed.stdout.contains("flow fixture completed"));
    assert_eq!(diagnosed.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert!(!diagnosed.stdout.contains("audit sink"));
    let mut serialized = serde_json::to_value(&diagnosed).unwrap();
    assert_eq!(serialized["warnings"], json!(["ACTIVITY_RECORD_FAILED"]));
    assert!(!serialized.to_string().contains("audit sink unavailable"));
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: SshDiagnosticResult = serde_json::from_value(serialized).unwrap();
    assert!(legacy.warnings.is_empty());
    let command = "echo marker-9f3c password=activity-secret-value";
    let executed = bus
        .run_ssh_command(diagnostic(command))
        .await
        .expect("command result must survive activity failure");
    assert_eq!(executed.exit_status, Some(0));
    assert!(executed.stdout.contains("flow fixture completed"));
    assert_eq!(executed.stderr, "");
    assert_eq!(executed.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert!(!executed.warnings.iter().any(|warning| {
        warning.contains("activity-secret-value") || warning.contains("audit sink")
    }));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        commands.lock().unwrap().as_slice(),
        ["uptime", command],
        "activity failure must not replay the remote command"
    );
    let missing = bus
        .create_ssh_sftp_directory(SftpPathInput {
            workspace_id: workspace.clone(),
            session_id: "missing-session".into(),
            path: "/activity-dir".into(),
        })
        .await;
    assert!(missing.is_err(), "a failed mutation must stay an error");
    assert!(sftp_ops.lock().unwrap().is_empty());
    let session = tokio::time::timeout(
        Duration::from_secs(8),
        bus.connect_ssh_session(SshConnectInput {
            workspace_id: workspace.clone(),
            connection_id: connection.id.clone(),
            cols: Some(80),
            rows: Some(24),
            secret: None,
        }),
    )
    .await
    .expect("connect timed out")
    .expect("connect failed");
    tokio::time::timeout(
        Duration::from_secs(8),
        bus.open_ssh_sftp(SftpSessionInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
        }),
    )
    .await
    .expect("sftp open timed out")
    .expect("sftp open failed");
    let mkdir = bus
        .create_ssh_sftp_directory(SftpPathInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
            path: "/activity-dir".into(),
        })
        .await
        .unwrap();
    assert_eq!(mkdir.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert!(!serde_json::to_string(&mkdir)
        .unwrap()
        .contains("activity-dir"));
    let renamed = bus
        .rename_ssh_sftp_path(SftpRenameInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
            old_path: "/activity-dir".into(),
            new_path: "/activity-renamed".into(),
        })
        .await
        .unwrap();
    assert_eq!(renamed.warnings, ["ACTIVITY_RECORD_FAILED"]);
    let deleted = bus
        .delete_ssh_sftp_path(SftpDeleteInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
            path: "/activity-renamed".into(),
            is_directory: true,
        })
        .await
        .unwrap();
    assert_eq!(deleted.warnings, ["ACTIVITY_RECORD_FAILED"]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let ops = sftp_ops.lock().unwrap().clone();
    assert_eq!(ops.iter().filter(|op| op.as_str() == "mkdir").count(), 1);
    assert_eq!(ops.iter().filter(|op| op.as_str() == "rename").count(), 1);
    assert_eq!(ops.iter().filter(|op| op.as_str() == "rmdir").count(), 1);
    let download_path = std::env::temp_dir().join(format!(
        "unfour-activity-download-{}",
        unfour_core::id::new_id()
    ));
    let upload_path = std::env::temp_dir().join(format!(
        "unfour-activity-upload-{}",
        unfour_core::id::new_id()
    ));
    std::fs::write(&upload_path, b"upload-body").unwrap();
    let download = bus
        .download_ssh_sftp_file(SftpTransferInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
            local_path: download_path.display().to_string(),
            remote_path: "/activity-file".into(),
            overwrite: true,
        })
        .await
        .unwrap();
    assert!(!download.transfer_id.is_empty());
    assert_eq!(download.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert!(!download
        .warnings
        .iter()
        .any(|warning| warning.contains("audit sink") || warning.contains(&download.local_path)));
    let listed = bus
        .list_ssh_sftp_transfers(SftpSessionInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
        })
        .await
        .unwrap();
    assert!(listed
        .iter()
        .any(|transfer| transfer.transfer_id == download.transfer_id));
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let states = bus
                .list_ssh_sftp_transfers(SftpSessionInput {
                    workspace_id: workspace.clone(),
                    session_id: session.session_id.clone(),
                })
                .await
                .unwrap();
            let state = states
                .iter()
                .find(|transfer| transfer.transfer_id == download.transfer_id)
                .expect("download handle disappeared");
            if state.status != "pending" && state.status != "running" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("download did not leave the running state");
    let upload = bus
        .upload_ssh_sftp_file(SftpTransferInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
            local_path: upload_path.display().to_string(),
            remote_path: "/activity-upload".into(),
            overwrite: true,
        })
        .await
        .unwrap();
    assert_ne!(upload.transfer_id, download.transfer_id);
    assert_eq!(upload.warnings, ["ACTIVITY_RECORD_FAILED"]);
    let recorded: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM activity_events WHERE action IN ('ssh.diagnostic','ssh.command','ssh.sftp.mkdir','ssh.sftp.rename','ssh.sftp.delete','ssh.sftp.transfer.start')")
        .fetch_one(bus.db.pool()).await.unwrap();
    assert_eq!(recorded, 0);
    let _ = bus
        .close_ssh_session(SshCloseInput {
            workspace_id: workspace,
            session_id: session.session_id,
        })
        .await;
    server_task.abort();
    let _ = std::fs::remove_file(download_path);
    let _ = std::fs::remove_file(upload_path);
}

#[tokio::test]
async fn native_ssh_session_keeps_its_id_when_activity_logging_fails() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let commands = Arc::new(Mutex::new(vec![]));
    let mut server = super::native::Server::new(commands);
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
            name: "session-activity".into(),
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
    let input = SshConnectInput {
        workspace_id: workspace.clone(),
        connection_id: connection.id.clone(),
        cols: Some(80),
        rows: Some(24),
        secret: None,
    };
    sqlx::query("CREATE TRIGGER fail_native_history BEFORE INSERT ON ssh_terminal_history BEGIN SELECT RAISE(FAIL, 'history unavailable'); END;")
        .execute(bus.db.pool())
        .await
        .unwrap();
    let history_failure = tokio::time::timeout(
        Duration::from_secs(8),
        bus.connect_ssh_session(input.clone()),
    )
    .await
    .expect("history-failure connect timed out")
    .expect_err("native history persistence failure must stay an error");
    assert!(
        history_failure.to_string().contains("history unavailable"),
        "engine history error was rewritten: {history_failure}"
    );
    let listed = bus.list_ssh_sessions(workspace.clone()).await.unwrap();
    assert!(
        listed.iter().all(|session| {
            !matches!(
                session.status.as_str(),
                "connected" | "degraded" | "reconnecting"
            )
        }),
        "history failure left a live session: {listed:?}"
    );
    sqlx::query("DROP TRIGGER fail_native_history")
        .execute(bus.db.pool())
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_native_session_activity BEFORE INSERT ON activity_events WHEN NEW.action IN ('ssh.session.connect','ssh.session.close') BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;")
        .execute(bus.db.pool())
        .await
        .unwrap();
    let session = tokio::time::timeout(Duration::from_secs(8), bus.connect_ssh_session(input))
        .await
        .expect("connect timed out")
        .expect("native session id must survive activity failure");
    assert!(!session.session_id.is_empty());
    assert_eq!(session.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert!(!serde_json::to_string(&session)
        .unwrap()
        .contains("audit sink"));
    let closed = bus
        .close_ssh_session(SshCloseInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
        })
        .await
        .expect("closed native session must survive activity failure");
    assert_eq!(closed.session_id, session.session_id);
    assert_eq!(closed.status, "disconnected");
    assert_eq!(closed.warnings, ["ACTIVITY_RECORD_FAILED"]);
    let listed = bus.list_ssh_sessions(workspace).await.unwrap();
    assert_eq!(
        listed
            .iter()
            .filter(|item| item.session_id == session.session_id && item.status == "disconnected")
            .count(),
        1
    );
    let recorded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action IN ('ssh.session.connect','ssh.session.close')",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(recorded, 0);
    server_task.abort();
}

#[tokio::test]
async fn native_history_persistence_failure_releases_the_session() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let mut server = super::native::Server::new(Arc::new(Mutex::new(vec![])));
    let opened_sessions = server.opened_sessions.clone();
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
            name: "history-cleanup".into(),
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
    let input = SshConnectInput {
        workspace_id: workspace.clone(),
        connection_id: connection.id,
        cols: Some(80),
        rows: Some(24),
        secret: None,
    };
    let opened = || *opened_sessions.lock().unwrap();

    sqlx::query("CREATE TRIGGER fail_save_session BEFORE INSERT ON ssh_terminal_history BEGIN SELECT RAISE(FAIL, 'save session unavailable'); END;")
        .execute(bus.db.pool())
        .await
        .unwrap();
    let save_failure = tokio::time::timeout(
        Duration::from_secs(8),
        bus.connect_ssh_session(input.clone()),
    )
    .await
    .expect("save-failure connect timed out")
    .expect_err("save_session failure must stay an error");
    assert!(
        save_failure
            .to_string()
            .contains("save session unavailable"),
        "original save error was rewritten: {save_failure}"
    );
    assert!(!save_failure.to_string().contains("ACTIVITY_RECORD_FAILED"));
    assert_no_live_ssh_session(&bus, &workspace, "save_session").await;
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(opened(), 1, "save_session failure reconnected");
    sqlx::query("DROP TRIGGER fail_save_session")
        .execute(bus.db.pool())
        .await
        .unwrap();

    sqlx::query("CREATE TRIGGER fail_append_output BEFORE UPDATE OF content ON ssh_terminal_history BEGIN SELECT RAISE(FAIL, 'append output unavailable'); END;")
        .execute(bus.db.pool())
        .await
        .unwrap();
    let append_failure = tokio::time::timeout(
        Duration::from_secs(8),
        bus.connect_ssh_session(input.clone()),
    )
    .await
    .expect("append-failure connect timed out")
    .expect_err("append_output failure must stay an error");
    assert!(
        append_failure
            .to_string()
            .contains("append output unavailable"),
        "original append error was rewritten: {append_failure}"
    );
    assert!(!append_failure
        .to_string()
        .contains("ACTIVITY_RECORD_FAILED"));
    assert_no_live_ssh_session(&bus, &workspace, "append_output").await;
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(opened(), 2, "append_output failure reconnected");
    sqlx::query("DROP TRIGGER fail_append_output")
        .execute(bus.db.pool())
        .await
        .unwrap();

    let session = tokio::time::timeout(
        Duration::from_secs(8),
        bus.connect_ssh_session(input.clone()),
    )
    .await
    .expect("connect timed out")
    .expect("connect after history recovery");
    assert_eq!(opened(), 3);
    sqlx::query("CREATE TRIGGER fail_close_history BEFORE UPDATE ON ssh_terminal_history BEGIN SELECT RAISE(FAIL, 'close history unavailable'); END;")
        .execute(bus.db.pool())
        .await
        .unwrap();
    let close_failure = bus
        .close_ssh_session(SshCloseInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id,
        })
        .await
        .expect_err("close history failure must stay an error");
    assert!(
        close_failure
            .to_string()
            .contains("close history unavailable"),
        "original close error was rewritten: {close_failure}"
    );
    assert!(!close_failure.to_string().contains("ACTIVITY_RECORD_FAILED"));
    assert_no_live_ssh_session(&bus, &workspace, "close_session").await;
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(opened(), 3, "close history failure reconnected");
    server_task.abort();
}

async fn assert_no_live_ssh_session(bus: &crate::CommandBus, workspace_id: &str, label: &str) {
    let listed = bus
        .list_ssh_sessions(workspace_id.to_string())
        .await
        .unwrap();
    assert!(
        listed.iter().all(|session| {
            !matches!(
                session.status.as_str(),
                "connected" | "degraded" | "reconnecting"
            )
        }),
        "{label} left a live session: {listed:?}"
    );
}
