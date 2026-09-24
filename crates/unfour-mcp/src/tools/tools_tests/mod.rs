use std::sync::Arc;

use serde_json::json;
use unfour_command_bus::{
    ApiCollectionListResult, ApiEnvironmentListResult, ApiHistoryDetailResult,
    ApiHistoryListResult, ApiRequestDetailResult, ApiRequestListResult, ConnectionListResult,
    CurrentWorkspaceResult, ReadCommand, ReadCommandResult, SafeConnection, SafeConnectionSummary,
    WorkspaceListResult, WorkspaceSummary,
};
use unfour_core::models::{
    ApiHistoryDetail, ApiResponse, ApiSavedRequest, DatabaseConnection, DatabaseQueryInput,
    DatabaseQueryResult, DatabaseQuerySafety, DatabaseSchema, KeyValue,
};

use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};

use super::ToolRegistry;

struct StubCommandBus;

impl CommandBusAdapter for StubCommandBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        Ok(match command {
            ReadCommand::CurrentWorkspace => {
                ReadCommandResult::CurrentWorkspace(CurrentWorkspaceResult {
                    workspace_id: "workspace-1".to_string(),
                    workspace_name: "Local Workspace".to_string(),
                    environment_type: "prod".to_string(),
                    mcp_policy: "auto".to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                })
            }
            ReadCommand::ListWorkspaces => ReadCommandResult::Workspaces(WorkspaceListResult {
                workspaces: vec![
                    WorkspaceSummary {
                        id: "workspace-1".to_string(),
                        name: "Local Workspace".to_string(),
                        is_default: true,
                        is_active: true,
                        environment_type: "prod".to_string(),
                        mcp_policy: "auto".to_string(),
                        last_opened_at: Some("2026-06-20T00:00:00Z".to_string()),
                    },
                    WorkspaceSummary {
                        id: "workspace-2".to_string(),
                        name: "Scratch".to_string(),
                        is_default: false,
                        is_active: false,
                        environment_type: "dev".to_string(),
                        mcp_policy: "auto".to_string(),
                        last_opened_at: None,
                    },
                ],
                active_workspace_id: "workspace-1".to_string(),
                count: 2,
                source: "command-bus".to_string(),
            }),
            ReadCommand::ListConnections { .. } => {
                ReadCommandResult::Connections(ConnectionListResult {
                    connections: vec![SafeConnection {
                        id: "database-1".to_string(),
                        name: "Local Database".to_string(),
                        connection_type: "database".to_string(),
                        workspace_id: "workspace-1".to_string(),
                        safe_summary: SafeConnectionSummary {
                            host: Some("localhost".to_string()),
                            database_type: Some("postgres".to_string()),
                            api_base_url: None,
                        },
                    }],
                    count: 1,
                    source: "command-bus".to_string(),
                })
            }
            ReadCommand::ApiListCollections { .. } => {
                ReadCommandResult::ApiCollections(ApiCollectionListResult {
                    collections: vec![],
                    count: 0,
                    source: "command-bus".to_string(),
                })
            }
            ReadCommand::ApiListRequests { .. } => {
                ReadCommandResult::ApiRequests(ApiRequestListResult {
                    requests: vec![],
                    count: 0,
                    source: "command-bus".to_string(),
                })
            }
            ReadCommand::ApiGetRequest { request_id } => {
                ReadCommandResult::ApiRequest(ApiRequestDetailResult {
                    request: ApiSavedRequest {
                        id: request_id,
                        workspace_id: "workspace-1".to_string(),
                        name: "Test Request".to_string(),
                        collection_id: "collection-1".to_string(),
                        parent_folder_id: None,
                        sort_order: 0,
                        auth_json: r#"{"type":"none"}"#.to_string(),
                        method: "GET".to_string(),
                        url: "https://api.example.com/test".to_string(),
                        headers_json: "[]".to_string(),
                        query_json: "[]".to_string(),
                        body: None,
                        body_kind: "json".to_string(),
                        pre_request_script: None,
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
                })
            }
            ReadCommand::ApiListHistory { .. } => {
                ReadCommandResult::ApiHistory(ApiHistoryListResult {
                    history: vec![],
                    count: 0,
                    source: "command-bus".to_string(),
                })
            }
            ReadCommand::ApiGetHistory { history_id, .. } => {
                ReadCommandResult::ApiHistoryDetailResult(ApiHistoryDetailResult {
                    detail: ApiHistoryDetail {
                        id: history_id,
                        workspace_id: "workspace-1".to_string(),
                        name: None,
                        method: "GET".to_string(),
                        url: "https://api.example.com/test".to_string(),
                        request_headers_json: "[]".to_string(),
                        request_query_json: "[]".to_string(),
                        request_body_kind: "json".into(),
                        request_body: None,
                        status: Some(200),
                        duration_ms: Some(10),
                        response_headers_json: "[]".to_string(),
                        response_body_preview: None,
                        created_at: String::new(),
                        updated_at: String::new(),
                    },
                    source: "command-bus".to_string(),
                })
            }
            ReadCommand::ApiListEnvironments { .. } => {
                ReadCommandResult::ApiEnvironments(ApiEnvironmentListResult {
                    environments: vec![],
                    count: 0,
                    source: "command-bus".to_string(),
                })
            }
            ReadCommand::ListActivity { .. } => {
                ReadCommandResult::Activity(unfour_command_bus::ActivityListResult {
                    activity: vec![],
                    count: 0,
                    source: "command-bus".to_string(),
                })
            }
        })
    }

    fn execute_saved_api_request(
        &self,
        _request_id: &str,
        _timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        Ok(ApiResponse {
            history_id: "history-1".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![KeyValue {
                key: "content-type".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            }],
            body: "{\"ok\":true}".to_string(),
            duration_ms: 42,
        })
    }

    fn list_db_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        Ok(vec![])
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        Ok(DatabaseSchema {
            connection_id: String::new(),
            tables: vec![],
        })
    }

    fn execute_db_query(
        &self,
        _input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        Ok(DatabaseQueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: 0,
            duration_ms: 0,
            safety: DatabaseQuerySafety {
                classification: "read".to_string(),
                requires_confirmation: false,
                confirmed: true,
                message: None,
            },
        })
    }

    fn system_health(&self) -> Result<unfour_core::models::SystemHealth, CommandBusAdapterError> {
        Ok(unfour_core::models::SystemHealth {
            app_name: "Unfour".to_string(),
            storage_ready: true,
            command_bus_ready: true,
            ai_reserved_capabilities: vec![],
        })
    }
}

