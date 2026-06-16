use serde_json::{json, Map, Value};
use unfour_command_bus::{ConnectionType, ReadCommand, ReadCommandResult};

use crate::command_bus_adapter::CommandBusAdapter;

use super::{object_with_allowed_keys, RegisteredTool, ToolCallError, ToolDefinition, ToolHandler};

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.workspace.current",
                title: "Current Unfour Workspace",
                description:
                    "Returns the active local workspace through the Unfour command bus.",
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "workspaceName": { "type": "string" },
                        "workspaceRoot": { "type": ["string", "null"] },
                        "mode": { "type": "string", "const": "local" },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": [
                        "workspaceId",
                        "workspaceName",
                        "workspaceRoot",
                        "mode",
                        "source"
                    ],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Real(workspace_current),
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.connection.list",
                title: "List Unfour Connections",
                description:
                    "Lists safe connection summaries for the active workspace through the Unfour command bus.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["all", "api", "database", "ssh"],
                            "default": "all",
                            "description": "Optional connection type filter."
                        }
                    },
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "connections": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "name": { "type": "string" },
                                    "type": {
                                        "type": "string",
                                        "enum": ["ssh", "database", "api"]
                                    },
                                    "workspaceId": { "type": "string" },
                                    "safeSummary": {
                                        "type": "object",
                                        "properties": {
                                            "host": { "type": "string" },
                                            "databaseType": { "type": "string" },
                                            "apiBaseUrl": { "type": "string" }
                                        },
                                        "additionalProperties": false
                                    }
                                },
                                "required": ["id", "name", "type", "workspaceId", "safeSummary"],
                                "additionalProperties": false
                            }
                        },
                        "count": { "type": "integer", "minimum": 0 },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["connections", "count", "source"],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Real(connection_list),
        },
    ]
}

fn workspace_current(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    object_with_allowed_keys(arguments, &[])?;
    let result = command_bus
        .execute_read(ReadCommand::CurrentWorkspace)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let ReadCommandResult::CurrentWorkspace(workspace) = result else {
        return Err(unexpected_result());
    };

    serialize_safe(workspace)
}

fn connection_list(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["type"])?;
    let connection_type = parse_connection_type(&arguments)?;
    let result = command_bus
        .execute_read(ReadCommand::ListConnections { connection_type })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let ReadCommandResult::Connections(connections) = result else {
        return Err(unexpected_result());
    };

    serialize_safe(connections)
}

fn parse_connection_type(arguments: &Map<String, Value>) -> Result<ConnectionType, ToolCallError> {
    match arguments.get("type") {
        None => Ok(ConnectionType::All),
        Some(Value::String(value)) => match value.as_str() {
            "all" => Ok(ConnectionType::All),
            "api" => Ok(ConnectionType::Api),
            "database" => Ok(ConnectionType::Database),
            "ssh" => Ok(ConnectionType::Ssh),
            _ => Err(ToolCallError::InvalidArguments(
                "unfour.connection.list argument `type` must be one of: all, api, database, ssh"
                    .to_string(),
            )),
        },
        Some(_) => Err(ToolCallError::InvalidArguments(
            "unfour.connection.list argument `type` must be a string".to_string(),
        )),
    }
}

fn serialize_safe(value: impl serde::Serialize) -> Result<Value, ToolCallError> {
    let mut value = serde_json::to_value(value).map_err(|_| ToolCallError::Execution {
        code: "TOOL_RESULT_SERIALIZATION_FAILED",
        message: "The tool result could not be serialized.",
    })?;
    remove_sensitive_fields(&mut value);
    Ok(value)
}

fn remove_sensitive_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, _| !is_sensitive_key(key));
            for value in object.values_mut() {
                remove_sensitive_fields(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                remove_sensitive_fields(value);
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().replace(['-', '_'], "").as_str(),
        "password"
            | "token"
            | "privatekey"
            | "authorization"
            | "cookie"
            | "connectionstring"
            | "proxyauthorization"
            | "xapikey"
            | "xauthtoken"
            | "credentialref"
    )
}

fn unexpected_result() -> ToolCallError {
    ToolCallError::Execution {
        code: "COMMAND_BUS_RESULT_MISMATCH",
        message: "The command-bus returned an unexpected result.",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::remove_sensitive_fields;

    #[test]
    fn sensitive_fields_are_removed_recursively() {
        let mut value = json!({
            "safe": "visible",
            "password": "secret",
            "token": "secret",
            "privateKey": "secret",
            "authorization": "secret",
            "cookie": "secret",
            "connectionString": "secret",
            "nested": [{
                "x-api-key": "secret",
                "host": "localhost"
            }]
        });

        remove_sensitive_fields(&mut value);

        let serialized = serde_json::to_string(&value).unwrap();
        for forbidden in [
            "password",
            "token",
            "privateKey",
            "authorization",
            "cookie",
            "connectionString",
            "x-api-key",
        ] {
            assert!(!serialized.contains(forbidden));
        }
        assert_eq!(value["safe"], "visible");
        assert_eq!(value["nested"][0]["host"], "localhost");
    }
}
