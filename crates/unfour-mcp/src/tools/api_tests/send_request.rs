use super::*;

// --- send_request tests ---

#[test]
fn send_request_returns_success_with_redacted_response() {
    let result = api_registry()
        .call("unfour.api.send_request", json!({ "requestId": "req-1" }))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let content = &result["structuredContent"];
    assert_eq!(content["ok"], true);
    assert_eq!(content["status"], 200);
    assert_eq!(content["statusText"], "OK");
    assert_eq!(content["durationMs"], 123);
    assert_eq!(content["source"], "command-bus");

    // Set-Cookie response header masked
    let headers = content["headers"].as_array().unwrap();
    let set_cookie = headers.iter().find(|h| h["name"] == "Set-Cookie").unwrap();
    assert!(set_cookie["value"].as_str().unwrap().starts_with("[mask "));

    // Body token masked
    let body = content["bodyPreview"].as_str().unwrap();
    assert!(
        body.contains("[mask "),
        "token should be masked in response body"
    );
    assert!(!body.contains("secret-jwt"), "raw token should not appear");
}

#[test]
fn send_request_replays_saved_request_in_explicit_workspace() {
    let result = api_registry()
        .call(
            "unfour.api.send_request",
            json!({
                "workspaceId": "ws-1",
                "requestId": "req-explicit-workspace"
            }),
        )
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["ok"], true);
}

#[test]
fn send_request_passes_environment_override_to_saved_scripted_replay() {
    let result = api_registry()
        .call(
            "unfour.api.send_request",
            json!({
                "requestId": "req-environment-override",
                "environmentId": "env-test"
            }),
        )
        .expect("saved replay should receive the per-call environment");

    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["ok"], true);
}

#[test]
fn saved_replay_requires_the_explicit_workspace_to_match_its_snapshot() {
    let result = api_registry()
        .call(
            "unfour.api.send_request",
            json!({"requestId": "req-1", "workspaceId": "another-workspace"}),
        )
        .unwrap();

    assert_eq!(result["isError"], true);
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "NOT_FOUND"
    );
}

#[test]
fn send_request_passes_environment_override_to_ad_hoc_send() {
    let result = api_registry()
        .call(
            "unfour.api.send_request",
            json!({
                "method": "POST",
                "url": "https://environment-override.example.test",
                "environmentId": "env-test"
            }),
        )
        .expect("ad-hoc send should receive the per-call environment");

    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["status"], 201);
}

#[test]
fn send_request_allows_dev_post_ad_hoc() {
    let result = api_registry()
        .call(
            "unfour.api.send_request",
            json!({
                "method": "POST",
                "url": "https://api.example.com/users",
                "headers": { "Authorization": "Bearer secret-token" },
                "body": "{\"name\":\"test\"}",
                "bodyKind": "json"
            }),
        )
        .expect("dev POST should be allowed");

    let content = &result["structuredContent"];
    assert_eq!(result["isError"], false);
    crate::response::assert_call_meta(&result, "dev", "medium");
    assert_eq!(content["status"], 201);
    assert!(!result.to_string().contains("secret-token"));
    assert!(!result.to_string().contains("secret-jwt"));
}

#[test]
fn send_request_blocks_prod_delete_ad_hoc() {
    struct ProdApiCommandBus;

    impl CommandBusAdapter for ProdApiCommandBus {
        fn execute_read(
            &self,
            _command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "ws-prod".to_string(),
                    workspace_name: "Prod".to_string(),
                    environment_type: "prod".to_string(),
                    mcp_policy: "auto".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            ))
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            panic!("prod DELETE should be blocked before execution")
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            unreachable!()
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            _connection_id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            unreachable!()
        }

        fn execute_db_query(
            &self,
            _input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            unreachable!()
        }
    }

    let registry = ToolRegistry::with_command_bus(Arc::new(ProdApiCommandBus));
    let result = registry
        .call(
            "unfour.api.send_request",
            json!({ "method": "DELETE", "url": "https://api.example.com/users/1" }),
        )
        .expect("policy denial should be structured");

    assert_eq!(result["isError"], true);
    crate::response::assert_call_meta(&result, "prod", "medium");
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
}