struct FailingCommandBus;

impl CommandBusAdapter for FailingCommandBus {
    fn execute_read(
        &self,
        _command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_READ_FAILED",
            message: "The command-bus read operation failed.",
            details: serde_json::json!({}),
        })
    }

    fn execute_saved_api_request(
        &self,
        _request_id: &str,
        _timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_API_SEND_FAILED",
            message: "The command-bus API send operation failed.",
            details: serde_json::json!({}),
        })
    }

    fn list_db_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_DB_LIST_FAILED",
            message: "The command-bus database list operation failed.",
            details: serde_json::json!({}),
        })
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_DB_SCHEMA_FAILED",
            message: "The command-bus database schema operation failed.",
            details: serde_json::json!({}),
        })
    }

    fn execute_db_query(
        &self,
        _input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "COMMAND_BUS_DB_QUERY_FAILED",
            message: "The command-bus database query operation failed.",
            details: serde_json::json!({}),
        })
    }
}

#[test]
fn tool_annotations_classify_side_effects() {
    let definitions = ToolRegistry::with_command_bus(Arc::new(StubCommandBus)).definitions();
    let annotations = |name: &str| {
        definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("missing tool {name}"))
            .annotations
    };

    // Local read-only tool: no external reach, no mutation.
    let ws = annotations("unfour.workspace.current");
    assert!(ws.read_only_hint);
    assert!(!ws.open_world_hint);

    // Reaches an external system (SSH host) but does not mutate it.
    let ssh = annotations("unfour.ssh.run_diagnostic");
    assert!(ssh.read_only_hint);
    assert!(ssh.open_world_hint);

    // Performs an external side effect (sends an HTTP request).
    let send = annotations("unfour.api.send_request");
    assert!(!send.read_only_hint);
    assert!(send.open_world_hint);
}

