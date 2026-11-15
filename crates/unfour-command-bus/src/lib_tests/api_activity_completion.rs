use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::*;

struct CapturedRequest {
    hits: AtomicUsize,
    first: Mutex<String>,
}

fn spawn_counting_api_server() -> (String, Arc<CapturedRequest>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind activity server");
    let address = listener.local_addr().expect("read activity server address");
    let captured = Arc::new(CapturedRequest {
        hits: AtomicUsize::new(0),
        first: Mutex::new(String::new()),
    });
    let captured_thread = captured.clone();
    std::thread::spawn(move || {
        for _ in 0..4 {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let hit = captured_thread.hits.fetch_add(1, Ordering::SeqCst);
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .expect("set read timeout");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        request.extend_from_slice(&chunk[..read]);
                        if request.windows(4).any(|window| window == b"\r\n\r\n") {
                            break;
                        }
                    }
                }
            }
            if hit == 0 {
                *captured_thread.first.lock().expect("store first request") =
                    String::from_utf8_lossy(&request).into_owned();
            }
            let body = r#"{"ok":true}"#;
            let _ = write!(
                stream,
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        }
    });
    (format!("http://{address}/once"), captured)
}

async fn fail_send_activity(bus: &CommandBus) {
    sqlx::query(
        "CREATE TRIGGER fail_send_activity BEFORE INSERT ON activity_events WHEN NEW.action = 'api.send_request' BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;",
    )
    .execute(bus.db.pool())
    .await
    .unwrap();
}

fn secret_header() -> KeyValue {
    KeyValue {
        key: "Authorization".into(),
        value: "Bearer activity-secret-canary".into(),
        enabled: true,
    }
}

fn assert_no_sensitive_leak(visible: &str) {
    for leaked in [
        "activity-secret-canary",
        "original-secret-canary",
        "post-secret-canary",
        "audit sink unavailable",
    ] {
        assert!(
            !visible.contains(leaked),
            "sensitive activity failure leaked: {leaked}"
        );
    }
}

