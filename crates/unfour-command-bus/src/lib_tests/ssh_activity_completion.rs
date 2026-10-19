use super::test_bus;
use serde_json::json;
#[cfg(not(feature = "ssh-native"))]
use unfour_core::models::{SshCloseInput, SshConnectInput, SshConnectionInput, SshSessionSummary};
use unfour_core::models::{
    SshHostKeyInput, SshHostKeyResetResult, SshKnownHostsImportInput, SshKnownHostsImportResult,
    SshTaskCleanupInput, SshTaskCleanupResult, SshTaskSaveInput, SshTaskStepInput,
};

const KNOWN_HOST: &str = "example.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQC7";

#[cfg(not(feature = "ssh-native"))]
fn connection(workspace_id: String) -> SshConnectionInput {
    SshConnectionInput {
        id: None,
        workspace_id,
        name: "activity".into(),
        host: "127.0.0.1".into(),
        port: Some(22),
        username: "fixture".into(),
        auth_kind: "none".into(),
        key_path: None,
        credential_ref: None,
        secret: None,
    }
}

#[cfg(not(feature = "ssh-native"))]
fn connect_input(workspace_id: String, connection_id: String) -> SshConnectInput {
    SshConnectInput {
        workspace_id,
        connection_id,
        cols: Some(80),
        rows: Some(24),
        secret: None,
    }
}