#[test]
fn tool_schemas_are_available() {
    let definitions = ToolRegistry::with_command_bus(Arc::new(StubCommandBus)).definitions();

    assert_eq!(definitions.len(), 71);
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.db.export_table"));
    assert!(definitions
        .iter()
        .all(|definition| definition.input_schema["type"] == "object"));
    assert!(definitions
        .iter()
        .all(|definition| !definition.name.starts_with("unfour.mock.")));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.workspace.current"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.connection.list"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.list_collections"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.list_requests"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.get_request"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.send_request"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.create_environment"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.update_environment"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.delete_environment"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.set_environment_variable"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.delete_environment_variable"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.db.list_connections"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.db.create_connection"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.db.list_tables"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.db.describe_table"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.db.query_readonly"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.list_history"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.get_history"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.db.test_connection"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.system.health"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.api.list_environments"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.workspace.list"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.activity.list"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.ssh.create_connection"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.ssh.run_diagnostic"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.db.execute"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.ssh.exec"));
    assert!(definitions
        .iter()
        .any(|definition| definition.name == "unfour.ssh.list_history"));
    assert_eq!(
        definitions
            .iter()
            .find(|definition| definition.name == "unfour.connection.list")
            .unwrap()
            .input_schema["properties"]["type"]["default"],
        "all"
    );
}

#[test]
fn workspace_current_returns_command_bus_result() {
    let result = ToolRegistry::with_command_bus(Arc::new(StubCommandBus))
        .call("unfour.workspace.current", json!({}))
        .expect("workspace tool should succeed");

    assert_eq!(result["structuredContent"]["workspaceId"], "workspace-1");
    assert_eq!(
        result["structuredContent"]["workspaceRoot"],
        serde_json::Value::Null
    );
    assert_eq!(result["structuredContent"]["source"], "command-bus");
    assert_eq!(result["isError"], false);
}

#[test]
fn workspace_list_returns_all_workspaces_marking_active() {
    let result = ToolRegistry::with_command_bus(Arc::new(StubCommandBus))
        .call("unfour.workspace.list", json!({}))
        .expect("workspace list tool should succeed");

    let content = &result["structuredContent"];
    assert_eq!(content["count"], 2);
    assert_eq!(content["activeWorkspaceId"], "workspace-1");
    assert_eq!(content["workspaces"][0]["id"], "workspace-1");
    assert_eq!(content["workspaces"][0]["isActive"], true);
    assert_eq!(content["workspaces"][0]["isDefault"], true);
    assert_eq!(content["workspaces"][1]["isActive"], false);
    assert_eq!(content["source"], "command-bus");
}

#[test]
fn prod_workspace_blocks_ssh_execution_with_policy_payload() {
    let result = ToolRegistry::with_command_bus(Arc::new(StubCommandBus))
        .call(
            "unfour.ssh.exec",
            json!({ "connectionId": "ssh-1", "command": "rm -rf /tmp/app" }),
        )
        .expect("policy denials are MCP tool results");

    assert_eq!(result["isError"], true);
    let content = crate::response::error_json(&result);
    assert_eq!(content["blocked"], true);
    assert_eq!(content["workspaceId"], "workspace-1");
    assert_eq!(content["workspaceName"], "Local Workspace");
    assert_eq!(content["environmentType"], "prod");
    assert_eq!(content["mcpPolicy"], "auto");
    assert_eq!(content["resolvedPolicy"], "read_only");
    assert_eq!(content["capability"], "ssh:exec");
    assert_eq!(content["risk"], "execute");
    crate::response::assert_call_meta(&result, "prod", "medium");
}

#[test]
fn connection_list_returns_only_safe_summary() {
    let result = ToolRegistry::with_command_bus(Arc::new(StubCommandBus))
        .call("unfour.connection.list", json!({ "type": "database" }))
        .expect("connection tool should succeed");

    assert_eq!(result["structuredContent"]["count"], 1);
    assert_eq!(
        result["structuredContent"]["connections"][0]["safeSummary"],
        json!({
            "host": "localhost",
            "databaseType": "postgres"
        })
    );
    assert_eq!(result["structuredContent"]["source"], "command-bus");
}

