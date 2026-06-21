use serde_json::{json, Value};

use crate::command_bus_adapter::CommandBusAdapter;

use super::{object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition};

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![RegisteredTool {
        definition: ToolDefinition {
            name: "unfour.system.health",
            title: "Unfour System Health",
            description:
                "Returns command-bus and storage readiness for diagnostics through the Unfour command bus.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "appName": { "type": "string" },
                    "storageReady": { "type": "boolean" },
                    "commandBusReady": { "type": "boolean" },
                    "aiReservedCapabilities": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "syncStrategy": { "type": "string" },
                    "source": { "type": "string", "const": "command-bus" }
                },
                "required": ["appName", "storageReady", "commandBusReady", "source"],
                "additionalProperties": false
            }),
            annotations: ToolAnnotations::local_read(),
        },
        handler: system_health,
    }]
}

fn system_health(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    object_with_allowed_keys(arguments, &[])?;

    let health = command_bus
        .system_health()
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    Ok(json!({
        "appName": health.app_name,
        "storageReady": health.storage_ready,
        "commandBusReady": health.command_bus_ready,
        "aiReservedCapabilities": health.ai_reserved_capabilities,
        "syncStrategy": health.sync_strategy,
        "source": "command-bus"
    }))
}
