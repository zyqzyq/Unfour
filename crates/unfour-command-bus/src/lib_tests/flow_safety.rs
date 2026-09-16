use super::flow::*;
use super::*;
use serde_json::json;

#[tokio::test]
async fn flow_saved_auth_is_materialized_without_a_ui_and_explicit_headers_win() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for (auth, expected) in [
        (json!({"type":"bearer","token":"fixture"}), "Bearer fixture"),
        (
            json!({"type":"basic","username":"test","password":"test"}),
            "Basic dGVzdDp0ZXN0",
        ),
    ] {
        let mut input = api_script_test_input(workspace.clone(), "http://localhost/".into());
        input.auth_json = Some(auth.to_string());
        let mut resolved = bus.api_client.materialize_auth(input).unwrap();
        assert_eq!(resolved.headers[0].value, expected);
        resolved.headers[0].value = "explicit".into();
        let resolved = bus.api_client.materialize_auth(resolved).unwrap();
        assert_eq!(resolved.headers.len(), 1);
        assert_eq!(resolved.headers[0].value, "explicit");
    }
    let mut input = api_script_test_input(workspace, "http://localhost/".into());
    input.auth_json = Some(
        json!({"type":"api-key","addTo":"query","key":"api_key","value":"fixture"}).to_string(),
    );
    assert_eq!(
        bus.api_client.materialize_auth(input).unwrap().query[0].key,
        "api_key"
    );
    let mut snapshot = json!({"authJson":json!({"type":"api-key","key":"custom-key","value":"fixture-auth-never-store"}).to_string()});
    unfour_flow_engine::expression::redact(&mut snapshot);
    assert!(!snapshot.to_string().contains("fixture-auth-never-store"));
}

#[tokio::test]
async fn flow_sensitive_inputs_are_usable_but_never_persisted() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = unfour_flow_engine::FlowService::new(bus.db.clone());
    let flow = service
        .save(definition(
            &workspace,
            json!([
                action(
                    "first",
                    "api",
                    "a",
                    json!({"password":{"$ref":"/inputs/password"}, "copied":"${/inputs/custom}", "nested":{"$ref":"/inputs/nested"}})
                ),
                action(
                    "second",
                    "api",
                    "b",
                    json!({"echo":{"$ref":"/steps/first/password"}})
                )
            ]),
        ))
        .await
        .unwrap();
    let mut input = request(&workspace, &flow.id);
    input.inputs = json!({"value":42,"password":"fixture-password-never-store","custom":"fixture-custom-never-store", "nested":{"credential":"fixture-nested-never-store","pin":789123}});
    input.secret_input_names = vec!["custom".into(), "nested".into()];
    let driver = std::sync::Arc::new(Driver::default());
    let run = service.run(input, driver.clone()).await.unwrap();
    let result = finished(&bus, &run).await;
    assert_eq!(result.status, "succeeded");
    assert_eq!(
        driver.calls.lock().unwrap()[1]["echo"],
        "fixture-password-never-store"
    );
    let stored: String = sqlx::query_scalar("SELECT run_json FROM flow_runs WHERE id = ?")
        .bind(&run.id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    assert!(!stored.contains("fixture-password-never-store"));
    assert!(!stored.contains("fixture-custom-never-store"));
    assert!(!stored.contains("fixture-nested-never-store"));
    assert!(!stored.contains("789123"));
    assert_eq!(result.id, run.id);
    let unsafe_definition = definition(
        &workspace,
        json!([action(
            "unsafe",
            "api",
            "a",
            json!({"headers":[{"key":"Authorization","value":"Bearer literal-fixture"}]})
        )]),
    );
    assert!(service.save(unsafe_definition).await.is_err());
}

#[tokio::test]
async fn flow_cancel_reaches_active_http_and_never_schedules_later_steps() {
    use std::io::{BufRead, BufReader, Read};
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/wait", listener.local_addr().unwrap());
    let (sent, received) = tokio::sync::oneshot::channel();
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
        sent.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(
            socket.read(&mut byte).unwrap(),
            0,
            "cancel must close the active HTTP request"
        );
    });
    let api = bus
        .save_api_request(api_script_test_input(workspace.clone(), url))
        .await
        .unwrap();
    let flow = bus
        .save_flow(definition(
            &workspace,
            json!([
                action("first", "api", &api.id, json!({})),
                action("later", "api", &api.id, json!({}))
            ]),
        ))
        .await
        .unwrap();
    let run = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), received)
        .await
        .unwrap()
        .unwrap();
    bus.cancel_flow_run(workspace, run.id.clone())
        .await
        .unwrap();
    let result = finished(&bus, &run).await;
    assert_eq!(result.status, "cancelled");
    assert_eq!(result.steps[1].status, "skipped");
    server.join().unwrap();
}

#[tokio::test]
async fn flow_stale_run_recovers_as_interrupted_without_replaying() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let flow = bus
        .save_flow(definition(
            &workspace,
            json!([{"id":"wait","name":"wait","kind":"wait","durationMs":1000,"timeoutMs":2000}]),
        ))
        .await
        .unwrap();
    let run = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
    // Simulate a process which stopped heartbeating. No resource side effects.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    sqlx::query("UPDATE flow_runs SET updated_at = '2000-01-01T00:00:00+00:00' WHERE id = ?")
        .bind(&run.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    let result = bus
        .get_flow_run(workspace.clone(), run.id.clone())
        .await
        .unwrap();
    assert_eq!(result.status, "interrupted");
    assert_eq!(result.steps[0].status, "interrupted");
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(
        bus.get_flow_run(workspace, run.id).await.unwrap().status,
        "interrupted"
    );
}
