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
    UnknownWithDiagnostics,
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
                Mode::UnknownWithDiagnostics => Err(AppError::FlowActionFailed {
                    source: Box::new(invalid("FLOW_OUTCOME_UNCERTAIN")),
                    details: json!({"log": "x".repeat(32 * 1024)}),
                }),
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
async fn flow_preserves_uncertain_outcome_when_ssh_diagnostics_exceed_history_limit() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let mut step = effect(20_000);
    step["action"]["capability"] = json!("ssh");
    // Expand a valid invocation input sixteen times in the recorded attempt.
    let bindings = (0..16)
        .map(|i| (format!("arg{i}"), json!({"$ref":"/inputs/value"})))
        .collect::<serde_json::Map<_, _>>();
    step["action"]["arguments"] = json!({"inputs": bindings});
    let flow = service
        .save(definition(&workspace, json!([step, later_wait()])))
        .await
        .unwrap();
    for (input_size, fits_without_log) in [(255 * 1024, true), (256 * 1024 - 16, false)] {
        let mut input = request(&workspace, &flow.id);
        input.inputs = json!({"value": "v".repeat(input_size)});
        let executor = Arc::new(Scripted {
            started: Mutex::new(None),
            mode: Mode::UnknownWithDiagnostics,
        });
        let started = service.run(input, executor).await.unwrap();
        let run = finished(&bus, &started).await;

        assert_eq!(run.error.as_deref(), Some("FLOW_OUTCOME_UNCERTAIN"));
        let attempt = &run.steps[0].attempts[0];
        assert_eq!(attempt.error, run.error);
        assert_eq!(attempt.input.is_object(), fits_without_log);
        assert!(attempt.output.is_none());
        assert!(run.steps[0].output.is_none());
        assert_eq!(run.steps[1].status, FlowStepRunStatus::Skipped);
        assert!(serde_json::to_vec(&run.steps).unwrap().len() <= 4 * 1024 * 1024);
        assert!(service.list_runs(&workspace, &flow.id).await.unwrap()[0].outcome_uncertain);
    }
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

#[tokio::test]
async fn flow_history_write_failure_does_not_rewrite_completed_action() {
    for failed_action in [false, true] {
        let bus = test_bus().await;
        let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
        let service = FlowService::new(bus.db.clone());
        let mut step = effect(5_000);
        step["action"]["resourceId"] = json!(if failed_action { "fail" } else { "ok" });
        step["action"]["arguments"] = json!({"confirmed": true});
        let flow = service
            .save(definition(&workspace, json!([step, later_wait()])))
            .await
            .unwrap();
        // Fail the result journal, but allow the terminal summary to be written.
        sqlx::query(
            "CREATE TRIGGER reject_result_journal BEFORE UPDATE OF run_json ON flow_runs
            WHEN NEW.finished_at IS NULL AND
              (json_extract(NEW.run_json, '$.steps[0].attempts[0].output') IS NOT NULL OR
               json_extract(NEW.run_json, '$.steps[0].attempts[0].error') IS NOT NULL)
            BEGIN SELECT RAISE(FAIL, 'fixture result journal unavailable'); END",
        )
        .execute(bus.db.pool())
        .await
        .unwrap();
        let driver = Arc::new(Driver::default());
        let run = service
            .run(request(&workspace, &flow.id), driver.clone())
            .await
            .unwrap();
        let result = finished(&bus, &run).await;
        assert_eq!(driver.calls.lock().unwrap().len(), 1);
        assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
        if failed_action {
            assert_eq!(result.status, FlowRunStatus::Failed);
            assert_eq!(result.error.as_deref(), Some("FLOW_TEST_FAILURE"));
            assert_eq!(result.steps[0].status, FlowStepRunStatus::Failed);
            assert_eq!(result.steps[0].error.as_deref(), Some("FLOW_TEST_FAILURE"));
        } else {
            assert_eq!(result.status, FlowRunStatus::Interrupted);
            assert_eq!(result.error.as_deref(), Some("FLOW_HISTORY_WRITE_FAILED"));
            assert_eq!(result.steps[0].status, FlowStepRunStatus::Succeeded);
            assert_eq!(result.steps[0].output.as_ref().unwrap()["confirmed"], true);
        }
        assert_history_matches(&bus, &result).await;
    }
}

