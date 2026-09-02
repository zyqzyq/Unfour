use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

// --- environment tests ---

#[test]
fn list_environments_masks_sensitive_variables_only() {
    let result = api_registry()
        .call("unfour.api.list_environments", json!({}))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let env = &result["structuredContent"]["environments"][0];
    assert_eq!(env["name"], "Staging");
    assert_eq!(env["isActive"], true);
    assert_eq!(env["variableCount"], 2);

    let vars = env["variables"].as_array().unwrap();
    let base = vars.iter().find(|v| v["key"] == "baseUrl").unwrap();
    // Non-sensitive value is shown verbatim so requests are intelligible.
    assert_eq!(base["value"], "https://api.staging.example.com");

    let token = vars.iter().find(|v| v["key"] == "token").unwrap();
    let token_val = token["value"].as_str().unwrap();
    assert!(token_val.starts_with("[mask "));
    assert!(token_val.contains("scheme=Bearer"));
    assert!(!token_val.contains("secret-token"));
}

#[test]
fn create_environment_returns_created_summary() {
    let result = api_registry()
        .call("unfour.api.create_environment", json!({ "name": "QA" }))
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    assert_eq!(
        result["structuredContent"]["apiEnvironment"]["id"],
        "env-created"
    );
    assert_eq!(result["structuredContent"]["apiEnvironment"]["name"], "QA");
    assert_eq!(
        result["structuredContent"]["apiEnvironment"]["variableCount"],
        0
    );
}

#[test]
fn update_environment_masks_sensitive_variables_in_result() {
    let result = api_registry()
        .call(
            "unfour.api.update_environment",
            json!({
                "environmentId": "env-1",
                "name": "Staging 2",
                "variables": [
                    { "key": "baseUrl", "value": "https://example.com", "enabled": true },
                    { "key": "token", "value": "Bearer secret-token", "enabled": true }
                ]
            }),
        )
        .expect("should succeed");

    assert_eq!(result["isError"], false);
    let environment = &result["structuredContent"]["apiEnvironment"];
    assert_eq!(environment["name"], "Staging 2");
    assert_eq!(environment["variables"][0]["value"], "https://example.com");
    let token = environment["variables"][1]["value"].as_str().unwrap();
    assert!(token.starts_with("[mask "));
    assert!(!token.contains("secret-token"));
}

#[test]
fn update_environment_requires_variables_array() {
    let error = api_registry()
        .call(
            "unfour.api.update_environment",
            json!({
                "environmentId": "env-1",
                "name": "Staging",
                "variables": { "baseUrl": "https://example.com" }
            }),
        )
        .expect_err("should reject invalid variables");

    assert!(matches!(error, ToolCallError::InvalidArguments(_)));
}

#[test]
fn delete_environment_reports_soft_delete() {
    let result = api_registry()
        .call(
            "unfour.api.delete_environment",
            json!({ "environmentId": "env-1" }),
        )
        .expect("full-access dev workspace should allow deletion");

    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["softDelete"], true);
    assert_eq!(result["structuredContent"]["deletedEnvironmentId"], "env-1");
    assert_eq!(result["structuredContent"]["remainingCount"], 0);
}

#[test]
fn set_environment_variable_updates_by_key_and_preserves_optional_metadata() {
    let registry = api_registry();
    let result = registry
        .call(
            "unfour.api.set_environment_variable",
            json!({
                "environmentId": "env-1",
                "key": "BASE_URL",
                "value": ""
            }),
        )
        .expect("set should update an existing variable by key");

    assert_eq!(result["isError"], false);
    let content = &result["structuredContent"];
    assert_eq!(content["created"], false);
    assert_eq!(content["variable"]["id"], "var-base-url");
    assert_eq!(content["variable"]["value"], "");
    assert_eq!(content["variable"]["enabled"], true);
    assert_eq!(content["variable"]["isSecret"], false);
    assert_eq!(content["variable"]["description"], "keep this description");
    crate::output_schema::assert_success_matches_output_schema(
        &registry,
        "unfour.api.set_environment_variable",
        &result,
    );
}

#[test]
fn set_environment_variable_masks_explicit_and_sensitive_secrets() {
    let explicit = api_registry()
        .call(
            "unfour.api.set_environment_variable",
            json!({
                "environmentId": "env-1",
                "key": "DISPLAY_ALIAS",
                "value": "explicit secret canary",
                "isSecret": true
            }),
        )
        .expect("explicit secret should be created");
    let explicit_value = explicit["structuredContent"]["variable"]["value"]
        .as_str()
        .unwrap();
    assert!(explicit_value.starts_with("[mask "));
    assert!(!explicit.to_string().contains("explicit secret canary"));

    let sensitive = api_registry()
        .call(
            "unfour.api.set_environment_variable",
            json!({
                "environmentId": "env-1",
                "key": "TOKEN",
                "value": "sensitive key canary",
                "isSecret": false
            }),
        )
        .expect("sensitive-key value should be created");
    assert_eq!(
        sensitive["structuredContent"]["variable"]["isSecret"],
        false
    );
    assert!(sensitive["structuredContent"]["variable"]["value"]
        .as_str()
        .unwrap()
        .starts_with("[mask "));
    assert!(!sensitive.to_string().contains("sensitive key canary"));
}

#[test]
fn set_environment_variable_auto_marks_new_sensitive_key() {
    let result = api_registry()
        .call(
            "unfour.api.set_environment_variable",
            json!({
                "environmentId": "env-1",
                "key": "ACCESS_TOKEN",
                "value": "auto secret canary"
            }),
        )
        .expect("sensitive key should be created");

    assert_eq!(result["structuredContent"]["created"], true);
    assert_eq!(result["structuredContent"]["variable"]["isSecret"], true);
    assert!(!result.to_string().contains("auto secret canary"));
}

