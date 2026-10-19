use super::flow::{definition, finished, request, Driver};
use super::*;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use unfour_core::models::*;
use unfour_core::AppError;
use unfour_flow_engine::{expression::invalid, FlowExecutor, FlowFuture, FlowService};

#[derive(Clone, Copy)]
enum Mode {
    /// Return only after the runner has cancelled the step token.
    SuccessAfterSignal,
    FailureAfterSignal,
    CleanStop,
    Hang,
}

struct Scripted {
    started: Mutex<Option<oneshot::Sender<()>>>,
    mode: Mode,
}

impl FlowExecutor for Scripted {
    fn prepare<'a>(
        &'a self,
        _: &'a FlowAction,
        _: &'a FlowRunInput,
        _: bool,
    ) -> FlowFuture<'a, Value> {
        Box::pin(async { Ok(json!({})) })
    }

    fn execute<'a>(
        &'a self,
        _: &'a FlowAction,
        _: &'a Value,
        _: &'a FlowRunInput,
        cancel: CancellationToken,
        _: &'a unfour_flow_engine::FlowPersistenceContext,
    ) -> FlowFuture<'a, Value> {
        Box::pin(async move {
            if let Some(started) = self.started.lock().unwrap().take() {
                let _ = started.send(());
            }
            match self.mode {
                Mode::Hang => std::future::pending().await,
                Mode::CleanStop => {
                    cancel.cancelled().await;
                    Err(invalid("FLOW_CANCELLED"))
                }
                Mode::SuccessAfterSignal => {
                    cancel.cancelled().await;
                    tokio::task::yield_now().await;
                    Ok(json!({"confirmed": true}))
                }
                Mode::FailureAfterSignal => {
                    cancel.cancelled().await;
                    tokio::task::yield_now().await;
                    Err(AppError::Validation("FLOW_TEST_FAILURE".into()))
                }
            }
        })
    }
}

fn effect(timeout_ms: u64) -> Value {
    json!({
        "id": "effect",
        "name": "effect",
        "kind": "action",
        "timeoutMs": timeout_ms,
        "action": {
            "capability": "api",
            "resourceId": "effect",
            "connectionId": null,
            "arguments": {}
        }
    })
}

fn later_wait() -> Value {
    json!({"id":"later","name":"later","kind":"wait","timeoutMs":1000,"durationMs":0})
}

async fn assert_history_matches(bus: &CommandBus, run: &FlowRun) {
    let stored: String = sqlx::query_scalar("SELECT run_json FROM flow_runs WHERE id = ?")
        .bind(&run.id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    let persisted: FlowRun = serde_json::from_str(&stored).unwrap();
    assert_eq!(persisted.status, run.status);
    assert_eq!(persisted.error, run.error);
    assert_eq!(persisted.steps.len(), run.steps.len());
    for (stored_step, step) in persisted.steps.iter().zip(&run.steps) {
        assert_eq!(stored_step.status, step.status);
        assert_eq!(stored_step.error, step.error);
        assert_eq!(stored_step.output, step.output);
        assert_eq!(
            stored_step
                .attempts
                .last()
                .map(|attempt| attempt.error.clone()),
            step.attempts.last().map(|attempt| attempt.error.clone())
        );
        assert_eq!(
            stored_step
                .attempts
                .last()
                .map(|attempt| attempt.output.clone()),
            step.attempts.last().map(|attempt| attempt.output.clone())
        );
    }
}

async fn run_scripted(mode: Mode, timeout_ms: u64, cancel: bool) -> (CommandBus, FlowRun) {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let (started_tx, started_rx) = oneshot::channel();
    let executor = Arc::new(Scripted {
        started: Mutex::new(Some(started_tx)),
        mode,
    });
    let flow = service
        .save(definition(
            &workspace,
            json!([effect(timeout_ms), later_wait()]),
        ))
        .await
        .unwrap();
    let run = service
        .run(request(&workspace, &flow.id), executor)
        .await
        .unwrap();
    started_rx.await.unwrap();
    if cancel {
        service.cancel(&workspace, &run.id).await.unwrap();
    }
    let result = finished(&bus, &run).await;
    assert_history_matches(&bus, &result).await;
    (bus, result)
}

#[tokio::test]
async fn flow_success_inside_timeout_grace_continues_the_run() {
    let (_bus, result) = run_scripted(Mode::SuccessAfterSignal, 300, false).await;
    assert_eq!(result.status, FlowRunStatus::Succeeded);
    assert_eq!(result.error, None);
    assert_eq!(result.steps[0].status, FlowStepRunStatus::Succeeded);
    assert_eq!(result.steps[0].error, None);
    assert_eq!(result.steps[0].output.as_ref().unwrap()["confirmed"], true);
    assert_eq!(
        result.steps[0].attempts[0].output.as_ref().unwrap()["confirmed"],
        true
    );
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Succeeded);
}

#[tokio::test]
async fn flow_success_inside_cancel_grace_keeps_history_and_skips_later_steps() {
    let (_bus, result) = run_scripted(Mode::SuccessAfterSignal, 5_000, true).await;
    assert_eq!(result.status, FlowRunStatus::Cancelled);
    assert_eq!(result.steps[0].status, FlowStepRunStatus::Succeeded);
    assert_eq!(result.steps[0].error, None);
    assert_eq!(result.steps[0].output.as_ref().unwrap()["confirmed"], true);
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
}

