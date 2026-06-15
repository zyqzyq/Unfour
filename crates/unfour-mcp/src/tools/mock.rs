use serde_json::{json, Value};

use super::{object_with_allowed_keys, RegisteredTool, ToolCallError, ToolDefinition, ToolHandler};

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.mock.ping",
                title: "Mock Ping",
                description:
                    "Returns a deterministic pong response without calling Unfour services.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "Message to include in the mock response."
                        }
                    },
                    "required": ["message"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "message": { "type": "string" },
                        "echo": { "type": "string" }
                    },
                    "required": ["ok", "message", "echo"],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Mock(ping),
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.mock.workspace_current",
                title: "Mock Current Workspace",
                description:
                    "Returns fixed mock workspace metadata without reading workspace state.",
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
                        "mode": { "type": "string" }
                    },
                    "required": ["workspaceId", "workspaceName", "mode"],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Mock(workspace_current),
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.mock.echo",
                title: "Mock Echo",
                description: "Returns the provided JSON value without calling Unfour services.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "value": {
                            "description": "Any JSON value to echo."
                        }
                    },
                    "required": ["value"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "value": {}
                    },
                    "required": ["ok", "value"],
                    "additionalProperties": false
                }),
            },
            handler: ToolHandler::Mock(echo),
        },
    ]
}

fn ping(arguments: Value) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["message"])?;
    let message = arguments
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolCallError::InvalidArguments(
                "unfour.mock.ping requires string argument `message`".to_string(),
            )
        })?;

    Ok(json!({
        "ok": true,
        "message": "pong",
        "echo": message,
    }))
}

fn workspace_current(arguments: Value) -> Result<Value, ToolCallError> {
    object_with_allowed_keys(arguments, &[])?;

    Ok(json!({
        "workspaceId": "mock-workspace",
        "workspaceName": "Mock Workspace",
        "mode": "mock",
    }))
}

fn echo(arguments: Value) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["value"])?;
    let value = arguments.get("value").cloned().ok_or_else(|| {
        ToolCallError::InvalidArguments(
            "unfour.mock.echo requires JSON argument `value`".to_string(),
        )
    })?;

    Ok(json!({
        "ok": true,
        "value": value,
    }))
}