#[test]
fn delete_environment_variable_deletes_by_key() {
    let registry = api_registry();
    let result = registry
        .call(
            "unfour.api.delete_environment_variable",
            json!({ "environmentId": "env-1", "key": "BASE_URL" }),
        )
        .expect("dev full access should delete without confirmation");

    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["deleted"], true);
    assert_eq!(result["structuredContent"]["environmentId"], "env-1");
    assert_eq!(result["structuredContent"]["key"], "BASE_URL");
    crate::output_schema::assert_success_matches_output_schema(
        &registry,
        "unfour.api.delete_environment_variable",
        &result,
    );
}

#[test]
fn delete_environment_variable_returns_stable_not_found_error() {
    let result = api_registry()
        .call(
            "unfour.api.delete_environment_variable",
            json!({ "environmentId": "env-1", "key": "MISSING" }),
        )
        .expect("not found should be a structured MCP result");

    assert_eq!(result["isError"], true);
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "NOT_FOUND"
    );
}

#[test]
fn environment_variable_lookup_blocks_cross_workspace_environment() {
    let result = api_registry()
        .call(
            "unfour.api.set_environment_variable",
            json!({
                "workspaceId": "ws-1",
                "environmentId": "env-from-another-workspace",
                "key": "BASE_URL",
                "value": "https://example.test"
            }),
        )
        .expect("cross-workspace environment should return a structured error");

    assert_eq!(result["isError"], true);
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "NOT_FOUND"
    );
}

struct EnvironmentPolicyStub {
    environment_type: &'static str,
    mcp_policy: &'static str,
    deleted: Arc<AtomicBool>,
}

impl CommandBusAdapter for EnvironmentPolicyStub {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        match command {
            ReadCommand::CurrentWorkspace => Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "ws-1".to_string(),
                    workspace_name: "Policy Workspace".to_string(),
                    environment_type: self.environment_type.to_string(),
                    mcp_policy: self.mcp_policy.to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            )),
            other => ApiStubCommandBus.execute_read(other),
        }
    }

    fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        ApiStubCommandBus.execute_saved_api_request(request_id, timeout_ms)
    }

    fn list_workspace_environments(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceEnvironment>, CommandBusAdapterError> {
        ApiStubCommandBus.list_workspace_environments(workspace_id)
    }

    fn create_api_environment_variable(
        &self,
        _workspace_id: &str,
        _environment_id: &str,
        _input: WorkspaceVariableInput,
    ) -> Result<WorkspaceEnvironmentVariable, CommandBusAdapterError> {
        panic!("policy should block environment-variable creation")
    }

    fn update_api_environment_variable(
        &self,
        _workspace_id: &str,
        _environment_id: &str,
        _variable_id: &str,
        _input: WorkspaceVariableInput,
    ) -> Result<WorkspaceEnvironmentVariable, CommandBusAdapterError> {
        panic!("policy should block environment-variable update")
    }

    fn delete_api_environment_variable(
        &self,
        workspace_id: &str,
        environment_id: &str,
        variable_id: &str,
    ) -> Result<Vec<WorkspaceEnvironmentVariable>, CommandBusAdapterError> {
        self.deleted.store(true, Ordering::SeqCst);
        ApiStubCommandBus.delete_api_environment_variable(workspace_id, environment_id, variable_id)
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

#[test]
fn delete_environment_variable_uses_guarded_confirmation_handshake() {
    let deleted = Arc::new(AtomicBool::new(false));
    let registry = ToolRegistry::with_command_bus(Arc::new(EnvironmentPolicyStub {
        environment_type: "test",
        mcp_policy: "auto",
        deleted: deleted.clone(),
    }));
    let arguments = json!({ "environmentId": "env-1", "key": "BASE_URL" });
    let first = registry
        .call("unfour.api.delete_environment_variable", arguments.clone())
        .expect("guarded delete should request confirmation");
    assert_eq!(first["isError"], true);
    assert!(!deleted.load(Ordering::SeqCst));
    let confirmation_text = crate::response::error_json(&first)["confirmation_text"]
        .as_str()
        .expect("confirmation text")
        .to_string();

    let mut confirmed = arguments;
    confirmed["confirm"] = json!(true);
    confirmed["confirmation_text"] = json!(confirmation_text);
    let result = registry
        .call("unfour.api.delete_environment_variable", confirmed)
        .expect("confirmed delete should execute");
    assert_eq!(result["isError"], false);
    assert!(deleted.load(Ordering::SeqCst));
}

#[test]
fn prod_policy_blocks_environment_variable_set_and_delete() {
    let deleted = Arc::new(AtomicBool::new(false));
    let registry = ToolRegistry::with_command_bus(Arc::new(EnvironmentPolicyStub {
        environment_type: "prod",
        mcp_policy: "auto",
        deleted: deleted.clone(),
    }));

    for (tool_name, arguments) in [
        (
            "unfour.api.set_environment_variable",
            json!({ "environmentId": "env-1", "key": "BASE_URL", "value": "prod" }),
        ),
        (
            "unfour.api.delete_environment_variable",
            json!({ "environmentId": "env-1", "key": "BASE_URL" }),
        ),
    ] {
        let result = registry
            .call(tool_name, arguments)
            .expect("prod denial should be structured");
        assert_eq!(result["isError"], true, "{tool_name}");
        assert_eq!(
            crate::response::error_json(&result)["error"]["code"],
            "WORKSPACE_POLICY_BLOCKED",
            "{tool_name}"
        );
    }
    assert!(!deleted.load(Ordering::SeqCst));
}
