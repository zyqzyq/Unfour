mod api;
mod database;
mod mock;
mod real;

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::response::{structured_tool_error, structured_tool_result};

type MockToolHandler = fn(Value) -> Result<Value, ToolCallError>;
type RealToolHandler = fn(&dyn CommandBusAdapter, Value) -> Result<Value, ToolCallError>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub output_schema: Value,
}

struct RegisteredTool {
    definition: ToolDefinition,
    handler: ToolHandler,
}

#[derive(Clone, Copy)]
enum ToolHandler {
    Mock(MockToolHandler),
    Real(RealToolHandler),
}

pub struct ToolRegistry {
    tools: Vec<RegisteredTool>,
    command_bus: Option<Arc<dyn CommandBusAdapter>>,
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
    pub fn mock() -> Self {
        Self {
            tools: mock::registered_tools(),
            command_bus: None,
        }
    }

    pub fn with_command_bus(command_bus: Arc<dyn CommandBusAdapter>) -> Self {
        let mut tools = mock::registered_tools();
        tools.extend(real::registered_tools());
        tools.extend(api::registered_tools());
        tools.extend(database::registered_tools());

        Self {
            tools,
            command_bus: Some(command_bus),
        }
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
        let result = match tool.handler {
            ToolHandler::Mock(handler) => handler(arguments),
            ToolHandler::Real(handler) => {
                let command_bus = self
                    .command_bus
                    .as_deref()
                    .ok_or(ToolCallError::Execution {
                        code: "COMMAND_BUS_UNAVAILABLE",
                        message: "The command-bus adapter is unavailable.",
                    })?;
                handler(command_bus, arguments)
            }
        };

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
        ApiCollectionListResult, ApiRequestDetailResult, ApiRequestListResult,
        ConnectionListResult, CurrentWorkspaceResult, ReadCommand, ReadCommandResult,
        SafeConnection, SafeConnectionSummary,
    };
    use unfour_core::models::{
        ApiResponse, ApiSavedRequest, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult,
        DatabaseQuerySafety, DatabaseSchema, KeyValue,
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
                            folder_path: None,
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
    fn mock_tool_schemas_are_available() {
        let definitions = ToolRegistry::mock().definitions();

        assert_eq!(definitions.len(), 3);
        assert!(definitions
            .iter()
            .all(|definition| definition.name.starts_with("unfour.mock.")));
        assert!(definitions
            .iter()
            .all(|definition| definition.input_schema["type"] == "object"));
    }

    #[test]
    fn mock_echo_returns_structured_json() {
        let result = ToolRegistry::mock()
            .call("unfour.mock.echo", json!({ "value": "anything" }))
            .expect("mock echo should succeed");

        assert_eq!(
            result["structuredContent"],
            json!({
                "ok": true,
                "value": "anything"
            })
        );
        assert_eq!(result["isError"], false);
    }

    #[test]
    fn real_tool_schemas_are_available_separately_from_mocks() {
        let definitions = ToolRegistry::with_command_bus(Arc::new(StubCommandBus)).definitions();

        assert_eq!(definitions.len(), 13);
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