#[tokio::test]
async fn flow_http_disconnect_after_remote_effect_is_uncertain_without_retry() {
    use std::io::{BufRead, BufReader};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/effect", listener.local_addr().unwrap());
    let effects = Arc::new(AtomicUsize::new(0));
    let observed = effects.clone();
    let server = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let (socket, _) = loop {
            match listener.accept() {
                Ok(accepted) => break accepted,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("HTTP fixture did not receive a request: {error}"),
            }
        };
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(socket);
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        observed.fetch_add(1, Ordering::SeqCst);
        // Commit the fixture effect, then drop the actual TCP transport
        // without sending an HTTP response.
    });
    let mut api = api_script_test_input(workspace.clone(), url);
    api.method = "POST".into();
    api.body = None;
    let api = bus.save_api_request(api).await.unwrap();
    let mut step = effect(5000);
    step["action"]["resourceId"] = json!(api.id);
    let flow = bus
        .save_flow(definition(&workspace, json!([step, later_wait()])))
        .await
        .unwrap();
    let run = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    let result = finished(&bus, &run).await;
    server.join().unwrap();
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    assert_unconfirmed(&result);
    assert_eq!(result.steps[0].attempts.len(), 1);
    assert!(bus.list_flow_runs(workspace, flow.id).await.unwrap()[0].outcome_uncertain);
}

#[tokio::test]
async fn flow_action_output_budget_preserves_success_and_stops_further_effects() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let mut step = effect(5_000);
    step["action"]["arguments"] = json!({"committed":true,"payload":"x".repeat(270_000)});
    let flow = service
        .save(definition(&workspace, json!([step, later_wait()])))
        .await
        .unwrap();
    let driver = Arc::new(Driver::default());
    let run = service
        .run(request(&workspace, &flow.id), driver.clone())
        .await
        .unwrap();
    let result = finished(&bus, &run).await;
    assert_eq!(result.status, FlowRunStatus::Interrupted);
    assert_eq!(result.error.as_deref(), Some("FLOW_OUTPUT_TOO_LARGE"));
    assert_eq!(result.steps[0].status, FlowStepRunStatus::Succeeded);
    assert_eq!(
        result.steps[0].output.as_ref().unwrap()["outputOmitted"],
        true
    );
    assert_eq!(result.steps[0].attempts[0].error, None);
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
    assert_eq!(driver.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn flow_action_history_budget_preserves_known_success_and_failure() {
    for failed in [false, true] {
        let bus = test_bus().await;
        let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
        let service = FlowService::new(bus.db.clone());
        let mut step = effect(500);
        let bindings = (0..17)
            .map(|i| (format!("arg{i}"), json!({"$ref":"/inputs/value"})))
            .collect::<serde_json::Map<_, _>>();
        step["action"]["arguments"] = json!(bindings);
        let flow = service
            .save(definition(&workspace, json!([step, later_wait()])))
            .await
            .unwrap();
        let mut input = request(&workspace, &flow.id);
        input.inputs = json!({"value":"v".repeat(255 * 1024)});
        let driver = Arc::new(Scripted {
            started: Mutex::new(None),
            mode: if failed {
                Mode::FailureAfterSignal
            } else {
                Mode::SuccessAfterSignal
            },
        });
        let run = service.run(input, driver).await.unwrap();
        let result = finished(&bus, &run).await;
        assert_eq!(result.steps[0].attempts.len(), 1);
        assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
        assert!(serde_json::to_vec(&result.steps).unwrap().len() <= 4_194_304);
        if failed {
            assert_eq!(result.status, FlowRunStatus::Failed);
            assert_eq!(result.error.as_deref(), Some("FLOW_TEST_FAILURE"));
            assert_eq!(result.steps[0].error, result.error);
        } else {
            assert_eq!(result.status, FlowRunStatus::Interrupted);
            assert_eq!(result.error.as_deref(), Some("FLOW_HISTORY_LIMIT"));
            assert_eq!(result.steps[0].status, FlowStepRunStatus::Succeeded);
            assert_eq!(
                result.steps[0].output.as_ref().unwrap()["outputOmitted"],
                true
            );
        }
    }
}
