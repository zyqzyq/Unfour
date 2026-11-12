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
    let payload = crate::response::error_json(&first);
    assert_eq!(payload["requires_confirmation"], true);
    let confirmation = payload["confirmation_text"].as_str().unwrap().to_string();
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
fn write_file_accepts_multiline_content_through_typed_file_operation() {
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
        .expect("multiline content should travel as data");

    assert_eq!(result["isError"], false);
    assert_eq!(
        result["structuredContent"]["path"],
        "/srv/app/docker-compose.yml"
    );
    assert_eq!(result["structuredContent"]["mode"], "overwrite");
    assert_eq!(result["structuredContent"]["bytes"], content.len());
}

#[test]
fn file_content_limit_is_four_mib_of_utf8_not_command_characters() {
    use unfour_core::models::SSH_FILE_MAX_CONTENT_BYTES;
    let content = format!("{}x", "界".repeat(SSH_FILE_MAX_CONTENT_BYTES / 3));
    assert_eq!(content.len(), SSH_FILE_MAX_CONTENT_BYTES);
    for mode in ["overwrite", "append", "create"] {
        let result = registry()
            .call(
                "unfour.ssh.write_file",
                json!({
                    "connectionId":"conn-1", "path":"/srv/config", "mode":mode, "content":content,
                }),
            )
            .unwrap();
        assert_eq!(result["isError"], false);
        assert_eq!(
            result["structuredContent"]["bytes"],
            SSH_FILE_MAX_CONTENT_BYTES
        );
        assert!(!result.to_string().contains(&content[..30]));
    }
    let result = registry()
        .call(
            "unfour.ssh.write_file",
            json!({
                "connectionId":"conn-1", "path":"/srv/config", "content":format!("{content}x"),
            }),
        )
        .unwrap();
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "SSH_FILE_CONTENT_TOO_LARGE"
    );
    let details = &crate::response::error_json(&result)["error"]["details"];
    assert_eq!(details["actualBytes"], SSH_FILE_MAX_CONTENT_BYTES + 1);
    assert_eq!(details["maxBytes"], SSH_FILE_MAX_CONTENT_BYTES);
}

#[test]
fn full_access_patch_can_replace_multiple_matches_without_confirmation_loop() {
    use crate::tools::policy::{
        McpRisk, ResolvedMcpPolicy, ToolPolicyEvaluation, WorkspacePolicyContext,
    };
    let evaluation = ToolPolicyEvaluation {
        workspace: WorkspacePolicyContext {
            workspace_id: "ws-active".into(),
            workspace_name: "Active".into(),
            environment_type: "dev".into(),
            mcp_policy: "full_access".into(),
            workspace_root: None,
            mode: "local".into(),
            source: "command-bus".into(),
        },
        resolved_policy: ResolvedMcpPolicy::FullAccess,
        risk: McpRisk::Write,
        saved_request: None,
    };
    let result = super::super::operations::ssh_patch_file(
        &super::fixtures::SshStubCommandBus,
        &evaluation,
        json!({"connectionId":"conn-1", "path":"/srv/config", "search":"old", "replace":"new"}),
    )
    .unwrap();
    assert_eq!(result["patched"], true);
    assert_eq!(result["diffSummary"]["replacements"], 2);
}

#[test]
fn patch_limit_applies_to_combined_search_and_replace() {
    use unfour_core::models::SSH_FILE_MAX_CONTENT_BYTES;
    let args = json!({"connectionId":"conn-1", "path":"/srv/config", "search":"a", "replace":"b".repeat(SSH_FILE_MAX_CONTENT_BYTES - 1)});
    let result = registry()
        .call("unfour.ssh.patch_file", args.clone())
        .unwrap();
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "CONFIRMATION_REQUIRED"
    );
    let mut oversized = args;
    oversized["search"] = json!("aa");
    let result = registry().call("unfour.ssh.patch_file", oversized).unwrap();
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "SSH_FILE_CONTENT_TOO_LARGE"
    );
}

#[test]
fn write_confirmation_cannot_be_reused_for_different_same_length_content() {
    let mut args = json!({"connectionId":"conn-1","path":"/etc/config","content":"old"});
    let first = registry()
        .call("unfour.ssh.write_file", args.clone())
        .unwrap();
    let confirmation = crate::response::error_json(&first)["confirmation_text"].clone();
    args["confirm"] = json!(true);
    args["confirmation_text"] = confirmation;
    let accepted = registry()
        .call("unfour.ssh.write_file", args.clone())
        .unwrap();
    assert_eq!(accepted["isError"], false);
    args["content"] = json!("new");
    let denied = registry().call("unfour.ssh.write_file", args).unwrap();
    assert_eq!(
        crate::response::error_json(&denied)["error"]["code"],
        "CONFIRMATION_REQUIRED"
    );
}

#[test]
fn sensitive_multi_match_confirmation_completes_and_binds_content() {
    let mut args =
        json!({"connectionId":"conn-1","path":"/etc/config","search":"old","replace":"new"});
    for _ in 0..2 {
        let result = registry()
            .call("unfour.ssh.patch_file", args.clone())
            .unwrap();
        assert_eq!(result["isError"], true);
        args["confirm"] = json!(true);
        args["confirmation_text"] =
            crate::response::error_json(&result)["confirmation_text"].clone();
    }
    let accepted = registry()
        .call("unfour.ssh.patch_file", args.clone())
        .unwrap();
    assert_eq!(accepted["structuredContent"]["patched"], true);
    args["replace"] = json!("NEW");
    let denied = registry().call("unfour.ssh.patch_file", args).unwrap();
    assert_eq!(
        crate::response::error_json(&denied)["error"]["code"],
        "CONFIRMATION_REQUIRED"
    );
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
    crate::response::assert_call_meta(&result, "prod", "medium");
    assert_eq!(
        crate::response::error_json(&result)["error"]["code"],
        "WORKSPACE_POLICY_BLOCKED"
    );
}
