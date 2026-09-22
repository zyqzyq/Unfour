use super::*;
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use unfour_core::models::*;
use unfour_flow_engine::{FlowExecutor, FlowFuture, FlowService};
#[path = "flow_history.rs"]
mod history;
#[cfg(feature = "ssh-native")]
#[path = "flow_native.rs"]
mod native;
#[path = "flow_wait_until.rs"]
mod wait_until;

pub(super) fn definition(workspace: &str, steps: Value) -> FlowDefinition {
    serde_json::from_value(json!({"id":"", "workspaceId":workspace, "name":"test", "revision":0,"inputs":["value"],"steps":steps})).unwrap()
}
pub(super) fn request(workspace: &str, id: &str) -> FlowRunInput {
    serde_json::from_value(json!({"workspaceId":workspace,"flowId":id,"environmentId":null,"inputs":{"value":42},"initiator":"human","confirmEffects":true})).unwrap()
}
pub(super) fn action(id: &str, capability: &str, resource: &str, arguments: Value) -> Value {
    json!({"id":id,"name":id,"kind":"action","timeoutMs":2000,"action":{"capability":capability,"resourceId":resource,"connectionId":null,"arguments":arguments}})
}
fn next(mut action: Value, target: &str) -> Value {
    action["next"] = json!(target);
    action
}
pub(super) async fn finished(bus: &CommandBus, run: &FlowRun) -> FlowRun {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let current = bus
                .get_flow_run(run.workspace_id.clone(), run.id.clone())
                .await
                .unwrap();
            if current.status != FlowRunStatus::Running {
                return current;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}
#[derive(Default)]
pub(super) struct Driver {
    pub(super) calls: Mutex<Vec<Value>>,
}
impl FlowExecutor for Driver {
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
    ) -> FlowFuture<'a, Value> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(action.arguments.clone());
            if action.resource_id == "fail" {
                return Err(AppError::Validation("FLOW_TEST_FAILURE".into()));
            }
            Ok(action.arguments.clone())
        })
    }
}

#[tokio::test]
async fn flow_serial_references_exclusive_branch_history_and_revision() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let driver = Arc::new(Driver::default());
    let mut flow = service.save(definition(&workspace, json!([
        action("api", "api", "api", json!({"value":{"$ref":"/inputs/value"}})),
        {"id":"condition","name":"condition","kind":"condition","timeoutMs":1000,"predicate":{"left":{"$ref":"/steps/api/value"},"op":"eq","right":42},"ifTrue":"ssh","ifFalse":"failure"},
        action("ssh", "ssh", "ssh", json!({"message":"value=${/steps/api/value}"})),
        next(action("db", "database", "db", json!({"value":{"$ref":"/steps/ssh/message"}})), "$end"),
        action("failure", "api", "fail", json!({}))
    ]))).await.unwrap();
    let run = service
        .run(request(&workspace, &flow.id), driver.clone())
        .await
        .unwrap();
    flow.name = "changed".into();
    service.save(flow.clone()).await.unwrap();
    assert!(service.save(flow.clone()).await.is_err());
    let result = finished(&bus, &run).await;
    assert_eq!(
        result.status,
        FlowRunStatus::Succeeded,
        "{:?}",
        result.error
    );
    assert_eq!(result.definition.name, "test");
    assert_eq!(result.definition.revision, 1);
    assert_eq!(result.steps[4].status, FlowStepRunStatus::Skipped);
    assert_eq!(driver.calls.lock().unwrap()[2]["value"], "value=42");
    let mut false_input = request(&workspace, &flow.id);
    false_input.inputs = json!({"value":43});
    let false_run = service.run(false_input, driver.clone()).await.unwrap();
    let false_result = finished(&bus, &false_run).await;
    assert_eq!(false_result.status, FlowRunStatus::Failed);
    assert_eq!(false_result.steps[2].status, FlowStepRunStatus::Skipped);
    assert_eq!(false_result.steps[3].status, FlowStepRunStatus::Skipped);
    assert_eq!(false_result.steps[4].status, FlowStepRunStatus::Failed);
    service.delete(&workspace, &flow.id).await.unwrap();
    assert_eq!(
        service.list_runs(&workspace, &flow.id).await.unwrap().len(),
        2
    );
    assert!(service.get_run("other", &run.id).await.is_err());
}

#[tokio::test]
async fn flow_fail_fast_missing_variable_and_backward_branch() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    for arguments in [json!({}), json!({"x":{"$ref":"/steps/missing/value"}})] {
        let driver = Arc::new(Driver::default());
        let flow = service
            .save(definition(
                &workspace,
                json!([
                    action("fail", "api", "fail", arguments),
                    action("later", "ssh", "ssh", json!({}))
                ]),
            ))
            .await
            .unwrap();
        let run = service
            .run(request(&workspace, &flow.id), driver.clone())
            .await
            .unwrap();
        let result = finished(&bus, &run).await;
        assert_eq!(result.status, FlowRunStatus::Failed);
        assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
        assert!(driver.calls.lock().unwrap().len() <= 1);
    }
    let backward = definition(
        &workspace,
        json!([next(action("a", "api", "a", json!({})), "a")]),
    );
    assert!(service.save(backward).await.is_err());
}

