use super::super::*;
use super::support::*;

#[tokio::test]
async fn task_secrets_are_redacted_from_events_errors_and_persisted_logs() {
    let (service, workspace_id) = service().await;
    let detail = service
        .save_task(docker_export_input(workspace_id))
        .await
        .unwrap();
    let step = detail.steps[0].clone();
    let secret = "task-secret-value";
    let secret_values = vec![secret.to_string()];
    let output = task_run_event_from_runner(
        "run-id",
        &detail.task.id,
        RunnerEvent::Driver(
            step.clone(),
            DriverEvent::Output {
                stream: "stdout".to_string(),
                data: format!("token={secret}\n"),
            },
        ),
        &secret_values,
    );
    let failed = task_run_event_from_runner(
        "run-id",
        &detail.task.id,
        RunnerEvent::StepFinished {
            step,
            status: "failed".to_string(),
            duration_ms: 1,
            exit_code: Some(1),
            error: Some(format!("remote rejected {secret}")),
        },
        &secret_values,
    );
    let outcome = redact_task_run_outcome(
        TaskRunOutcome {
            status: "failed".to_string(),
            error: Some(format!("run failed with {secret}")),
        },
        &secret_values,
    );
    let final_event = run_event("run-id", &detail.task.id, &outcome.status, outcome.error);

    for event in [&output, &failed, &final_event] {
        let serialized = serde_json::to_string(event).unwrap();
        assert!(!serialized.contains(secret));
        assert!(serialized.contains(unfour_core::redaction::REDACTED_VALUE));
    }

    std::fs::create_dir_all(service.task_log_dir.as_ref()).unwrap();
    let log_path = service.task_log_dir.join("secret-redaction.log");
    {
        let mut log = TaskLogWriter::create(&log_path).unwrap();
        log.write_event(&output);
        log.write_event(&failed);
        log.write_event(&final_event);
    }
    let log = std::fs::read_to_string(&log_path).unwrap();
    assert!(!log.contains(secret));
    assert!(log.contains(unfour_core::redaction::REDACTED_VALUE));
    std::fs::remove_dir_all(service.task_log_dir.as_ref()).unwrap();
}
