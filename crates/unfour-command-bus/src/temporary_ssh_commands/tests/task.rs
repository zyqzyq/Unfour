use super::*;
use crate::{CommandBusExtensions, SshTaskExecutionGuard};
use unfour_core::models::{
    SshTaskCancelInput, SshTaskCleanupInput, SshTaskRunInput, SshTaskSaveInput,
};
use unfour_core::{AppError, AppResult};

struct DenyTask;
impl SshTaskExecutionGuard for DenyTask {
    fn validate<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async { Err(AppError::Validation("SYNTHETIC_TASK_GUARD".into())) })
    }
}

#[tokio::test]
async fn temporary_task_bus_keeps_guards_and_leaves_no_runtime_records() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let definition: SshTaskSaveInput = serde_json::from_value(serde_json::json!({
        "workspaceId": workspace, "name": "synthetic task", "defaultConnectionId": null,
        "steps": [{"name":"command", "stepType":"command", "position":0,
            "enabled":true, "configJson":{"command":"fixture-task", "timeoutSeconds":5}}]
    }))
    .unwrap();
    let detail = bus.save_ssh_task(definition).await.unwrap();
    let input = SshTaskRunInput {
        workspace_id: workspace.clone(),
        task_id: detail.task.id.clone(),
        connection_id: None,
        inputs: Default::default(),
        secret_input_names: vec![],
        context: None,
    };
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let mut server = Server {
        temporary_task: true,
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
    let server_task = tokio::spawn(async move {
        server.run_on_socket(config, &socket).await.unwrap();
    });
    let before: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
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
    wait_for(&bus, &workspace, "connected").await;
    bus.extensions =
        CommandBusExtensions::default().with_ssh_task_execution_guards(vec![Arc::new(DenyTask)]);
    for mode in ["exec", "terminal"] {
        assert!(bus
            .run_temporary_ssh_task_with_mode(input.clone(), &handle, mode)
            .await
            .unwrap_err()
            .to_string()
            .contains("SYNTHETIC_TASK_GUARD"));
    }
    assert!(!calls.lock().unwrap().contains(&"task"));
    bus.extensions = CommandBusExtensions::default();
    assert!(bus
        .run_temporary_ssh_task_with_mode(input.clone(), &handle, "automatic-fallback")
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_EXECUTION_MODE_INVALID"));
    let mut conflicting = input.clone();
    conflicting.connection_id = Some("saved-target".into());
    assert!(bus
        .run_temporary_ssh_task(conflicting, &handle)
        .await
        .unwrap_err()
        .to_string()
        .contains("SSH_TASK_TEMPORARY_TARGET_CONFLICT"));
    let run = bus.run_temporary_ssh_task(input, &handle).await.unwrap();
    let finished = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let runs = bus
                .list_ssh_task_runs(workspace.clone(), detail.task.id.clone())
                .await
                .unwrap();
            if let Some(run) = runs
                .into_iter()
                .find(|item| item.id == run.id && item.status != "running")
            {
                break run;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(finished.status, "success");
    assert!(
        finished.connection_id.is_none()
            && finished.context.is_none()
            && finished.log_path.is_empty()
    );
    assert!(finished
        .warnings
        .iter()
        .any(|value| value == "SSH_TASK_TEMPORARY_RUN"));
    let log = bus
        .read_ssh_task_run_log(workspace.clone(), run.id.clone())
        .await
        .unwrap();
    assert!(log.contains("synthetic-task-output"));
    bus.cancel_ssh_task_run(SshTaskCancelInput {
        workspace_id: workspace.clone(),
        run_id: run.id.clone(),
    })
    .await
    .unwrap();
    let cleared = bus
        .clear_ssh_task_runs(SshTaskCleanupInput {
            workspace_id: workspace.clone(),
            task_id: Some(detail.task.id.clone()),
        })
        .await
        .unwrap();
    assert_eq!(cleared.deleted_runs, 1);
    assert_eq!(cleared.deleted_logs, 0);
    let after: (i64,) = sqlx::query_as("SELECT total_changes()")
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "temporary run/list/read/cancel must not write activity, run, binding or logs"
    );
    assert_eq!(
        bus.get_ssh_task(workspace.clone(), detail.task.id.clone())
            .await
            .unwrap()
            .local_binding,
        detail.local_binding
    );
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|&&call| call == "auth")
            .count(),
        1
    );
    bus.close_temporary_ssh_workspace(&workspace).unwrap();
    assert!(bus
        .list_ssh_task_runs(workspace.clone(), detail.task.id)
        .await
        .unwrap()
        .is_empty());
    assert!(bus.read_ssh_task_run_log(workspace, run.id).await.is_err());
    server_task.abort();
}