struct SavedGetPolicyBus {
    request_reads: std::sync::atomic::AtomicUsize,
    script: Option<&'static str>,
    environment: &'static str,
}

impl CommandBusAdapter for SavedGetPolicyBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        match ApiStubCommandBus.execute_read(command)? {
            ReadCommandResult::ApiRequest(mut result) => {
                assert_eq!(
                    self.request_reads
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                    0
                );
                result.request.method = "GET".into();
                result.request.url = "https://example.test/catalog".into();
                result.request.revision = 17;
                result.request.post_response_script = self.script.map(str::to_owned);
                Ok(ReadCommandResult::ApiRequest(result))
            }
            ReadCommandResult::Workspaces(mut result) => {
                result.workspaces[0].environment_type = self.environment.into();
                Ok(ReadCommandResult::Workspaces(result))
            }
            _ => panic!("a saved replay resolves its workspace from the saved request"),
        }
    }

    fn execute_saved_api_request_snapshot(
        &self,
        snapshot: &ApiSavedRequest,
        timeout_ms: Option<u64>,
        _environment_id: Option<&str>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        assert_eq!(snapshot.workspace_id, "ws-1");
        assert_eq!(snapshot.method, "GET");
        assert_eq!(snapshot.url, "https://example.test/catalog");
        assert_eq!(snapshot.revision, 17);
        assert_eq!(snapshot.post_response_script.as_deref(), self.script);
        ApiStubCommandBus.execute_saved_api_request(&snapshot.id, timeout_ms)
    }

    fn execute_saved_api_request(
        &self,
        _request_id: &str,
        _timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        panic!("policy-checked replay must not execute by ID");
    }

    fn list_db_connections(
        &self,
        _: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        unreachable!()
    }

    fn get_db_schema(&self, _: &str, _: &str) -> Result<DatabaseSchema, CommandBusAdapterError> {
        unreachable!()
    }

    fn execute_db_query(
        &self,
        _: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        unreachable!()
    }
}

#[test]
fn saved_replay_uses_one_definition_for_policy_and_execution() {
    for (environment, script, risk) in [
        ("prod", None, "low"),
        ("dev", Some("// Inspect the response"), "medium"),
    ] {
        let bus = Arc::new(SavedGetPolicyBus {
            request_reads: Default::default(),
            script,
            environment,
        });
        let result = ToolRegistry::with_command_bus(bus.clone())
            .call(
                "unfour.api.send_request",
                json!({
                    "requestId": "catalog",
                    "method": "POST",
                    "url": "https://example.test/ad-hoc",
                }),
            )
            .unwrap();
        assert_eq!(result["isError"], false);
        crate::response::assert_call_meta(&result, environment, risk);
        assert_eq!(
            bus.request_reads.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}

#[test]
fn send_request_blocks_scripted_saved_get_in_prod() {
    let bus = SavedGetPolicyBus {
        request_reads: Default::default(),
        script: Some("// Inspect the response"),
        environment: "prod",
    };
    let result = ToolRegistry::with_command_bus(Arc::new(bus))
        .call(
            "unfour.api.send_request",
            json!({ "requestId": "scripted-get" }),
        )
        .expect("policy denial should be structured");
    assert_eq!(result["isError"], true);
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
}

#[test]
fn send_request_accepts_timeout_above_the_mcp_default() {
    // Exact forwarding is covered at the parser and command-bus boundary;
    // this verifies the public tool contract accepts the explicit value.
    let result = api_registry()
        .call(
            "unfour.api.send_request",
            json!({ "requestId": "req-1", "timeoutMs": 120000 }),
        )
        .expect("should succeed");
    assert_eq!(result["structuredContent"]["ok"], true);
}

#[test]
fn send_request_rejects_missing_request_id() {
    let result = api_registry().call("unfour.api.send_request", json!({}));
    assert!(result.is_err(), "should fail without requestId");
}

#[test]
fn send_request_validates_optional_request_id_before_selecting_replay() {
    let registry = api_registry();
    let mut args =
        json!({"requestId": "   ", "method": "POST", "url": "https://example.test/catalog"});
    let error = registry
        .call("unfour.api.send_request", args.clone())
        .unwrap_err();
    assert!(
        matches!(error, ToolCallError::InvalidArguments(message) if message.contains("requestId"))
    );

    // An empty optional ID has historically selected the ad-hoc form.
    args["requestId"] = json!("");
    let response = registry.call("unfour.api.send_request", args).unwrap();
    assert_eq!(response["structuredContent"]["status"], 201);
}

#[test]
fn send_request_returns_structured_error_on_failure() {
    let registry = ToolRegistry::with_command_bus(Arc::new(FailingApiCommandBus));
    let result = registry
        .call("unfour.api.send_request", json!({ "requestId": "req-1" }))
        .expect("execution errors become MCP tool results");

    assert_eq!(result["isError"], true);
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "COMMAND_BUS_API_SEND_FAILED"
    );
    assert!(!result.to_string().contains("ACTIVITY_RECORD_FAILED"));
}

#[test]
fn command_bus_read_failure_returns_structured_error() {
    let registry = ToolRegistry::with_command_bus(Arc::new(FailingApiCommandBus));
    let result = registry
        .call("unfour.api.list_collections", json!({}))
        .expect("execution errors become MCP tool results");

    assert_eq!(result["isError"], true);
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "COMMAND_BUS_READ_FAILED"
    );
}

