use super::*;

#[derive(Clone, Default)]
struct CapturedDiagnostics(std::sync::Arc<std::sync::Mutex<String>>);

impl tracing::field::Visit for CapturedDiagnostics {
    fn record_debug(&mut self, _: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        write!(self.0.lock().unwrap(), "{value:?}\n").unwrap();
    }
}

impl tracing::Subscriber for CapturedDiagnostics {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        values.record(&mut self.clone());
    }
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        event.record(&mut self.clone());
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

fn variable(key: &str, value: &str, secret: bool, enabled: bool) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id: None,
        key: key.into(),
        value: value.into(),
        is_secret: secret,
        is_enabled: enabled,
        description: None,
        sort_order: 0,
    }
}

#[tokio::test]
async fn secret_projection_scripted_method_validation_keeps_errors_safe() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let sentinel = format!("mcp-method-{}", unfour_core::id::new_id());
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Method".into())
        .await
        .unwrap();
    bus.workspace_environment_variable_create(
        workspace.clone(),
        environment.id.clone(),
        variable("ordinary", &sentinel, true, true),
    )
    .await
    .unwrap();
    let mut input = api_script_test_input(workspace, "http://127.0.0.1:1/".into());
    input.pre_request_script =
        Some("pm.request.method = pm.environment.get('ordinary') + ' invalid';".into());
    let result = bus
        .send_api_request_with_scripts_in_environment(input, Some(environment.id))
        .await
        .unwrap();
    assert!(result.response.is_none());
    assert!(result
        .http_error
        .as_ref()
        .unwrap()
        .contains("invalid HTTP method"));
    assert!(
        !serde_json::to_string(&result).unwrap().contains(&sentinel),
        "secret method leaked through validation error"
    );
}

#[tokio::test]
async fn secret_projection_scripted_method_is_hidden_in_history_and_activity() {
    let diagnostics = CapturedDiagnostics::default();
    let _subscriber = tracing::subscriber::set_default(diagnostics.clone());
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let sentinel = format!("MCP-METHOD-{}", unfour_core::id::new_id()).to_uppercase();
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Method".into())
        .await
        .unwrap();
    bus.workspace_environment_variable_create(
        workspace.clone(),
        environment.id.clone(),
        variable("ordinary", &sentinel, true, true),
    )
    .await
    .unwrap();
    let (url, receiver) = spawn_api_test_server();
    let mut input = api_script_test_input(workspace.clone(), url);
    input.pre_request_script = Some("pm.request.method = pm.environment.get('ordinary');".into());
    let result = bus
        .send_api_request_with_scripts_in_environment(input, Some(environment.id))
        .await
        .unwrap();
    let wire = receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert!(wire.starts_with(&sentinel));
    let response = result.response.unwrap();
    let history = bus
        .api_history_detail(workspace.clone(), response.history_id)
        .await
        .unwrap();
    let activity: Vec<String> =
        sqlx::query_scalar("SELECT details_json FROM activity_events WHERE workspace_id=?")
            .bind(&workspace)
            .fetch_all(bus.db.pool())
            .await
            .unwrap();
    let observable =
        serde_json::to_string(&(response.request_snapshot, history, activity)).unwrap();
    assert!(
        !observable.contains(&sentinel),
        "secret method leaked through snapshot/history/activity"
    );
    let logs = diagnostics.0.lock().unwrap();
    assert!(logs.contains("api_request_completed"));
    assert!(
        !logs.contains(&sentinel),
        "secret method leaked through diagnostics"
    );
}

#[tokio::test]
async fn secret_projection_multipart_errors_hide_resolved_field_names() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let sentinel = format!("mcp-field-{}", unfour_core::id::new_id());
    bus.workspace_variable_create(
        workspace.clone(),
        variable("ordinary", &sentinel, true, true),
    )
    .await
    .unwrap();
    let mut input = api_script_test_input(workspace.clone(), "http://127.0.0.1:1/".into());
    input.method = "POST".into();
    input.body_kind = "multipart-form-data".into();
    input.body =
        Some(r#"[{"id":"file","enabled":true,"key":"{{ordinary}}","type":"file"}]"#.into());
    let direct = bus.send_api_request(input.clone()).await.unwrap_err();
    let saved = bus.save_api_request(input).await.unwrap();
    let scripted = bus
        .execute_saved_api_request_with_scripts_in_workspace(
            Some(workspace.clone()),
            &saved.id,
            None,
            None,
        )
        .await
        .unwrap();
    assert!(direct
        .to_string()
        .contains("requires a local file selection"));
    assert!(scripted.response.is_none());
    let observable = format!("{}{}", direct, serde_json::to_string(&scripted).unwrap());
    assert!(
        !observable.contains(&sentinel),
        "resolved secret field name leaked through an execution error"
    );
    let history = bus.list_api_history(workspace.clone(), None).await.unwrap();
    let activity: Vec<String> =
        sqlx::query_scalar("SELECT details_json FROM activity_events WHERE workspace_id=?")
            .bind(workspace)
            .fetch_all(bus.db.pool())
            .await
            .unwrap();
    assert!(history.is_empty());
    assert!(!activity.join("").contains(&sentinel));
}

