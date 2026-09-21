use super::flow::*;
use super::*;
use serde_json::json;
use unfour_core::models::{FlowRunStatus, FlowStepRunStatus};

#[tokio::test]
async fn flow_canonical_multipart_is_rejected_before_execution() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = api_script_test_input(workspace.clone(), "http://127.0.0.1:1/never".into());
    input.body_kind = unfour_core::models::MULTIPART_BODY_KIND.into();
    input.body = Some("[]".into());
    let api = bus.save_api_request(input).await.unwrap();
    for probe in [false, true] {
        let node = if probe {
            json!({"id":"probe","name":"probe","kind":"poll","timeoutMs":1000,"intervalMs":10,"maxAttempts":1,
                "probe":{"capability":"api","resourceId":api.id,"arguments":{}},
                "predicate":{"left":true,"op":"eq","right":true}})
        } else {
            action("request", "api", &api.id, json!({}))
        };
        let flow = bus
            .save_flow(definition(&workspace, json!([node])))
            .await
            .unwrap();
        let result = bus.run_flow(request(&workspace, &flow.id)).await.unwrap();
        assert_eq!(result.status, FlowRunStatus::ValidationFailed);
        assert_eq!(
            result.error.as_deref(),
            Some("FLOW_API_SCRIPT_OR_MULTIPART_UNSUPPORTED")
        );
        assert!(result.steps.iter().all(|step| step.attempts.is_empty()));
    }
}

#[tokio::test]
async fn flow_manual_secrets_validate_runtime_names_before_declared_secret_union() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = unfour_flow_engine::FlowService::new(bus.db.clone());
    let mut flow = definition(
        &workspace,
        json!([action("echo", "api", "echo", json!({}))]),
    );
    flow.inputs = serde_json::from_value(json!([
        {"name":"declared","type":"string","secret":true},
        {"name":"optional","type":"string","secret":true},
        {"name":"defaulted","type":"number","default":5}
    ]))
    .unwrap();
    let flow = service.save(flow).await.unwrap();
    for names in [
        vec!["extra", "defaulted"],
        vec!["typo"],
        vec!["optional"],
        vec!["clientRfe"],
    ] {
        let mut input = request(&workspace, &flow.id);
        input.inputs = json!({"declared":"fixture-declared-secret", "extra":"fixture-extra-secret", "clientRef":"innocuous-private-value"});
        input.secret_input_names = names.iter().map(|name| (*name).into()).collect();
        let driver = std::sync::Arc::new(Driver::default());
        let run = service.run(input, driver.clone()).await.unwrap();
        let result = finished(&bus, &run).await;
        if names[0] == "extra" {
            assert_eq!(result.status, FlowRunStatus::Succeeded);
            for name in ["extra", "defaulted", "declared", "optional"] {
                assert!(result.context.secret_input_names.contains(&name.into()));
            }
        } else {
            assert_eq!(result.status, FlowRunStatus::ValidationFailed);
            assert_eq!(result.error.as_deref(), Some("FLOW_UNKNOWN_SECRET_INPUT"));
            assert!(driver.calls.lock().unwrap().is_empty());
            assert_eq!(result.context.inputs, json!({}));
            let persisted = service.get_run(&workspace, &result.id).await.unwrap();
            assert!(!serde_json::to_string(&persisted)
                .unwrap()
                .contains("innocuous-private-value"));
        }
        let stored = serde_json::to_string(&result).unwrap();
        assert!(!stored.contains("fixture-declared-secret"));
        if names[0] == "extra" {
            assert!(!stored.contains("fixture-extra-secret"));
        }
    }
}

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
    assert_eq!(result.status, FlowRunStatus::Succeeded);
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
    assert_eq!(result.status, FlowRunStatus::Cancelled);
    assert_eq!(result.steps[1].status, FlowStepRunStatus::Skipped);
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
    assert_eq!(result.status, FlowRunStatus::Interrupted);
    assert_eq!(result.steps[0].status, FlowStepRunStatus::Interrupted);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(
        bus.get_flow_run(workspace, run.id).await.unwrap().status,
        FlowRunStatus::Interrupted
    );
}

#[tokio::test]
async fn flow_legacy_storage_reads_without_rewriting_historical_snapshots() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let service = unfour_flow_engine::FlowService::new(bus.db.clone());
    let flow = service
        .save(definition(
            &workspace,
            json!([action(
                "echo",
                "api",
                "echo",
                json!({"value":{"$ref":"/inputs/value"}})
            )]),
        ))
        .await
        .unwrap();
    let run = service
        .run(
            request(&workspace, &flow.id),
            std::sync::Arc::new(Driver::default()),
        )
        .await
        .unwrap();
    let result = finished(&bus, &run).await;
    let mut legacy = serde_json::to_value(result).unwrap();
    legacy["definition"]["inputs"] = json!(["value"]);
    for step in legacy["steps"].as_array_mut().unwrap() {
        let step = step.as_object_mut().unwrap();
        for key in ["startedAt", "nextCheckAt", "output"] {
            step.remove(key);
        }
    }
    let legacy_json = serde_json::to_string(&legacy).unwrap();
    sqlx::query("UPDATE flow_runs SET run_json = ? WHERE id = ?")
        .bind(&legacy_json)
        .bind(&run.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    let mut legacy_definition = serde_json::to_value(&flow).unwrap();
    legacy_definition["inputs"] = json!(["value"]);
    sqlx::query("UPDATE flow_definitions SET definition_json = ? WHERE id = ?")
        .bind(legacy_definition.to_string())
        .bind(&flow.id)
        .execute(bus.db.pool())
        .await
        .unwrap();
    let mut loaded = service.get(&workspace, &flow.id).await.unwrap();
    assert_eq!(
        loaded.inputs[0].input_type,
        unfour_core::models::FlowInputType::Json
    );
    loaded.name = "new revision".into();
    service.save(loaded).await.unwrap();
    let history = service.get_run(&workspace, &run.id).await.unwrap();
    assert_eq!(history.definition.revision, 1);
    assert_eq!(history.definition.name, "test");
    assert_eq!(history.status, FlowRunStatus::Succeeded);
    assert!(history.steps[0].output.is_none());
    let unchanged: String = sqlx::query_scalar("SELECT run_json FROM flow_runs WHERE id = ?")
        .bind(&run.id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    assert_eq!(unchanged, legacy_json);
}