#[test]
fn command_bus_failure_returns_structured_tool_error() {
    let result = ToolRegistry::with_command_bus(Arc::new(FailingCommandBus))
        .call("unfour.workspace.current", json!({}))
        .expect("execution failures are MCP tool results");

    assert_eq!(result["isError"], true);
    let payload = crate::response::error_json(&result);
    assert_eq!(payload["error"]["code"], "COMMAND_BUS_READ_FAILED");
    assert_eq!(
        payload["error"]["message"],
        "The command-bus read operation failed."
    );
}

#[test]
fn default_schema_for_catalog_falls_back_only_when_catalog_is_omitted() {
    let bus = StubCommandBus;
    let schema = bus
        .get_db_schema_for_catalog("workspace-1", "conn-1", None)
        .expect("omitted catalog uses get_db_schema");
    assert!(schema.tables.is_empty());

    let error = bus
        .get_db_schema_for_catalog("workspace-1", "conn-1", Some("billing"))
        .expect_err("explicit catalog requires a catalog-aware adapter");
    assert_eq!(error.code, "COMMAND_BUS_OPERATION_UNSUPPORTED");
    assert!(error
        .message
        .contains("catalog-aware database schema reads"));
}

struct PolicyBus {
    environment_type: &'static str,
    mcp_policy: &'static str,
}

impl CommandBusAdapter for PolicyBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        match command {
            ReadCommand::CurrentWorkspace => Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "workspace-1".to_string(),
                    workspace_name: "Policy Workspace".to_string(),
                    environment_type: self.environment_type.to_string(),
                    mcp_policy: self.mcp_policy.to_string(),
                    workspace_root: None,
                    mode: "local".to_string(),
                    source: "command-bus".to_string(),
                },
            )),
            _ => Err(CommandBusAdapterError {
                code: "UNEXPECTED",
                message: "unexpected command",
                details: json!({}),
            }),
        }
    }

    fn execute_saved_api_request(
        &self,
        _request_id: &str,
        _timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        Err(CommandBusAdapterError {
            code: "UNEXPECTED",
            message: "unexpected command",
            details: json!({}),
        })
    }

    fn list_db_connections(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        Ok(vec![])
    }

    fn get_db_schema(
        &self,
        _workspace_id: &str,
        _connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        Ok(DatabaseSchema {
            connection_id: String::new(),
            tables: vec![],
        })
    }

    fn execute_db_query(
        &self,
        _input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        Ok(DatabaseQueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: 0,
            duration_ms: 0,
            safety: DatabaseQuerySafety {
                classification: "read".to_string(),
                requires_confirmation: false,
                confirmed: true,
                message: None,
            },
        })
    }

    fn system_health(&self) -> Result<unfour_core::models::SystemHealth, CommandBusAdapterError> {
        Ok(unfour_core::models::SystemHealth {
            app_name: "Unfour".to_string(),
            storage_ready: true,
            command_bus_ready: true,
            ai_reserved_capabilities: vec![],
        })
    }
}

fn without_duration(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(meta) = value.get_mut("_meta").and_then(|meta| meta.as_object_mut()) {
        meta.remove("durationMs");
    }
    value
}

fn assert_alias_matches_canonical(
    registry: &ToolRegistry,
    canonical: &str,
    arguments: serde_json::Value,
) {
    let alias = super::underscore_tool_alias(canonical);
    let canonical_result = registry
        .call(canonical, arguments.clone())
        .unwrap_or_else(|error| panic!("{canonical} failed: {error:?}"));
    let alias_result = registry
        .call(&alias, arguments)
        .unwrap_or_else(|error| panic!("{alias} failed: {error:?}"));

    assert_eq!(alias_result["_meta"]["tool"], canonical);
    assert_eq!(
        without_duration(alias_result),
        without_duration(canonical_result)
    );
}

#[test]
fn underscore_alias_calls_match_canonical_read_tool() {
    let registry = ToolRegistry::with_command_bus(Arc::new(StubCommandBus));
    assert_alias_matches_canonical(&registry, "unfour.system.health", json!({}));
    let alias = registry
        .call("unfour_system_health", json!({}))
        .expect("alias should resolve");
    assert_eq!(alias["_meta"]["tool"], "unfour.system.health");
    assert_eq!(alias["isError"], false);
}

