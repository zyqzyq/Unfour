use super::*;
use std::collections::VecDeque;

struct ProbeDriver {
    replies: Mutex<VecDeque<AppResult<Value>>>,
    calls: Mutex<Vec<String>>,
}
impl FlowExecutor for ProbeDriver {
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
        action: &'a FlowAction,
        _: &'a Value,
        _: &'a FlowRunInput,
        _: CancellationToken,
        _: &'a unfour_flow_engine::FlowPersistenceContext,
    ) -> FlowFuture<'a, Value> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(action.resource_id.clone());
            if action.resource_id == "probe" {
                self.replies
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or(Ok(json!({"state":"pending"})))
            } else {
                Ok(action.arguments.clone())
            }
        })
    }
}
fn wait_step(policy: &str) -> Value {
    json!({"id":"check","name":"check","kind":"waitUntil","timeoutMs":2000,
        "probe":{"capability":"api","resourceId":"probe","arguments":{}},
        "successWhen":{"left":{"$ref":"/probe/state"},"op":"eq","right":"success"},
        "failureWhen":{"left":{"$ref":"/probe/state"},"op":"in","right":["failed","cancelled"]},
        "probeErrorPolicy":policy,"intervalMs":10,"maxAttempts":5})
}
async fn execute(replies: Vec<AppResult<Value>>, step: Value) -> (FlowRun, Arc<ProbeDriver>) {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let flow = service.save(definition(&workspace, json!([
        action("start","api","start",json!({})), step,
        action("after","api","after",json!({"result":{"$ref":"/steps/check/result"},"attempts":{"$ref":"/steps/check/attempts"},"elapsedMs":{"$ref":"/steps/check/elapsedMs"}}))
    ]))).await.unwrap();
    let driver = Arc::new(ProbeDriver {
        replies: Mutex::new(replies.into()),
        calls: Mutex::new(vec![]),
    });
    let run = service
        .run(request(&workspace, &flow.id), driver.clone())
        .await
        .unwrap();
    (finished(&bus, &run).await, driver)
}

#[tokio::test]
async fn flow_wait_until_retries_transient_and_exposes_latest_result_summary() {
    let (run, driver) = execute(
        vec![
            Err(AppError::ApiNetwork("sensitive diagnostic".into())),
            Err(AppError::ApiTimeout("timeout".into())),
            Ok(json!({"state":"pending"})),
            Ok(json!({"state":"success","value":42})),
        ],
        wait_step("retryTransientErrors"),
    )
    .await;
    assert_eq!(serde_json::to_value(&run).unwrap()["status"], "succeeded");
    assert_eq!(
        driver.calls.lock().unwrap().as_slice(),
        ["start", "probe", "probe", "probe", "probe", "after"]
    );
    let attempts = &run.steps[1].attempts;
    assert_eq!(attempts[0].error.as_deref(), Some("NETWORK_ERROR"));
    assert_eq!(attempts[1].error.as_deref(), Some("API_TIMEOUT"));
    let summary = run.steps[2].attempts[0].output.as_ref().unwrap();
    assert_eq!(summary["result"]["value"], 42);
    assert_eq!(summary["attempts"], 4);
    assert!(summary["elapsedMs"].as_u64().unwrap() >= 30);
    assert!(!serde_json::to_string(&run)
        .unwrap()
        .contains("sensitive diagnostic"));
}

#[tokio::test]
async fn flow_wait_until_final_output_respects_total_history_limit() {
    let mut step = wait_step("failImmediately");
    step["maxAttempts"] = json!(20);
    step["timeoutMs"] = json!(20000);
    let replies = (0..17)
        .map(|i| {
            Ok(json!({"state":if i==16 {"success"} else {"pending"},"padding":"x".repeat(240_000)}))
        })
        .collect();
    let (run, driver) = execute(replies, step).await;
    assert_eq!(run.error.as_deref(), Some("FLOW_HISTORY_LIMIT"));
    assert_eq!(run.steps[1].attempts.len(), 17);
    assert!(run.steps[1].output.is_none());
    assert_eq!(driver.calls.lock().unwrap().len(), 18);
}