#[test]
fn send_request_keeps_completed_response_when_activity_record_fails() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ActivityWarningBus {
        calls: AtomicUsize,
    }

    fn warned_response() -> ApiResponse {
        ApiResponse {
            warnings: vec!["ACTIVITY_RECORD_FAILED".into()],
            request_snapshot: None,
            history_id: "hist-kept".into(),
            status: 201,
            status_text: "Created".into(),
            headers: vec![KeyValue {
                key: "Set-Cookie".into(),
                value: "session=secret-session-id".into(),
                enabled: true,
            }],
            body: r#"{"id":1,"token":"secret-jwt"}"#.into(),
            duration_ms: 12,
        }
    }

    impl CommandBusAdapter for ActivityWarningBus {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            ApiStubCommandBus.execute_read(command)
        }

        fn execute_saved_api_request(
            &self,
            request_id: &str,
            timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            ApiStubCommandBus.execute_saved_api_request(request_id, timeout_ms)
        }

        fn execute_saved_api_request_snapshot(
            &self,
            _snapshot: &ApiSavedRequest,
            _timeout_ms: Option<u64>,
            _environment_id: Option<&str>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(warned_response())
        }

        fn send_api_request_in_environment(
            &self,
            _input: ApiRequestInput,
            _environment_id: Option<&str>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(warned_response())
        }

        fn list_db_connections(
            &self,
            workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            ApiStubCommandBus.list_db_connections(workspace_id)
        }

        fn get_db_schema(
            &self,
            workspace_id: &str,
            connection_id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            ApiStubCommandBus.get_db_schema(workspace_id, connection_id)
        }

        fn execute_db_query(
            &self,
            input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            ApiStubCommandBus.execute_db_query(input)
        }
    }

    let bus = Arc::new(ActivityWarningBus {
        calls: AtomicUsize::new(0),
    });
    let registry = ToolRegistry::with_command_bus(bus.clone());
    for arguments in [
        json!({"requestId": "req-1"}),
        json!({"method": "POST", "url": "https://api.example.test/users"}),
    ] {
        bus.calls.store(0, Ordering::SeqCst);
        let result = registry
            .call("unfour.api.send_request", arguments)
            .expect("completed send stays successful");
        assert_eq!(result["isError"], false);
        assert_eq!(bus.calls.load(Ordering::SeqCst), 1);
        let content = &result["structuredContent"];
        assert_eq!(content["ok"], true);
        assert_eq!(content["status"], 201);
        assert_eq!(content["warnings"], json!(["ACTIVITY_RECORD_FAILED"]));
        assert!(!result.to_string().contains("secret-jwt"));
        assert!(!result.to_string().contains("secret-session-id"));
        assert!(!result.to_string().contains("audit sink unavailable"));
        crate::output_schema::assert_success_matches_output_schema(
            &registry,
            "unfour.api.send_request",
            &result,
        );
    }
}