#[tokio::test]
async fn flow_timeout_and_cancel_keep_a_definite_failure() {
    let (_bus, result) = run_scripted(Mode::FailureAfterSignal, 500, true).await;
    assert_eq!(result.status, FlowRunStatus::Failed);
    assert_eq!(result.error.as_deref(), Some("FLOW_TEST_FAILURE"));
    assert_eq!(result.steps[0].status, FlowStepRunStatus::Failed);
    assert_eq!(result.steps[0].error.as_deref(), Some("FLOW_TEST_FAILURE"));
    assert_eq!(
        result.steps[0].attempts[0].error.as_deref(),
        Some("FLOW_TEST_FAILURE")
    );
    assert!(result.steps[0].output.is_none());
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
}

#[tokio::test]
async fn flow_unconfirmed_attempt_is_not_recorded_as_timeout() {
    let (_bus, result) = run_scripted(Mode::Hang, 300, false).await;
    assert_unconfirmed(&result);
}

#[tokio::test]
async fn flow_unconfirmed_attempt_is_not_recorded_as_cancellation() {
    let (_bus, result) = run_scripted(Mode::Hang, 5_000, true).await;
    assert_unconfirmed(&result);
}

fn assert_unconfirmed(result: &FlowRun) {
    assert_eq!(result.status, FlowRunStatus::Failed);
    assert_ne!(result.status, FlowRunStatus::TimedOut);
    assert_ne!(result.status, FlowRunStatus::Cancelled);
    assert_eq!(result.error.as_deref(), Some("FLOW_OUTCOME_UNCERTAIN"));
    assert_eq!(result.steps[0].status, FlowStepRunStatus::Failed);
    assert_eq!(
        result.steps[0].error.as_deref(),
        Some("FLOW_OUTCOME_UNCERTAIN")
    );
    assert!(result.steps[0].output.is_none());
    assert_eq!(
        result.steps[0].attempts[0].error.as_deref(),
        Some("FLOW_OUTCOME_UNCERTAIN")
    );
    assert!(result.steps[0].attempts[0].output.is_none());
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
}

#[tokio::test]
async fn flow_executor_clean_stop_on_deadline_stays_timed_out() {
    let (_bus, result) = run_scripted(Mode::CleanStop, 300, false).await;
    assert_eq!(result.status, FlowRunStatus::TimedOut);
    assert_eq!(result.error.as_deref(), Some("FLOW_TIMEOUT"));
    assert_eq!(result.steps[0].status, FlowStepRunStatus::TimedOut);
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
}

#[tokio::test]
async fn flow_executor_clean_stop_on_heartbeat_stays_cancelled() {
    let (_bus, result) = run_scripted(Mode::CleanStop, 5_000, true).await;
    assert_eq!(result.status, FlowRunStatus::Cancelled);
    assert_eq!(result.error.as_deref(), Some("FLOW_CANCELLED"));
    assert_eq!(result.steps[0].status, FlowStepRunStatus::Cancelled);
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
}

#[tokio::test]
async fn flow_true_timeout_between_attempts_stays_timed_out() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let flow = service
        .save(definition(
            &workspace,
            json!([{
                "id": "poll",
                "name": "poll",
                "kind": "poll",
                "timeoutMs": 40,
                "probe": {
                    "capability": "api",
                    "resourceId": "probe",
                    "arguments": {"ready": false}
                },
                "predicate": {"left": {"$ref": "/probe/ready"}, "op": "eq", "right": true},
                "intervalMs": 10,
                "maxAttempts": 100
            }, later_wait()]),
        ))
        .await
        .unwrap();
    let run = service
        .run(request(&workspace, &flow.id), Arc::new(Driver::default()))
        .await
        .unwrap();
    let result = finished(&bus, &run).await;
    assert_history_matches(&bus, &result).await;
    assert_eq!(result.status, FlowRunStatus::TimedOut);
    assert_eq!(result.error.as_deref(), Some("FLOW_TIMEOUT"));
    assert_eq!(result.steps[0].status, FlowStepRunStatus::TimedOut);
    assert_ne!(
        result.steps[0].error.as_deref(),
        Some("FLOW_OUTCOME_UNCERTAIN")
    );
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
}

#[tokio::test]
async fn flow_http_response_during_cancel_grace_is_stored() {
    use std::io::{BufRead, BufReader, Write};
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/effect", listener.local_addr().unwrap());
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(socket.try_clone().unwrap());
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        started_tx.send(()).unwrap();
        release_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let body = r#"{"ok":true}"#;
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let api = bus
        .save_api_request(api_script_test_input(workspace.clone(), url))
        .await
        .unwrap();
    let flow = bus
        .save_flow(definition(
            &workspace,
            json!([
                {
                    "id": "effect",
                    "name": "effect",
                    "kind": "action",
                    "timeoutMs": 5000,
                    "action": {
                        "capability": "api",
                        "resourceId": api.id,
                        "connectionId": null,
                        "arguments": {}
                    }
                },
                later_wait()
            ]),
        ))
        .await
        .unwrap();
    let run = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), started_rx)
        .await
        .unwrap()
        .unwrap();
    bus.cancel_flow_run(workspace, run.id.clone())
        .await
        .unwrap();
    // Heartbeat observes the cancel flag every 100ms. Release the response
    // after that, still inside the two-second grace window.
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    release_tx.send(()).unwrap();
    let result = finished(&bus, &run).await;
    assert_history_matches(&bus, &result).await;
    assert_eq!(result.status, FlowRunStatus::Cancelled);
    assert_eq!(result.steps[0].status, FlowStepRunStatus::Succeeded);
    assert_eq!(result.steps[0].error, None);
    assert_eq!(result.steps[0].output.as_ref().unwrap()["status"], 200);
    assert_eq!(result.steps[0].output.as_ref().unwrap()["body"]["ok"], true);
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
    server.join().unwrap();
}