async fn send_activity_count(bus: &CommandBus) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM activity_events WHERE action = 'api.send_request'")
        .fetch_one(bus.db.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn history_and_activity_failures_preserve_the_response_and_both_warnings() {
    for scripted in [false, true] {
        let bus = test_bus().await;
        let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
        sqlx::query("CREATE TRIGGER reject_history BEFORE INSERT ON api_history BEGIN SELECT RAISE(FAIL, 'history unavailable'); END")
            .execute(bus.db.pool()).await.unwrap();
        fail_send_activity(&bus).await;
        let (url, captured) = spawn_counting_api_server();
        let mut input = api_script_test_input(workspace, url);
        input.method = "POST".into();
        let response = if scripted {
            let execution = bus.send_api_request_with_scripts(input).await.unwrap();
            assert_eq!(
                execution.warnings,
                ["API_HISTORY_RECORD_FAILED", "ACTIVITY_RECORD_FAILED"]
            );
            assert!(execution.http_error.is_none());
            execution.response.unwrap()
        } else {
            bus.send_api_request(input).await.unwrap()
        };
        assert_eq!(response.status, 201);
        assert!(response.history_id.is_empty());
        assert_eq!(
            response.warnings,
            ["API_HISTORY_RECORD_FAILED", "ACTIVITY_RECORD_FAILED"]
        );
        assert_eq!(captured.hits.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn post_script_environment_persistence_failure_retains_http_response() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Post failure".into())
        .await
        .unwrap();
    bus.workspace_environment_set_active(workspace.clone(), Some(environment.id.clone()))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_post_variable BEFORE INSERT ON workspace_environment_variables BEGIN SELECT RAISE(FAIL, 'sensitive-storage-canary'); END")
        .execute(bus.db.pool()).await.unwrap();
    let (url, captured) = spawn_counting_api_server();
    let mut input = api_script_test_input(workspace.clone(), url);
    input.method = "POST".into();
    input.post_response_script =
        Some("pm.environment.set('after', 'done'); console.log('post ran');".into());
    let execution = bus
        .send_api_request_with_scripts(input)
        .await
        .expect("retain received response");
    assert_eq!(execution.response.as_ref().unwrap().status, 201);
    assert!(execution.http_error.is_none());
    assert_eq!(
        execution.post_response.status,
        ScriptExecutionStatus::Failed
    );
    assert_eq!(
        execution.post_response.error.as_ref().unwrap().code,
        "API_POST_RESPONSE_FINALIZATION_FAILED"
    );
    assert!(!execution.post_response.console.is_empty());
    assert!(!serde_json::to_string(&execution)
        .unwrap()
        .contains("sensitive-storage-canary"));
    assert_eq!(captured.hits.load(Ordering::SeqCst), 1);
    assert_eq!(
        bus.list_api_history(workspace, None).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn cancellation_after_http_completion_keeps_response_and_marks_post_phase() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let (url, captured) = spawn_counting_api_server();
    let mut input = api_script_test_input(workspace, url);
    input.method = "POST".into();
    input.post_response_script = Some("while (true) {}".into());
    let runner = bus.clone();
    let task = tokio::spawn(async move {
        runner
            .send_api_request_with_scripts_controlled("cancel-post", input)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_history")
                .fetch_one(bus.db.pool())
                .await
                .unwrap();
            if count == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(bus.cancel_api_request("cancel-post"));
    let execution = task.await.unwrap().unwrap();
    assert_eq!(execution.response.as_ref().unwrap().status, 201);
    assert!(execution.http_error.is_none());
    assert_eq!(execution.warnings, ["API_POST_RESPONSE_CANCELLED"]);
    assert_eq!(captured.hits.load(Ordering::SeqCst), 1);
    assert_eq!(bus.api_executions.active_count(), 0);
}

#[tokio::test]
async fn cancellation_after_post_environment_commit_preserves_script_result() {
    use std::{future::Future, pin::Pin, task::Poll, time::Duration};
    use tokio::sync::Notify;
    use unfour_core::domain::{CommandContext, DomainMutation};

    struct PauseBeforeCommit {
        ready: Notify,
        resume: Notify,
    }

    impl TransactionalCommandHook for PauseBeforeCommit {
        fn on_mutations<'a>(
            &'a self,
            _: &'a mut sqlx::SqliteConnection,
            _: &'a CommandContext,
            _: &'a [DomainMutation],
        ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
            Box::pin(async move {
                self.ready.notify_one();
                self.resume.notified().await;
                Ok(())
            })
        }
    }

    let mut bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Post commit cancellation".into())
        .await
        .unwrap();
    bus.workspace_environment_set_active(workspace.clone(), Some(environment.id.clone()))
        .await
        .unwrap();
    let pause = Arc::new(PauseBeforeCommit {
        ready: Notify::new(),
        resume: Notify::new(),
    });
    bus.extensions = CommandBusExtensions::new(vec![pause.clone()]);
    let (url, captured) = spawn_counting_api_server();
    let mut input = api_script_test_input(workspace, url);
    input.post_response_script = Some(
        "pm.environment.set('after', 'committed'); console.log('post completed'); pm.test('response received', () => pm.expect(pm.response.code).to.equal(201));".into(),
    );
    let execution = bus.send_api_request_with_scripts_controlled("cancel-post-commit", input);
    tokio::pin!(execution);
    tokio::select! {
        _ = pause.ready.notified() => {},
        result = &mut execution => panic!("post commit was not reached: {result:?}"),
        _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("post commit did not start"),
    }

    // Queue ahead of post-commit cleanup on the ephemeral DB's only connection.
    // Holding that connection after COMMIT gives cancellation a deterministic
    // window where the environment is durable but cleanup is still pending.
    let connection = bus.db.pool().acquire();
    tokio::pin!(connection);
    std::future::poll_fn(|cx| {
        assert!(connection.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    pause.resume.notify_one();
    let mut connection = tokio::select! {
        connection = &mut connection => connection.unwrap(),
        result = &mut execution => panic!("cleanup must wait for the held connection: {result:?}"),
        _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("environment transaction did not commit"),
    };
    let value: String = sqlx::query_scalar(
        "SELECT value FROM workspace_environment_variables WHERE environment_id = ? AND key = 'after'",
    )
    .bind(&environment.id)
    .fetch_one(&mut *connection)
    .await
    .unwrap();
    assert_eq!(value, "committed");
    assert!(bus.cancel_api_request("cancel-post-commit"));
    std::future::poll_fn(|cx| {
        assert!(execution.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(connection);
    let result = tokio::time::timeout(Duration::from_secs(5), execution)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.response.as_ref().unwrap().status, 201);
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Success);
    assert!(!result.post_response.console.is_empty());
    assert_eq!(result.post_response.tests.len(), 1);
    assert!(result.warnings.is_empty());
    assert_eq!(captured.hits.load(Ordering::SeqCst), 1);
    assert_eq!(bus.api_executions.active_count(), 0);
}

#[tokio::test]
async fn plain_send_keeps_response_and_history_when_activity_record_fails() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let (url, captured) = spawn_counting_api_server();
    let mut input = api_script_test_input(workspace.clone(), url);
    input.method = "POST".into();
    input.headers = vec![secret_header()];
    input.body = Some(r#"{"name":"kept"}"#.into());
    input.body_kind = "json".into();
    fail_send_activity(&bus).await;

    let response = bus.send_api_request(input).await.expect("completed send");
    assert_eq!(captured.hits.load(Ordering::SeqCst), 1);
    assert!(captured
        .first
        .lock()
        .unwrap()
        .contains("Bearer activity-secret-canary"));
    assert_eq!(response.status, 201);
    assert!(!response.history_id.is_empty());
    assert_eq!(response.warnings, ["ACTIVITY_RECORD_FAILED"]);
    let mut serialized = serde_json::to_value(&response).unwrap();
    assert_eq!(serialized["warnings"], json!(["ACTIVITY_RECORD_FAILED"]));
    assert_eq!(serialized["historyId"], response.history_id);
    serialized.as_object_mut().unwrap().remove("warnings");
    let legacy: ApiResponse = serde_json::from_value(serialized).unwrap();
    assert!(legacy.warnings.is_empty());
    let history = bus
        .api_history_detail(workspace, response.history_id.clone())
        .await
        .unwrap();
    assert_eq!(history.id, response.history_id);
    assert_eq!(send_activity_count(&bus).await, 0);
    assert_no_sensitive_leak(&serde_json::to_string(&(response, history)).unwrap());
}

#[tokio::test]
async fn scripted_send_keeps_response_history_and_environment_when_activity_record_fails() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Activity".into())
        .await
        .unwrap();
    bus.workspace_environment_set_active(workspace.clone(), Some(environment.id.clone()))
        .await
        .unwrap();
    bus.workspace_environment_variable_create(
        workspace.clone(),
        environment.id.clone(),
        WorkspaceVariableInput {
            id: None,
            key: "token".into(),
            value: "original-secret-canary".into(),
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
        },
    )
    .await
    .unwrap();
    let (url, captured) = spawn_counting_api_server();
    let mut input = api_script_test_input(workspace.clone(), url);
    input.headers = vec![secret_header()];
    input.pre_request_script = Some(
        r#"pm.environment.set("token", "activity-secret-canary"); pm.environment.set("trace", "kept-marker");"#
            .into(),
    );
    input.post_response_script = Some(r#"pm.environment.set("after", "post-marker");"#.into());
    let saved = bus.save_api_request(input.clone()).await.unwrap();
    fail_send_activity(&bus).await;

    let direct = bus
        .send_api_request_with_scripts(input)
        .await
        .expect("scripted send completed");
    let replay = bus
        .execute_saved_api_request_with_scripts_in_workspace(
            Some(workspace.clone()),
            &saved.id,
            None,
            None,
        )
        .await
        .expect("saved replay completed");

    assert_eq!(captured.hits.load(Ordering::SeqCst), 2);
    assert!(captured
        .first
        .lock()
        .unwrap()
        .contains("Bearer activity-secret-canary"));
    assert_eq!(direct.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert_eq!(replay.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert!(direct.http_error.is_none());
    assert_eq!(direct.pre_request.status, ScriptExecutionStatus::Success);
    assert_eq!(direct.post_response.status, ScriptExecutionStatus::Success);
    let response = direct.response.as_ref().expect("http response retained");
    let replay_response = replay
        .response
        .as_ref()
        .expect("saved replay retains http response");
    assert_eq!(response.status, 201);
    assert_eq!(response.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert_eq!(replay_response.warnings, ["ACTIVITY_RECORD_FAILED"]);
    assert_ne!(
        response.history_id, replay_response.history_id,
        "each completed send keeps its own history id"
    );
    let environments = bus
        .workspace_environments_list(workspace.clone())
        .await
        .unwrap();
    let persisted = environments
        .iter()
        .find(|item| item.id == environment.id)
        .unwrap();
    let value = |key: &str| {
        persisted
            .variables
            .iter()
            .find(|variable| variable.key == key)
            .map(|variable| variable.value.as_str())
    };
    assert_eq!(value("token"), Some("activity-secret-canary"));
    assert_eq!(value("trace"), Some("kept-marker"));
    assert_eq!(value("after"), Some("post-marker"));
    assert_eq!(send_activity_count(&bus).await, 0);
    let history = bus
        .list_api_history(workspace.clone(), Some(10))
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    let detail = bus
        .api_history_detail(workspace, response.history_id.clone())
        .await
        .unwrap();
    assert_no_sensitive_leak(&serde_json::to_string(&(direct, detail)).unwrap());
}

#[tokio::test]
async fn failed_post_script_keeps_http_result_when_activity_record_fails() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Post failure".into())
        .await
        .unwrap();
    bus.workspace_environment_set_active(workspace.clone(), Some(environment.id.clone()))
        .await
        .unwrap();
    let (url, captured) = spawn_counting_api_server();
    let mut input = api_script_test_input(workspace.clone(), url);
    input.headers = vec![secret_header()];
    input.pre_request_script =
        Some(r#"pm.environment.set("token", "activity-secret-canary");"#.into());
    input.post_response_script = Some(
        r#"pm.environment.set("token", "post-secret-canary"); pm.environment.set("partial", "rolled-back"); throw new Error("post failed");"#.into(),
    );
    fail_send_activity(&bus).await;

    let result = bus
        .send_api_request_with_scripts(input)
        .await
        .expect("post-script failure is not a lost send");
    assert_eq!(captured.hits.load(Ordering::SeqCst), 1);
    assert_eq!(result.pre_request.status, ScriptExecutionStatus::Success);
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Failed);
    assert!(result.http_error.is_none());
    assert_eq!(result.warnings, ["ACTIVITY_RECORD_FAILED"]);
    let response = result.response.as_ref().expect("completed http response");
    assert_eq!(response.status, 201);
    assert!(!response.history_id.is_empty());
    let environments = bus
        .workspace_environments_list(workspace.clone())
        .await
        .unwrap();
    let persisted = environments
        .iter()
        .find(|item| item.id == environment.id)
        .unwrap();
    assert_eq!(persisted.variables.len(), 1);
    assert_eq!(persisted.variables[0].key, "token");
    assert_eq!(persisted.variables[0].value, "activity-secret-canary");
    assert_eq!(send_activity_count(&bus).await, 0);
    let detail = bus
        .api_history_detail(workspace, response.history_id.clone())
        .await
        .unwrap();
    assert_no_sensitive_leak(&serde_json::to_string(&(result, detail)).unwrap());
}

#[tokio::test]
async fn incomplete_api_operations_are_not_reported_as_activity_success() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let (url, captured) = spawn_counting_api_server();
    fail_send_activity(&bus).await;

    let mut transport = api_script_test_input(workspace.clone(), "http://127.0.0.1:1/down".into());
    transport.headers = vec![secret_header()];
    let error = bus.send_api_request(transport.clone()).await.unwrap_err();
    let error = error.to_string();
    assert!(!error.contains("ACTIVITY_RECORD_FAILED"));
    assert!(!error.contains("audit sink unavailable"));
    assert!(!error.contains("activity-secret-canary"));

    let scripted_transport = bus
        .send_api_request_with_scripts(transport)
        .await
        .expect("transport failure stays a typed http result");
    assert!(scripted_transport.response.is_none());
    assert!(scripted_transport.http_error.is_some());
    assert!(scripted_transport.warnings.is_empty());
    assert_ne!(
        scripted_transport.http_error_code.as_deref(),
        Some("ACTIVITY_RECORD_FAILED")
    );

    let mut script = api_script_test_input(workspace.clone(), url);
    script.headers = vec![secret_header()];
    script.pre_request_script = Some(r#"throw new Error("pre failed");"#.into());
    let failed_script = bus.send_api_request_with_scripts(script).await.unwrap();
    assert!(failed_script.response.is_none());
    assert!(failed_script.http_error.is_none());
    assert!(failed_script.warnings.is_empty());
    assert_eq!(
        failed_script.pre_request.status,
        ScriptExecutionStatus::Failed
    );
    assert_eq!(captured.hits.load(Ordering::SeqCst), 0);
    assert_eq!(send_activity_count(&bus).await, 0);
    let history_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_history")
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    assert_eq!(history_count, 0);
    assert_no_sensitive_leak(&serde_json::to_string(&failed_script).unwrap());
    assert_no_sensitive_leak(&error);
}
