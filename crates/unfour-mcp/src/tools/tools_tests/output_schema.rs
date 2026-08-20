use std::sync::Arc;

use serde_json::json;

use crate::command_bus_adapter::LocalCommandBusAdapter;
use crate::output_schema::assert_success_matches_output_schema;
use crate::response::{assert_call_meta, error_json};
use crate::tools::ToolRegistry;

use super::{FailingCommandBus, StubCommandBus};

fn stub_registry() -> ToolRegistry {
    ToolRegistry::with_command_bus(Arc::new(StubCommandBus))
}

#[test]
fn health_structured_content_matches_output_schema() {
    let registry = stub_registry();
    let result = registry
        .call("unfour.system.health", json!({}))
        .expect("health should succeed");
    assert_success_matches_output_schema(&registry, "unfour.system.health", &result);
    assert_eq!(result["structuredContent"]["appName"], "Unfour");
    assert_eq!(result["structuredContent"]["storageReady"], true);
    assert_eq!(result["structuredContent"]["commandBusReady"], true);
    assert_eq!(result["structuredContent"]["source"], "command-bus");
}

#[test]
fn workspace_list_structured_content_matches_output_schema() {
    let registry = stub_registry();
    let result = registry
        .call("unfour.workspace.list", json!({}))
        .expect("workspace list should succeed");
    assert_success_matches_output_schema(&registry, "unfour.workspace.list", &result);
    assert_eq!(result["structuredContent"]["count"], 2);
    assert_eq!(
        result["structuredContent"]["activeWorkspaceId"],
        "workspace-1"
    );
}

#[test]
fn db_list_connections_structured_content_matches_output_schema() {
    let registry = stub_registry();
    let result = registry
        .call("unfour.db.list_connections", json!({}))
        .expect("db list connections should succeed");
    assert_success_matches_output_schema(&registry, "unfour.db.list_connections", &result);
    assert_eq!(result["structuredContent"]["count"], 0);
    assert_eq!(result["structuredContent"]["connections"], json!([]));
    assert_eq!(result["structuredContent"]["source"], "command-bus");
}

#[test]
fn meta_does_not_affect_output_schema_validation() {
    let registry = stub_registry();
    let result = registry
        .call("unfour.system.health", json!({}))
        .expect("health should succeed");
    assert_call_meta(&result, "prod", "low");
    assert_success_matches_output_schema(&registry, "unfour.system.health", &result);
    assert!(result["structuredContent"].get("riskLevel").is_none());
    assert!(result["structuredContent"].get("environment").is_none());
    assert!(result["structuredContent"].get("durationMs").is_none());
}

#[test]
fn ephemeral_no_arg_tools_match_their_output_schemas() {
    let command_bus =
        LocalCommandBusAdapter::ephemeral().expect("ephemeral command bus should initialize");
    let registry = ToolRegistry::with_command_bus(command_bus);
    for tool_name in [
        "unfour.system.health",
        "unfour.workspace.current",
        "unfour.workspace.list",
        "unfour.workspace.list_variables",
        "unfour.connection.list",
        "unfour.activity.list",
        "unfour.api.list_collections",
        "unfour.api.list_requests",
        "unfour.api.list_history",
        "unfour.api.list_environments",
        "unfour.db.list_connections",
        "unfour.ssh.list_connections",
        "unfour.ssh.list_history",
        "unfour.ssh.list_tasks",
    ] {
        let result = registry
            .call(tool_name, json!({}))
            .unwrap_or_else(|error| panic!("{tool_name} should return an MCP result: {error:?}"));
        assert_success_matches_output_schema(&registry, tool_name, &result);
    }
}

#[test]
fn execution_error_omits_structured_content_and_keeps_text_payload() {
    let result = ToolRegistry::with_command_bus(Arc::new(FailingCommandBus))
        .call("unfour.workspace.current", json!({}))
        .expect("execution failures are MCP tool results");
    let payload = error_json(&result);
    assert_eq!(payload["error"]["code"], "COMMAND_BUS_READ_FAILED");
    assert_eq!(
        payload["error"]["message"],
        "The command-bus read operation failed."
    );
    assert_call_meta(&result, "unknown", "medium");
    assert!(!result.to_string().contains("password"));
}

#[test]
fn policy_blocked_omits_structured_content_and_keeps_text_payload() {
    let result = stub_registry()
        .call(
            "unfour.ssh.exec",
            json!({ "connectionId": "ssh-1", "command": "rm -rf /tmp/app" }),
        )
        .expect("policy denials are MCP tool results");
    let payload = error_json(&result);
    assert_eq!(payload["error"]["code"], "WORKSPACE_POLICY_BLOCKED");
    assert_eq!(payload["blocked"], true);
    assert_eq!(payload["workspaceId"], "workspace-1");
    assert_eq!(payload["environmentType"], "prod");
    assert_eq!(payload["capability"], "ssh:exec");
    assert_call_meta(&result, "prod", "medium");
}
