use std::sync::Arc;

use serde_json::json;
use unfour_command_bus::{CurrentWorkspaceResult, ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use crate::tools::ToolRegistry;

use super::fixtures::registry;

#[test]
fn patch_file_multi_match_requires_confirmation_then_replaces_all() {
    let first = registry()
        .call(
            "unfour.ssh.patch_file",
            json!({
                "connectionId": "conn-1",
                "path": "/srv/app/config.toml",
                "search": "debug = false",
                "replace": "debug = true"
            }),
        )
        .expect("multi-match patch should return confirmation");

    assert_eq!(first["isError"], true);
    assert_eq!(first["structuredContent"]["requires_confirmation"], true);
    let confirmation = first["structuredContent"]["confirmation_text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(confirmation.starts_with("SSH_PATCH_MULTIPLE_MATCHES:"));

    let confirmed = registry()
        .call(
            "unfour.ssh.patch_file",
            json!({
                "connectionId": "conn-1",
                "path": "/srv/app/config.toml",
                "search": "debug = false",
                "replace": "debug = true",
                "confirm": true,
                "confirmation_text": confirmation
            }),
        )
        .expect("confirmed multi-match patch should execute");

    assert_eq!(confirmed["isError"], false);
    assert_eq!(confirmed["structuredContent"]["patched"], true);
    assert_eq!(confirmed["structuredContent"]["matches"], 2);
    assert_eq!(
        confirmed["structuredContent"]["diffSummary"]["replacements"],
        2
    );
}

#[test]
fn write_file_accepts_multiline_content_without_multiline_shell_command() {
    let content = "services:\n  db:\n    image: postgres:15\n";
    let result = registry()
        .call(
            "unfour.ssh.write_file",
            json!({
                "connectionId": "conn-1",
                "path": "/srv/app/docker-compose.yml",
                "content": content
            }),
        )
        .expect("multiline content should be encoded into a control-free command");

    assert_eq!(result["isError"], false);
    assert_eq!(
        result["structuredContent"]["path"],
        "/srv/app/docker-compose.yml"
    );
    assert_eq!(result["structuredContent"]["mode"], "overwrite");
    assert_eq!(result["structuredContent"]["bytes"], content.len());
}

#[test]
fn write_file_is_blocked_in_prod() {
    struct ProdSshCommandBus;

    impl CommandBusAdapter for ProdSshCommandBus {
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
            unreachable!()
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

    let registry = ToolRegistry::with_command_bus(Arc::new(ProdSshCommandBus));
    let result = registry
        .call(
            "unfour.ssh.write_file",
            json!({
                "connectionId": "conn-1",
                "path": "/tmp/app/config.txt",
                "content": "safe"
            }),
        )
        .expect("policy denial should be structured");
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["environment"], "prod");
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
}
