use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::sanitize::redact_json_in_place;

use super::{object_with_allowed_keys, RegisteredTool, ToolCallError, ToolDefinition, ToolHandler};

const DEFAULT_ACTIVITY_LIMIT: i64 = 50;
const MAX_ACTIVITY_LIMIT: i64 = 200;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![RegisteredTool {
        definition: ToolDefinition {
            name: "unfour.activity.list",
            title: "List Workspace Activity",
            description:
                "Lists recent workspace activity events (workspace, connection, API, database, and SSH changes) through the Unfour command bus, newest first. Sensitive fields in event details are masked. Useful for diagnosing what changed before a failure started.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspaceId": {
                        "type": "string",
                        "description": "Optional workspace ID. Uses the active workspace if omitted."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of events to return (default 50, max 200)."
                    }
                },
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "activity": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "workspaceId": { "type": ["string", "null"] },
                                "action": { "type": "string" },
                                "target": { "type": ["string", "null"] },
                                "details": {},
                                "createdAt": { "type": "string" }
                            },
                            "required": ["id", "action", "details", "createdAt"],
                            "additionalProperties": false
                        }
                    },
                    "count": { "type": "integer", "minimum": 0 },
                    "source": { "type": "string", "const": "command-bus" }
                },
                "required": ["activity", "count", "source"],
                "additionalProperties": false
            }),
        },
        handler: ToolHandler::Real(activity_list),
    }]
}

fn activity_list(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "limit"])?;
    let workspace_id = parse_optional_string(&arguments, "workspaceId")?;
    let limit = parse_optional_limit(&arguments)?;

    let result = command_bus
        .execute_read(ReadCommand::ListActivity {
            workspace_id,
            limit,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    let ReadCommandResult::Activity(activity) = result else {
        return Err(unexpected_result());
    };

    let items: Vec<Value> = activity
        .activity
        .into_iter()
        .map(|item| {
            // Defense-in-depth: stored details are already redacted summaries, but
            // mask again before the payload reaches a potentially cloud-hosted LLM.
            let mut details = item.details;
            redact_json_in_place(&mut details);
            json!({
                "id": item.id,
                "workspaceId": item.workspace_id,
                "action": item.action,
                "target": item.target,
                "details": details,
                "createdAt": item.created_at
            })
        })
        .collect();

    Ok(json!({
        "activity": items,
        "count": activity.count,
        "source": "command-bus"
    }))
}

fn parse_optional_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a string",
            key
        ))),
    }
}

fn parse_optional_limit(arguments: &Map<String, Value>) -> Result<Option<i64>, ToolCallError> {
    match arguments.get("limit") {
        None => Ok(Some(DEFAULT_ACTIVITY_LIMIT)),
        Some(Value::Number(n)) => {
            let value = n.as_i64().ok_or_else(|| {
                ToolCallError::InvalidArguments(
                    "argument `limit` must be a positive integer".to_string(),
                )
            })?;
            Ok(Some(value.clamp(1, MAX_ACTIVITY_LIMIT)))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `limit` must be a number".to_string(),
        )),
    }
}

fn unexpected_result() -> ToolCallError {
    ToolCallError::Execution {
        code: "COMMAND_BUS_RESULT_MISMATCH",
        message: "The command-bus returned an unexpected result.",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use unfour_command_bus::{ActivityItem, ActivityListResult, ReadCommand, ReadCommandResult};
    use unfour_core::models::{
        ApiResponse, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult,
        DatabaseQuerySafety, DatabaseSchema,
    };

    use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
    use crate::tools::ToolRegistry;

    struct ActivityStubCommandBus;

    impl CommandBusAdapter for ActivityStubCommandBus {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            match command {
                ReadCommand::ListActivity { limit, .. } => {
                    // Verify the command-bus default limit is forwarded.
                    assert_eq!(limit, Some(50));
                    Ok(ReadCommandResult::Activity(ActivityListResult {
                        activity: vec![
                            ActivityItem {
                                id: "evt-2".to_string(),
                                workspace_id: Some("ws-1".to_string()),
                                action: "database.connection.create".to_string(),
                                target: Some("conn-1".to_string()),
                                details: json!({ "name": "Prod DB", "password": "supersecret" }),
                                created_at: "2026-06-20T01:00:00Z".to_string(),
                            },
                            ActivityItem {
                                id: "evt-1".to_string(),
                                workspace_id: Some("ws-1".to_string()),
                                action: "workspace.create".to_string(),
                                target: Some("ws-1".to_string()),
                                details: json!({ "name": "Local" }),
                                created_at: "2026-06-20T00:00:00Z".to_string(),
                            },
                        ],
                        count: 2,
                        source: "command-bus".to_string(),
                    }))
                }
                _ => Err(CommandBusAdapterError {
                    code: "UNEXPECTED",
                    message: "unexpected command",
                }),
            }
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

    struct FailingActivityCommandBus;

    impl CommandBusAdapter for FailingActivityCommandBus {
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
            unreachable!()
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
            unreachable!()
        }
    }

    fn registry() -> ToolRegistry {
        ToolRegistry::with_command_bus(Arc::new(ActivityStubCommandBus))
    }

    #[test]
    fn activity_tool_is_registered() {
        assert!(registry()
            .definitions()
            .iter()
            .any(|d| d.name == "unfour.activity.list"));
    }

    #[test]
    fn list_activity_returns_newest_first_with_masked_details() {
        let result = registry()
            .call("unfour.activity.list", json!({}))
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        let content = &result["structuredContent"];
        assert_eq!(content["count"], 2);
        assert_eq!(content["source"], "command-bus");

        let items = content["activity"].as_array().unwrap();
        assert_eq!(items[0]["action"], "database.connection.create");
        assert_eq!(items[1]["action"], "workspace.create");

        // Sensitive detail field masked; non-sensitive preserved.
        let masked = items[0]["details"]["password"].as_str().unwrap();
        assert!(masked.starts_with("[mask "));
        assert!(!result.to_string().contains("supersecret"));
        assert_eq!(items[0]["details"]["name"], "Prod DB");
    }

    #[test]
    fn list_activity_rejects_unknown_argument() {
        let result = registry().call("unfour.activity.list", json!({ "bogus": 1 }));
        assert!(result.is_err());
    }

    #[test]
    fn list_activity_returns_structured_error_on_failure() {
        let registry = ToolRegistry::with_command_bus(Arc::new(FailingActivityCommandBus));
        let result = registry
            .call("unfour.activity.list", json!({}))
            .expect("execution errors become MCP tool results");

        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "COMMAND_BUS_READ_FAILED"
        );
    }
}