#[tokio::test]
async fn secret_projection_execution_keeps_effective_credentials_and_safe_history() {
    use std::io::{Read, Write};

    let diagnostics = CapturedDiagnostics::default();
    let _subscriber = tracing::subscriber::set_default(diagnostics.clone());

    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let workspace_secret = format!("mcp-workspace-{}", unfour_core::id::new_id());
    let environment_secret = format!("mcp-environment-{}", unfour_core::id::new_id());
    let temporary_secret = format!("mcp-temporary-{}", unfour_core::id::new_id());
    let disabled_secret = format!("mcp-disabled-{}", unfour_core::id::new_id());
    for entry in [
        variable("ordinary", &workspace_secret, true, true),
        variable("page", "visible-public-value", false, true),
        variable("fallback", "workspace-fallback", false, true),
    ] {
        bus.workspace_variable_create(workspace.clone(), entry)
            .await
            .unwrap();
    }
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Projection".into())
        .await
        .unwrap();
    bus.workspace_environment_variables_replace(
        workspace.clone(),
        environment.id.clone(),
        vec![
            variable("ordinary", &environment_secret, true, true),
            variable("fallback", &disabled_secret, true, false),
        ],
    )
    .await
    .unwrap();

    for temporary in [false, true] {
        let effective = if temporary {
            &temporary_secret
        } else {
            &environment_secret
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/echo", listener.local_addr().unwrap());
        let response_secret = effective.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let size = stream.read(&mut chunk).unwrap();
                if size == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..size]);
            }
            sender
                .send(String::from_utf8_lossy(&request).into_owned())
                .unwrap();
            let body = serde_json::json!({"echo": response_secret, "page": "visible-public-value"})
                .to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Ordinary: {response_secret}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let mut input = api_script_test_input(workspace.clone(), url);
        input.auth_json = Some(r#"{"type":"bearer","token":"{{ordinary}}"}"#.into());
        input.headers = vec![
            KeyValue {
                key: "X-Ordinary".into(),
                value: "{{ordinary}}".into(),
                enabled: true,
            },
            KeyValue {
                key: "X-Public".into(),
                value: "{{page}}".into(),
                enabled: true,
            },
            KeyValue {
                key: "X-Fallback".into(),
                value: "{{fallback}}".into(),
                enabled: true,
            },
        ];
        if temporary {
            input.temporary_variables.push(KeyValue {
                key: "ordinary".into(),
                value: temporary_secret.clone(),
                enabled: true,
            });
        }
        let response = if temporary {
            input.post_response_script = Some(format!(
                "pm.test('raw response', () => pm.expect(pm.response.json().echo).to.equal('{effective}'));"
            ));
            let result = bus
                .send_api_request_with_scripts_in_environment(input, Some(environment.id.clone()))
                .await
                .unwrap();
            assert_eq!(result.post_response.status, ScriptExecutionStatus::Success);
            assert!(
                result.post_response.tests[0].passed,
                "post scripts must see raw response credentials"
            );
            result.response.unwrap()
        } else {
            bus.send_api_request_in_environment(input, Some(environment.id.clone()))
                .await
                .unwrap()
        };
        let wire = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        server.join().unwrap();
        assert!(wire.contains(&format!("Bearer {effective}")));
        assert!(wire.contains("visible-public-value"));
        assert!(wire.contains("workspace-fallback"));
        assert!(!wire.contains(&workspace_secret));
        assert!(!wire.contains(&disabled_secret));
        // Trusted runtime consumers still receive the actual body for post scripts.
        assert!(response.body.contains(effective));
        let diagnostic = response.clone().into_diagnostic_projection();
        let diagnostic_json = serde_json::to_string(&diagnostic).unwrap();
        assert!(!diagnostic_json.contains(effective));
        assert!(diagnostic_json.contains("visible-public-value"));
        assert!(!diagnostic_json.contains("diagnosticProjection"));
        let history = bus
            .api_history_detail(workspace.clone(), response.history_id.clone())
            .await
            .unwrap();
        let persisted: (String, String, String, String) = sqlx::query_as(
            "SELECT url, request_headers_json, response_headers_json, response_body_preview FROM api_history WHERE id=?",
        ).bind(&response.history_id).fetch_one(bus.db.pool()).await.unwrap();
        let activity: Vec<String> =
            sqlx::query_scalar("SELECT details_json FROM activity_events WHERE workspace_id=?")
                .bind(&workspace)
                .fetch_all(bus.db.pool())
                .await
                .unwrap();
        let observable =
            serde_json::to_string(&(response.request_snapshot, history, persisted, activity))
                .unwrap();
        for sentinel in [
            &workspace_secret,
            &environment_secret,
            &temporary_secret,
            &disabled_secret,
        ] {
            assert!(
                !observable.contains(sentinel),
                "secret leaked in snapshot/history/activity"
            );
        }
        assert!(observable.contains("visible-public-value"));
    }
    let logs = diagnostics.0.lock().unwrap();
    assert!(logs.contains("api_request_completed"));
    for sentinel in [
        &workspace_secret,
        &environment_secret,
        &temporary_secret,
        &disabled_secret,
    ] {
        assert!(
            !logs.contains(sentinel),
            "secret leaked through diagnostics"
        );
    }
}
