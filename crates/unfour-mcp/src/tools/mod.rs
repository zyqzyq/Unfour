mod activity;
mod api;
mod database;
mod real;
mod ssh;
mod system;

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::response::{structured_tool_error, structured_tool_result};

type ToolHandler = fn(&dyn CommandBusAdapter, Value) -> Result<Value, ToolCallError>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub output_schema: Value,
    pub annotations: ToolAnnotations,
}

/// MCP tool behavior hints (`tools/list` `annotations`). They let a client
/// reason about safety without parsing descriptions: whether a tool mutates
/// state, and whether it reaches systems outside the local app data store.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    pub read_only_hint: bool,
    pub destructive_hint: bool,
    pub idempotent_hint: bool,
    pub open_world_hint: bool,
}

impl ToolAnnotations {
    /// Read-only against the local app-data store only (no external systems).
    pub(super) const fn local_read() -> Self {
        Self {
            read_only_hint: true,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: false,
        }
    }

    /// Read-only, but reaches an external system (a remote database or SSH host).
    pub(super) const fn remote_read() -> Self {
        Self {
            read_only_hint: true,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: true,
        }
    }

    /// Performs an external action with a side effect (e.g. sends an HTTP
    /// request and records history). Not destructive, but not idempotent.
    pub(super) const fn remote_action() -> Self {
        Self {
            read_only_hint: false,
            destructive_hint: false,
            idempotent_hint: false,
            open_world_hint: true,
        }
    }
}

struct RegisteredTool {
    definition: ToolDefinition,
    handler: ToolHandler,
}

pub struct ToolRegistry {
    tools: Vec<RegisteredTool>,
    command_bus: Arc<dyn CommandBusAdapter>,
}

#[derive(Debug, PartialEq)]
pub enum ToolCallError {
    UnknownTool(String),
    InvalidArguments(String),
    Execution {
        code: &'static str,
        message: &'static str,
    },
}

impl ToolRegistry {
    pub fn with_command_bus(command_bus: Arc<dyn CommandBusAdapter>) -> Self {
        let mut tools = real::registered_tools();
        tools.extend(api::registered_tools());
        tools.extend(database::registered_tools());
        tools.extend(system::registered_tools());
        tools.extend(activity::registered_tools());
        tools.extend(ssh::registered_tools());

        Self { tools, command_bus }
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| tool.definition.clone())
            .collect()
    }

    pub fn call(&self, name: &str, arguments: Value) -> Result<Value, ToolCallError> {
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.definition.name == name)
            .ok_or_else(|| ToolCallError::UnknownTool(name.to_string()))?;
        let result = (tool.handler)(self.command_bus.as_ref(), arguments);

        match result {
            Ok(value) => Ok(structured_tool_result(value)),
            Err(ToolCallError::Execution { code, message }) => {
                Ok(structured_tool_error(code, message))
            }
            Err(error) => Err(error),
        }
    }
}

pub(super) fn object_with_allowed_keys(
    arguments: Value,
    allowed_keys: &[&str],
) -> Result<Map<String, Value>, ToolCallError> {
    let object = arguments.as_object().ok_or_else(|| {
        ToolCallError::InvalidArguments("tool arguments must be a JSON object".to_string())
    })?;

    if let Some(key) = object
        .keys()
        .find(|key| !allowed_keys.contains(&key.as_str()))
    {
        return Err(ToolCallError::InvalidArguments(format!(
            "unexpected tool argument `{key}`"
        )));
    }

    Ok(object.clone())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use unfour_command_bus::{
        ApiCollectionListResult, ApiEnvironmentListResult, ApiHistoryDetailResult,
        ApiHistoryListResult, ApiRequestDetailResult, ApiRequestListResult, ConnectionListResult,
        CurrentWorkspaceResult, ReadCommand, ReadCommandResult, SafeConnection,
        SafeConnectionSummary, WorkspaceListResult, WorkspaceSummary,
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
                            last_opened_at: Some("2026-06-20T00:00:00Z".to_string()),
                        },
                        WorkspaceSummary {
                            id: "workspace-2".to_string(),
                            name: "Scratch".to_string(),
                            is_default: false,
                            is_active: false,
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
                            request_body: None,
                            status: Some(200),
                            duration_ms: Some(10),
                            response_headers_json: "[]".to_string(),
                            response_body_preview: None,
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
            })
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_DB_LIST_FAILED",
                message: "The command-bus database list operation failed.",
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
            })
        }

        fn execute_db_query(
            &self,
            _input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_DB_QUERY_FAILED",
                message: "The command-bus database query operation failed.",
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

        assert_eq!(definitions.len(), 18);
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
            .any(|definition| definition.name == "unfour.db.list_connections"));
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
            .any(|definition| definition.name == "unfour.ssh.run_diagnostic"));
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
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "COMMAND_BUS_READ_FAILED"
        );
        assert_eq!(
            result["structuredContent"]["error"]["message"],
            "The command-bus read operation failed."
        );
    }
}