#[test]
fn tools_list_exposes_canonical_names_only() {
    let definitions = ToolRegistry::with_command_bus(Arc::new(StubCommandBus)).definitions();
    let names = definitions
        .iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    assert!(names.contains(&"unfour.system.health"));
    assert!(names.contains(&"unfour.ssh.exec"));
    assert!(!names.contains(&"unfour_system_health"));
    assert!(!names.contains(&"unfour_ssh_exec"));
    assert!(names.iter().all(|name| name.contains('.')));
}

#[test]
fn unknown_tool_name_stays_unknown() {
    let registry = ToolRegistry::with_command_bus(Arc::new(StubCommandBus));
    let error = registry
        .call("unfour.system_health", json!({}))
        .expect_err("partial underscore rewrite is not an alias");
    assert_eq!(
        error,
        super::ToolCallError::UnknownTool("unfour.system_health".to_string())
    );
    let error = registry
        .call("not.a.tool", json!({}))
        .expect_err("unknown names stay unknown");
    assert_eq!(
        error,
        super::ToolCallError::UnknownTool("not.a.tool".to_string())
    );
}

#[test]
fn alias_index_rejects_duplicate_canonical_names_and_alias_collisions() {
    let duplicate = super::build_tool_alias_index(["unfour.system.health", "unfour.system.health"]);
    assert_eq!(
        duplicate,
        Err(super::ToolNameIndexError::DuplicateCanonical {
            name: "unfour.system.health".to_string(),
        })
    );

    let collision = super::build_tool_alias_index(["unfour.system.health", "unfour_system_health"]);
    assert_eq!(
        collision,
        Err(super::ToolNameIndexError::AliasCollision {
            alias: "unfour_system_health".to_string(),
            existing: "unfour_system_health".to_string(),
            incoming: "unfour.system.health".to_string(),
        })
    );

    let shared_alias = super::build_tool_alias_index(["a.b_c", "a_b.c"]);
    assert_eq!(
        shared_alias,
        Err(super::ToolNameIndexError::AliasCollision {
            alias: "a_b_c".to_string(),
            existing: "a.b_c".to_string(),
            incoming: "a_b.c".to_string(),
        })
    );
}

#[test]
fn alias_uses_the_same_policy_as_canonical_for_read_and_execute_tools() {
    let prod = ToolRegistry::with_command_bus(Arc::new(PolicyBus {
        environment_type: "prod",
        mcp_policy: "auto",
    }));
    assert_alias_matches_canonical(&prod, "unfour.system.health", json!({}));
    let exec_args = json!({ "connectionId": "ssh-1", "command": "rm -rf /tmp/app" });
    assert_alias_matches_canonical(&prod, "unfour.ssh.exec", exec_args.clone());
    let blocked = crate::response::error_json(
        &prod
            .call("unfour_ssh_exec", exec_args.clone())
            .expect("policy denial is a tool result"),
    );
    assert_eq!(blocked["error"]["code"], "WORKSPACE_POLICY_BLOCKED");
    assert_eq!(blocked["resolvedPolicy"], "read_only");
    assert_eq!(blocked["capability"], "ssh:exec");
    assert_eq!(blocked["risk"], "execute");

    let read_only = ToolRegistry::with_command_bus(Arc::new(PolicyBus {
        environment_type: "test",
        mcp_policy: "read_only",
    }));
    assert_alias_matches_canonical(&read_only, "unfour.system.health", json!({}));
    assert_alias_matches_canonical(&read_only, "unfour.ssh.exec", exec_args.clone());

    let guarded = ToolRegistry::with_command_bus(Arc::new(PolicyBus {
        environment_type: "test",
        mcp_policy: "guarded",
    }));
    assert_alias_matches_canonical(&guarded, "unfour.system.health", json!({}));
    assert_alias_matches_canonical(&guarded, "unfour.ssh.exec", exec_args);
    let confirmation = crate::response::error_json(
        &guarded
            .call(
                "unfour_ssh_exec",
                json!({ "connectionId": "ssh-1", "command": "rm -rf /tmp/app" }),
            )
            .expect("guarded execute asks for confirmation"),
    );
    assert_eq!(confirmation["requires_confirmation"], true);
    assert_ne!(
        confirmation["error"]["code"],
        "MCP_TOOL_POLICY_UNCLASSIFIED"
    );
}

#[path = "output_schema.rs"]
mod output_schema;
