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

#[test]
fn send_request_blocks_scripted_saved_get_in_prod() {
    struct ProdScriptedGetCommandBus;

    impl CommandBusAdapter for ProdScriptedGetCommandBus {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            match command {
                ReadCommand::ApiGetRequest { request_id } => {
                    Ok(ReadCommandResult::ApiRequest(ApiRequestDetailResult {
                        request: ApiSavedRequest {
                            id: request_id,
                            workspace_id: "ws-prod".to_string(),
                            name: "Scripted GET".to_string(),
                            collection_id: "collection".to_string(),
                            parent_folder_id: None,
                            sort_order: 0,
                            auth_json: "{}".to_string(),
                            method: "GET".to_string(),
                            url: "https://api.example.test".to_string(),
                            headers_json: "[]".to_string(),
                            query_json: "[]".to_string(),
                            body: None,
                            body_kind: "none".to_string(),
                            pre_request_script: Some(
                                "pm.environment.set('mutated', 'yes')".to_string(),
                            ),
                            post_response_script: None,
                            script_schema_version: 1,
                            settings_json: r#"{"timeoutMs":null}"#.to_string(),
                            created_at: String::new(),
                            updated_at: String::new(),
                            deleted_at: None,
                            revision: 1,
                            sync_status: "local".to_string(),
                            remote_id: None,
                        },
                        source: "command-bus".to_string(),
                    }))
                }
                ReadCommand::ListWorkspaces => {
                    Ok(ReadCommandResult::Workspaces(WorkspaceListResult {
                        workspaces: vec![WorkspaceSummary {
                            id: "ws-prod".to_string(),
                            name: "Prod".to_string(),
                            is_default: true,
                            is_active: true,
                            environment_type: "prod".to_string(),
                            mcp_policy: "auto".to_string(),
                            last_opened_at: None,
                        }],
                        active_workspace_id: "ws-prod".to_string(),
                        count: 1,
                        source: "command-bus".to_string(),
                    }))
                }
                _ => unreachable!(),
            }
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            panic!("scripted prod GET should be blocked before execution")
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

    let result = ToolRegistry::with_command_bus(Arc::new(ProdScriptedGetCommandBus))
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
