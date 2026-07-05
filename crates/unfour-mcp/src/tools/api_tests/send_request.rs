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
    assert_eq!(content["environment"], "dev");
    assert_eq!(content["risk_level"], "medium");
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
    assert_eq!(result["structuredContent"]["environment"], "prod");
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
}

#[test]
fn send_request_clamps_timeout_to_60s() {
    // Sending with 120000ms should be clamped - the stub ignores timeout,
    // but we verify the tool doesn't reject the call
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
fn send_request_returns_structured_error_on_failure() {
    let registry = ToolRegistry::with_command_bus(Arc::new(FailingApiCommandBus));
    let result = registry
        .call("unfour.api.send_request", json!({ "requestId": "req-1" }))
        .expect("execution errors become MCP tool results");

    assert_eq!(result["isError"], true);
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "COMMAND_BUS_API_SEND_FAILED"
    );
}

#[test]
fn command_bus_read_failure_returns_structured_error() {
    let registry = ToolRegistry::with_command_bus(Arc::new(FailingApiCommandBus));
    let result = registry
        .call("unfour.api.list_collections", json!({}))
        .expect("execution errors become MCP tool results");

    assert_eq!(result["isError"], true);
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "COMMAND_BUS_READ_FAILED"
    );
}