#[tokio::test]
async fn flow_poll_latest_attempts_timeout_and_cancel() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    for (timeout, attempts, expected) in [
        (2000, 3, "failed"),
        (30, 100, "timedOut"),
        (2000, 100, "cancelled"),
    ] {
        let flow=service.save(definition(&workspace,json!([{"id":"poll","name":"poll","kind":"poll","timeoutMs":timeout,"probe":{"capability":"api","resourceId":"probe","arguments":{"ready":false}},"predicate":{"left":{"$ref":"/probe/ready"},"op":"eq","right":true},"intervalMs":10,"maxAttempts":attempts},action("after","api","after",json!({}))]))).await.unwrap();
        let driver = Arc::new(Driver::default());
        let run = service
            .run(request(&workspace, &flow.id), driver.clone())
            .await
            .unwrap();
        if expected == "cancelled" {
            tokio::time::sleep(Duration::from_millis(20)).await;
            service.cancel(&workspace, &run.id).await.unwrap();
        }
        let result = finished(&bus, &run).await;
        assert_eq!(result.status.as_str(), expected, "{:?}", result.error);
        assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
        if attempts == 3 {
            assert_eq!(result.steps[0].attempts.len(), 3);
            assert_eq!(result.error.as_deref(), Some("FLOW_MAX_ATTEMPTS"));
        }
    }
}

#[tokio::test]
async fn flow_real_http_start_poll_and_database() {
    use std::io::{BufRead, BufReader, Write};
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let mut paths = vec![];
        for index in 0..4 {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            paths.push(first);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
            }
            let body = if index == 0 {
                json!({"jobId":42})
            } else {
                json!({"ready":index==3,"attempt":index})
            }
            .to_string();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        }
        paths
    });
    let mut start_input = api_script_test_input(workspace.clone(), format!("{url}/start"));
    start_input.method = "POST".into();
    let start = bus.save_api_request(start_input).await.unwrap();
    let probe = bus
        .save_api_request(api_script_test_input(
            workspace.clone(),
            format!("{url}/status"),
        ))
        .await
        .unwrap();
    let db = bus
        .save_database_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "flow sqlite".into(),
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
    let flow=bus.save_flow(definition(&workspace,json!([
        action("start","api",&start.id,json!({})),
        {"id":"poll","name":"poll","kind":"poll","timeoutMs":5000,"probe":{"capability":"api","resourceId":probe.id,"arguments":{"url":format!("{url}/status/${{/steps/start/body/jobId}}")}},"predicate":{"left":{"$ref":"/probe/body/ready"},"op":"eq","right":true},"intervalMs":10,"maxAttempts":5},
        action("db","database",&db.id,json!({"sql":"SELECT ${/steps/poll/body/attempt} AS attempts"}))
    ]))).await.unwrap();
    let run = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    let result = finished(&bus, &run).await;
    assert_eq!(
        result.status,
        FlowRunStatus::Succeeded,
        "{:?}",
        result.error
    );
    assert_eq!(result.steps[1].attempts.len(), 3);
    assert_eq!(
        result.steps[1].attempts[0].output.as_ref().unwrap()["body"]["ready"],
        false
    );
    assert_eq!(
        result.steps[1].attempts[2].output.as_ref().unwrap()["body"]["ready"],
        true
    );
    let paths = server.join().unwrap();
    let requests: Vec<_> = paths
        .iter()
        .map(|line| {
            let mut parts = line.split_whitespace();
            let method = parts.next().unwrap();
            let path = parts.next().unwrap().trim_end_matches('?');
            (method, path)
        })
        .collect();
    assert_eq!(requests[0], ("POST", "/start"));
    assert!(requests[1..].iter().all(|p| *p == ("GET", "/status/42")));
    bus.delete_api_request(workspace.clone(), start.id)
        .await
        .unwrap();
    let invalid = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    assert_eq!(invalid.status, FlowRunStatus::ValidationFailed);
    assert_eq!(
        bus.get_flow_run(workspace, run.id).await.unwrap().status,
        FlowRunStatus::Succeeded
    );
}

#[tokio::test]
async fn flow_revision_pin_rejects_before_remote_execution_or_history() {
    let bus = test_bus().await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = FlowService::new(bus.db.clone());
    let driver = Arc::new(Driver::default());
    let saved = service
        .save(definition(
            &ws,
            json!([action("effect", "api", "api", json!({"effect":true}))]),
        ))
        .await
        .unwrap();
    let changed = service.save(saved.clone()).await.unwrap();
    let error = service
        .run_at_revision(
            request(&ws, &saved.id),
            driver.clone(),
            Some(saved.revision),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Validation(ref code) if code == "FLOW_CONFIRMATION_STALE"));
    assert!(driver.calls.lock().unwrap().is_empty());
    assert!(service.list_runs(&ws, &saved.id).await.unwrap().is_empty());
    let error = bus
        .run_flow_at_revision(request(&ws, &saved.id), Some(saved.revision))
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Validation(ref code) if code == "FLOW_CONFIRMATION_STALE"));
    assert!(service.list_runs(&ws, &saved.id).await.unwrap().is_empty());
    let run = service
        .run_at_revision(
            request(&ws, &saved.id),
            driver.clone(),
            Some(changed.revision),
        )
        .await
        .unwrap();
    let result = finished(&bus, &run).await;
    assert_eq!(result.definition.revision, changed.revision);
    assert_eq!(result.status, FlowRunStatus::Succeeded);
    assert_eq!(driver.calls.lock().unwrap().len(), 1);
}