async fn host_key_count(bus: &crate::CommandBus, workspace_id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM ssh_host_keys WHERE workspace_id = ?1")
        .bind(workspace_id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap()
}

async fn task_run_count(bus: &crate::CommandBus, workspace_id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1")
        .bind(workspace_id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn completed_ssh_trust_and_cleanup_survive_activity_failure() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let task = bus
        .save_ssh_task(SshTaskSaveInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "cleanup".into(),
            description: String::new(),
            default_connection_id: None,
            steps: vec![SshTaskStepInput {
                id: None,
                name: "command".into(),
                step_type: "command".into(),
                position: 0,
                enabled: true,
                config_version: Some(1),
                config_json: json!({"command":"echo kept","timeoutSeconds":10}),
            }],
        })
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ssh_task_run (id, workspace_id, task_id, connection_id, status, started_at, finished_at, error_message, log_path) VALUES ('run-kept', ?1, ?2, NULL, 'success', '2026-10-19T00:00:00Z', '2026-10-19T00:00:01Z', NULL, 'missing.log')",
    )
    .bind(&workspace)
    .bind(&task.task.id)
    .execute(bus.db.pool())
    .await
    .unwrap();
    let imported = bus
        .import_ssh_known_hosts(SshKnownHostsImportInput {
            workspace_id: workspace.clone(),
            content: KNOWN_HOST.into(),
        })
        .await
        .unwrap();
    assert_eq!(imported.imported, 1);
    assert!(imported.warnings.is_empty());
    assert_eq!(host_key_count(&bus, &workspace).await, 1);

    assert!(bus
        .import_ssh_known_hosts(SshKnownHostsImportInput {
            workspace_id: workspace.clone(),
            content: "   ".into(),
        })
        .await
        .is_err());
    assert!(bus
        .reset_ssh_host_fingerprint(SshHostKeyInput {
            workspace_id: workspace.clone(),
            host: " ".into(),
            port: 22,
        })
        .await
        .is_err());
    assert!(bus
        .clear_ssh_task_runs(SshTaskCleanupInput {
            workspace_id: workspace.clone(),
            task_id: Some("missing-task".into()),
        })
        .await
        .is_err());
    assert_eq!(host_key_count(&bus, &workspace).await, 1);
    assert_eq!(task_run_count(&bus, &workspace).await, 1);

    sqlx::query(
        "CREATE TRIGGER fail_ssh_completion BEFORE INSERT ON activity_events WHEN NEW.action IN ('ssh.task.history.clear','ssh.host_key.reset','ssh.known_hosts.import') BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;",
    )
    .execute(bus.db.pool())
    .await
    .unwrap();

    let cleared = bus
        .clear_ssh_task_runs(SshTaskCleanupInput {
            workspace_id: workspace.clone(),
            task_id: Some(task.task.id.clone()),
        })
        .await
        .expect("deleted task history must survive activity failure");
    assert_eq!(cleared.deleted_runs, 1);
    assert_eq!(cleared.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert_eq!(task_run_count(&bus, &workspace).await, 0);
    let mut serialized = serde_json::to_value(&cleared).unwrap();
    assert!(!serialized.to_string().contains("audit sink"));
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: SshTaskCleanupResult = serde_json::from_value(serialized).unwrap();
    assert!(legacy.warnings.is_empty());

    let reset = bus
        .reset_ssh_host_fingerprint(SshHostKeyInput {
            workspace_id: workspace.clone(),
            host: "example.com".into(),
            port: 22,
        })
        .await
        .expect("deleted fingerprint must survive activity failure");
    assert!(reset.deleted);
    assert_eq!(reset.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert_eq!(host_key_count(&bus, &workspace).await, 0);
    let mut serialized = serde_json::to_value(&reset).unwrap();
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: SshHostKeyResetResult = serde_json::from_value(serialized).unwrap();
    assert!(legacy.warnings.is_empty());
    let second = bus
        .reset_ssh_host_fingerprint(SshHostKeyInput {
            workspace_id: workspace.clone(),
            host: "example.com".into(),
            port: 22,
        })
        .await
        .unwrap();
    assert!(!second.deleted);
    assert!(second.warnings.is_empty());
    assert_eq!(host_key_count(&bus, &workspace).await, 0);

    let reimported = bus
        .import_ssh_known_hosts(SshKnownHostsImportInput {
            workspace_id: workspace.clone(),
            content: KNOWN_HOST.into(),
        })
        .await
        .expect("imported host key must survive activity failure");
    assert_eq!(reimported.imported, 1);
    assert_eq!(reimported.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert_eq!(host_key_count(&bus, &workspace).await, 1);
    let mut serialized = serde_json::to_value(&reimported).unwrap();
    assert!(!serialized.to_string().contains("AAAAB3NzaC1yc2E"));
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: SshKnownHostsImportResult = serde_json::from_value(serialized).unwrap();
    assert!(legacy.warnings.is_empty());

    let recorded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action IN ('ssh.task.history.clear','ssh.host_key.reset','ssh.known_hosts.import')",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(recorded, 1, "only the pre-failure import is recorded");
}

#[cfg(not(feature = "ssh-native"))]
#[tokio::test]
async fn ssh_session_activity_failure_keeps_the_session_id() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let saved = bus
        .save_ssh_connection(connection(workspace.clone()))
        .await
        .unwrap();

    sqlx::query("CREATE TRIGGER fail_terminal_history BEFORE INSERT ON ssh_terminal_history BEGIN SELECT RAISE(FAIL, 'history unavailable'); END;")
        .execute(bus.db.pool())
        .await
        .unwrap();
    let history_failure = bus
        .connect_ssh_session(connect_input(workspace.clone(), saved.id.clone()))
        .await;
    let error = history_failure.expect_err("terminal history failure must stay an error");
    assert!(
        error.to_string().contains("history unavailable"),
        "engine history error was rewritten: {error}"
    );
    assert!(
        !error.to_string().contains("ACTIVITY_RECORD_FAILED"),
        "history persistence must stay distinct from activity warnings: {error}"
    );
    let listed = bus.list_ssh_sessions(workspace.clone()).await.unwrap();
    assert!(
        listed.iter().all(|session| !matches!(
            session.status.as_str(),
            "connected" | "degraded" | "reconnecting"
        )),
        "failed history persistence left a live session: {listed:?}"
    );
    let recorded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action = 'ssh.session.connect'",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(recorded, 0);
    sqlx::query("DROP TRIGGER fail_terminal_history")
        .execute(bus.db.pool())
        .await
        .unwrap();

    sqlx::query("CREATE TRIGGER fail_session_activity BEFORE INSERT ON activity_events WHEN NEW.action IN ('ssh.session.connect','ssh.session.close') BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;")
        .execute(bus.db.pool())
        .await
        .unwrap();
    assert!(bus
        .connect_ssh_session(connect_input(
            workspace.clone(),
            "missing-connection".into()
        ))
        .await
        .is_err());
    let session = bus
        .connect_ssh_session(connect_input(workspace.clone(), saved.id.clone()))
        .await
        .expect("connected session id must survive activity failure");
    assert!(!session.session_id.is_empty());
    assert_eq!(session.status, "connected");
    assert_eq!(session.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert!(!serde_json::to_string(&session)
        .unwrap()
        .contains("audit sink"));
    let listed = bus.list_ssh_sessions(workspace.clone()).await.unwrap();
    assert_eq!(
        listed
            .iter()
            .filter(|item| item.session_id == session.session_id)
            .count(),
        1
    );
    let mut serialized = serde_json::to_value(&session).unwrap();
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: SshSessionSummary = serde_json::from_value(serialized).unwrap();
    assert!(legacy.warnings.is_empty());

    assert!(bus
        .close_ssh_session(SshCloseInput {
            workspace_id: workspace.clone(),
            session_id: "missing-session".into(),
        })
        .await
        .is_err());
    let closed = bus
        .close_ssh_session(SshCloseInput {
            workspace_id: workspace.clone(),
            session_id: session.session_id.clone(),
        })
        .await
        .expect("closed session must survive activity failure");
    assert_eq!(closed.session_id, session.session_id);
    assert_eq!(closed.status, "disconnected");
    assert_eq!(closed.warnings, ["ACTIVITY_RECORD_FAILED"]);
    let listed = bus.list_ssh_sessions(workspace.clone()).await.unwrap();
    assert_eq!(
        listed
            .iter()
            .filter(|item| item.session_id == session.session_id)
            .count(),
        1
    );
    assert_eq!(
        listed
            .iter()
            .find(|item| item.session_id == session.session_id)
            .unwrap()
            .status,
        "disconnected"
    );
    let recorded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action IN ('ssh.session.connect','ssh.session.close')",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(recorded, 0);
}