#[tokio::test]
async fn flow_wait_until_failure_precedes_success_and_errors_fail_fast() {
    let mut step = wait_step("retryTransientErrors");
    step["successWhen"] = json!({"left":true,"op":"eq","right":true});
    let (run, driver) = execute(vec![Ok(json!({"state":"failed"}))], step).await;
    assert_eq!(run.error.as_deref(), Some("FLOW_FAILURE_CONDITION"));
    assert_eq!(driver.calls.lock().unwrap().len(), 2);
    for (policy, error, code) in [
        (
            "retryTransientErrors",
            AppError::ReadOnly("blocked".into()),
            "READ_ONLY_CONNECTION",
        ),
        (
            "failImmediately",
            AppError::ApiNetwork("offline".into()),
            "NETWORK_ERROR",
        ),
    ] {
        let (run, driver) = execute(vec![Err(error)], wait_step(policy)).await;
        assert_eq!(run.error.as_deref(), Some(code));
        assert_eq!(driver.calls.lock().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn flow_wait_until_exhaustion_timeout_cancel_and_progress() {
    let mut step = wait_step("retryTransientErrors");
    step["maxAttempts"] = json!(2);
    let (run, _) = execute(vec![], step.clone()).await;
    assert_eq!(run.error.as_deref(), Some("FLOW_MAX_ATTEMPTS"));
    assert_eq!(run.steps[1].attempts.len(), 2);
    step["timeoutMs"] = json!(40);
    step["maxAttempts"] = Value::Null;
    let (run, _) = execute(vec![], step.clone()).await;
    assert_eq!(serde_json::to_value(&run).unwrap()["status"], "timedOut");

    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    step["timeoutMs"] = json!(5000);
    step["intervalMs"] = json!(1000);
    let flow = service
        .save(definition(&workspace, json!([step])))
        .await
        .unwrap();
    let driver = Arc::new(ProbeDriver {
        replies: Mutex::new(VecDeque::new()),
        calls: Mutex::new(vec![]),
    });
    let run = service
        .run(request(&workspace, &flow.id), driver)
        .await
        .unwrap();
    let progress = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let view = service.get_run(&workspace, &run.id).await.unwrap();
            let value = serde_json::to_value(&view).unwrap();
            if value["steps"][0]["nextCheckAt"].is_string() {
                break value;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(progress["steps"][0]["startedAt"].is_string());
    assert_eq!(
        progress["steps"][0]["attempts"][0]["output"]["state"],
        "pending"
    );
    service.cancel(&workspace, &run.id).await.unwrap();
    let result = finished(&bus, &run).await;
    assert_eq!(
        serde_json::to_value(&result).unwrap()["status"],
        "cancelled"
    );
    assert_eq!(result.steps[0].attempts.len(), 1);
}

#[tokio::test]
async fn flow_structured_secret_inputs_are_redacted_even_on_validation_failure() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let mut flow = definition(
        &workspace,
        json!([action(
            "echo",
            "api",
            "echo",
            json!({"copy":{"$ref":"/inputs/custom"}})
        )]),
    );
    flow.inputs = serde_json::from_value(json!([
        {"name":"custom","type":"string","required":true,"secret":true,"description":"Deployment password"},
        {"name":"dryRun","type":"boolean","default":false}
    ]))
    .unwrap();
    let flow = service.save(flow).await.unwrap();
    for valid in [true, false] {
        let mut input = request(&workspace, &flow.id);
        input.inputs = json!({"custom":"secret-unique-fixture"});
        if !valid {
            input.inputs["dryRun"] = json!("wrong type");
        }
        let run = service
            .run(input, Arc::new(Driver::default()))
            .await
            .unwrap();
        let result = finished(&bus, &run).await;
        assert_eq!(
            result.status,
            if valid {
                FlowRunStatus::Succeeded
            } else {
                FlowRunStatus::ValidationFailed
            }
        );
        let stored: String = sqlx::query_scalar("SELECT run_json FROM flow_runs WHERE id = ?")
            .bind(&run.id)
            .fetch_one(bus.db.pool())
            .await
            .unwrap();
        assert!(!stored.contains("secret-unique-fixture"));
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("secret-unique-fixture"));
        if valid {
            assert_eq!(result.context.inputs["dryRun"], false);
        }
    }
    for malformed in [
        json!([{"custom":"malformed-secret-fixture"}]),
        json!("malformed-secret-fixture"),
    ] {
        let mut input = request(&workspace, &flow.id);
        input.inputs = malformed;
        let run = service
            .run(input, Arc::new(Driver::default()))
            .await
            .unwrap();
        assert_eq!(run.status, FlowRunStatus::ValidationFailed);
        let stored: String = sqlx::query_scalar("SELECT run_json FROM flow_runs WHERE id = ?")
            .bind(&run.id)
            .fetch_one(bus.db.pool())
            .await
            .unwrap();
        assert!(!stored.contains("malformed-secret-fixture"));
    }
}

#[tokio::test]
async fn flow_wait_until_real_http_transient_statuses_and_permanent_failure() {
    use std::io::{BufRead, BufReader, Write};
    for (statuses, expected) in [
        (vec![503, 429, 408, 200], FlowRunStatus::Succeeded),
        (vec![401], FlowRunStatus::Failed),
    ] {
        let bus = test_bus().await;
        let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let expected_attempts = statuses.len();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut paths = vec![];
            for status in std::iter::once(200).chain(statuses) {
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(std::time::Instant::now() < deadline, "fixture timed out");
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                paths.push(first);
                loop {
                    let mut line = String::new();
                    assert!(
                        reader.read_line(&mut line).unwrap() > 0,
                        "request closed before headers"
                    );
                    if line == "\r\n" {
                        break;
                    }
                }
                let body = r#"{"state":"success","job":42}"#;
                write!(socket,"HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
            paths
        });
        let mut start_input = api_script_test_input(workspace.clone(), format!("{url}/start"));
        start_input.method = "POST".into();
        let start = bus.save_api_request(start_input).await.unwrap();
        let probe = bus
            .save_api_request(api_script_test_input(
                workspace.clone(),
                format!("{url}/probe"),
            ))
            .await
            .unwrap();
        let mut step = wait_step("retryTransientErrors");
        step["probe"]["resourceId"] = json!(probe.id);
        step["successWhen"]["left"] = json!({"$ref":"/probe/body/state"});
        step["failureWhen"] = Value::Null;
        let flow = bus
            .save_flow(definition(
                &workspace,
                json!([action("start", "api", &start.id, json!({})), step]),
            ))
            .await
            .unwrap();
        let run = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
        let result = finished(&bus, &run).await;
        assert_eq!(result.status, expected, "{:?}", result.error);
        assert_eq!(result.steps[1].attempts.len(), expected_attempts);
        assert_eq!(
            result.steps[1].attempts[0].error.as_deref(),
            Some(if expected == FlowRunStatus::Succeeded {
                "FLOW_HTTP_STATUS_503"
            } else {
                "FLOW_HTTP_STATUS_401"
            })
        );
        let paths = server.join().unwrap();
        assert!(paths[0].starts_with("POST /start"));
        assert!(paths[1..].iter().all(|path| path.starts_with("GET /probe")));
    }
}

#[tokio::test]
async fn flow_wait_until_invalid_resources_are_rejected_before_any_action() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut api =
        api_script_test_input(workspace.clone(), "http://127.0.0.1:1/never-contact".into());
    api.method = "POST".into();
    let saved = bus.save_api_request(api.clone()).await.unwrap();
    let mut step = wait_step("failImmediately");
    for (id, code) in [
        (saved.id, "FLOW_PROBE_REQUIRES_READ_OPERATION"),
        ("missing".into(), "FLOW_RESOURCE_MISSING"),
    ] {
        step["probe"]["resourceId"] = json!(id);
        let flow = bus
            .save_flow(definition(&workspace, json!([step.clone()])))
            .await
            .unwrap();
        let result = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
        assert_eq!(result.status, FlowRunStatus::ValidationFailed);
        assert_eq!(result.error.as_deref(), Some(code));
        assert!(result.steps[0].attempts.is_empty());
    }
    api.method = "GET".into();
    api.pre_request_script = Some("console.log('unsupported')".into());
    let saved = bus.save_api_request(api).await.unwrap();
    step["probe"]["resourceId"] = json!(saved.id);
    let flow = bus
        .save_flow(definition(&workspace, json!([step])))
        .await
        .unwrap();
    let result = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    assert_eq!(
        result.error.as_deref(),
        Some("FLOW_API_SCRIPT_OR_MULTIPART_UNSUPPORTED")
    );
}
