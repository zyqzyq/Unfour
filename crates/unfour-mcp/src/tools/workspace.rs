use serde_json::{json, Map, Value};
use unfour_core::models::{WorkspaceVariable, WorkspaceVariableInput};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::sanitize::{is_sensitive_key, mask_secret};

use super::confirmation::ensure_confirmed_if_guarded;
use super::policy::ToolPolicyEvaluation;
use super::{
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
};

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.workspace.list_variables",
                title: "List Workspace Variables",
                description: "Lists workspace-global variables through the command bus. Secret variables and sensitive keys are masked before returning to the MCP client.",
                input_schema: workspace_only_schema(),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: list_variables,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.workspace.replace_variables",
                title: "Replace Workspace Variables",
                description: "Replaces the complete workspace-global variable set through the command bus. Omitted existing variables are soft-deleted. For items that include an existing id, omitted optional fields keep their current values. Returned values are masked when secret or sensitive.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "variables": { "type": "array", "items": variable_input_schema() }
                    },
                    "required": ["variables"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: replace_variables,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.workspace.create_variable",
                title: "Create Workspace Variable",
                description: "Creates one workspace-global variable through the command bus. Returned values are masked when secret or sensitive.",
                input_schema: variable_mutation_schema(false),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: create_variable,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.workspace.update_variable",
                title: "Update Workspace Variable",
                description: "Updates one workspace-global variable through the command bus. Omitted optional fields keep their current values. Returned values are masked when secret or sensitive.",
                input_schema: variable_mutation_schema(true),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: update_variable,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.workspace.delete_variable",
                title: "Delete Workspace Variable",
                description: "Soft-deletes one workspace-global variable. Guarded policy requires the content-bound confirmation handshake.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "variableId": { "type": "string" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["variableId"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write_destructive(),
            },
            handler: delete_variable,
        },
    ]
}

fn list_variables(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    object_with_allowed_keys(arguments, &["workspaceId"])?;
    let variables = command_bus
        .list_workspace_variables(&evaluation.workspace.workspace_id)
        .map_err(execution_error)?;
    Ok(variable_list_result(&variables))
}

fn replace_variables(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "variables"])?;
    let existing = command_bus
        .list_workspace_variables(&evaluation.workspace.workspace_id)
        .map_err(execution_error)?;
    let variables = parse_variable_list(arguments.get("variables"), &existing)?;
    let variables = command_bus
        .replace_workspace_variables(&evaluation.workspace.workspace_id, variables)
        .map_err(execution_error)?;
    Ok(variable_list_result(&variables))
}

fn create_variable(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "key",
            "value",
            "isSecret",
            "isEnabled",
            "description",
            "sortOrder",
        ],
    )?;
    let input = parse_variable_input(&arguments, None)?;
    let variable = command_bus
        .create_workspace_variable(&evaluation.workspace.workspace_id, input)
        .map_err(execution_error)?;
    Ok(json!({ "variable": safe_variable(&variable), "source": "command-bus" }))
}

fn update_variable(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "variableId",
            "key",
            "value",
            "isSecret",
            "isEnabled",
            "description",
            "sortOrder",
        ],
    )?;
    let variable_id = required_string(&arguments, "variableId")?;
    let existing = find_workspace_variable(
        command_bus,
        &evaluation.workspace.workspace_id,
        &variable_id,
    )?;
    let input = parse_variable_input(&arguments, Some(&existing))?;
    let variable = command_bus
        .update_workspace_variable(&evaluation.workspace.workspace_id, &variable_id, input)
        .map_err(execution_error)?;
    Ok(json!({ "variable": safe_variable(&variable), "source": "command-bus" }))
}

fn delete_variable(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "variableId",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let variable_id = required_string(&arguments, "variableId")?;
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "WORKSPACE_DELETE_VARIABLE",
        "Deleting a workspace variable hides local configuration and requires confirmation under guarded policy.",
        json!({
            "tool": "unfour.workspace.delete_variable",
            "workspaceId": evaluation.workspace.workspace_id,
            "variableId": variable_id
        }),
    )?;
    let variables = command_bus
        .delete_workspace_variable(&evaluation.workspace.workspace_id, &variable_id)
        .map_err(execution_error)?;
    Ok(json!({
        "deleted": true,
        "variableId": variable_id,
        "remainingCount": variables.len(),
        "source": "command-bus"
    }))
}

fn workspace_only_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "workspaceId": { "type": "string" } },
        "additionalProperties": false
    })
}

fn variable_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "key": { "type": "string" },
            "value": { "type": "string" },
            "isSecret": { "type": "boolean" },
            "isEnabled": { "type": "boolean" },
            "description": { "type": ["string", "null"] },
            "sortOrder": { "type": "integer" }
        },
        "required": ["key", "value"],
        "additionalProperties": false
    })
}

fn variable_mutation_schema(update: bool) -> Value {
    let mut schema = variable_input_schema();
    let properties = schema["properties"].as_object_mut().expect("object schema");
    properties.insert("workspaceId".to_string(), json!({ "type": "string" }));
    properties.remove("id");
    if update {
        properties.insert("variableId".to_string(), json!({ "type": "string" }));
        schema["required"] = json!(["variableId", "key", "value"]);
    }
    schema
}

fn parse_variable_list(
    value: Option<&Value>,
    existing: &[WorkspaceVariable],
) -> Result<Vec<WorkspaceVariableInput>, ToolCallError> {
    let Some(Value::Array(items)) = value else {
        return Err(ToolCallError::InvalidArguments(
            "argument `variables` must be an array of workspace variable objects".to_string(),
        ));
    };
    items
        .iter()
        .map(|item| {
            let Some(object) = item.as_object() else {
                return Err(ToolCallError::InvalidArguments(
                    "argument `variables` contains an invalid workspace variable".to_string(),
                ));
            };
            let id = match object.get("id") {
                None | Some(Value::Null) => None,
                Some(Value::String(value)) if !value.trim().is_empty() => {
                    Some(value.trim().to_string())
                }
                _ => {
                    return Err(ToolCallError::InvalidArguments(
                        "argument `variables[].id` must be a non-empty string when provided"
                            .to_string(),
                    ))
                }
            };
            let baseline = id
                .as_ref()
                .and_then(|id| existing.iter().find(|variable| variable.id == *id));
            let mut input = parse_variable_input(object, baseline)?;
            input.id = id;
            Ok(input)
        })
        .collect()
}

fn parse_variable_input(
    arguments: &Map<String, Value>,
    existing: Option<&WorkspaceVariable>,
) -> Result<WorkspaceVariableInput, ToolCallError> {
    let key = required_string(arguments, "key")?;
    let value = required_string_allow_empty(arguments, "value")?;
    let description = match arguments.get("description") {
        None => existing.and_then(|variable| variable.description.clone()),
        Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        _ => {
            return Err(ToolCallError::InvalidArguments(
                "argument `description` must be a string or null".to_string(),
            ))
        }
    };
    Ok(WorkspaceVariableInput {
        id: None,
        key,
        value,
        is_secret: optional_bool(
            arguments,
            "isSecret",
            existing.map(|variable| variable.is_secret).unwrap_or(false),
        )?,
        is_enabled: optional_bool(
            arguments,
            "isEnabled",
            existing.map(|variable| variable.is_enabled).unwrap_or(true),
        )?,
        description,
        sort_order: optional_i64(
            arguments,
            "sortOrder",
            existing.map(|variable| variable.sort_order).unwrap_or(0),
        )?,
    })
}

fn find_workspace_variable(
    command_bus: &dyn CommandBusAdapter,
    workspace_id: &str,
    variable_id: &str,
) -> Result<WorkspaceVariable, ToolCallError> {
    command_bus
        .list_workspace_variables(workspace_id)
        .map_err(execution_error)?
        .into_iter()
        .find(|variable| variable.id == variable_id)
        .ok_or(ToolCallError::Execution {
            code: "WORKSPACE_VARIABLE_NOT_FOUND",
            message: "The requested workspace variable was not found.",
        })
}

fn safe_variable(variable: &WorkspaceVariable) -> Value {
    let value = if variable.is_secret || is_sensitive_key(&variable.key) {
        mask_secret(&variable.value)
    } else {
        variable.value.clone()
    };
    json!({
        "id": variable.id,
        "workspaceId": variable.workspace_id,
        "key": variable.key,
        "value": value,
        "isSecret": variable.is_secret,
        "isEnabled": variable.is_enabled,
        "description": variable.description,
        "sortOrder": variable.sort_order,
        "revision": variable.revision
    })
}

fn variable_list_result(variables: &[WorkspaceVariable]) -> Value {
    json!({
        "variables": variables.iter().map(safe_variable).collect::<Vec<_>>(),
        "count": variables.len(),
        "source": "command-bus"
    })
}

fn required_string(arguments: &Map<String, Value>, key: &str) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a non-empty string"
        ))),
    }
}

fn required_string_allow_empty(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(value)) => Ok(value.clone()),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a string"
        ))),
    }
}

fn optional_bool(
    arguments: &Map<String, Value>,
    key: &str,
    default: bool,
) -> Result<bool, ToolCallError> {
    match arguments.get(key) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a boolean"
        ))),
    }
}

fn optional_i64(
    arguments: &Map<String, Value>,
    key: &str,
    default: i64,
) -> Result<i64, ToolCallError> {
    match arguments.get(key) {
        None => Ok(default),
        Some(Value::Number(value)) => value.as_i64().ok_or_else(|| {
            ToolCallError::InvalidArguments(format!("argument `{key}` must be an integer"))
        }),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be an integer"
        ))),
    }
}

fn execution_error(error: crate::command_bus_adapter::CommandBusAdapterError) -> ToolCallError {
    ToolCallError::Execution {
        code: error.code,
        message: error.message,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use unfour_command_bus::{CurrentWorkspaceResult, ReadCommand, ReadCommandResult};

    use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
    use crate::tools::ToolRegistry;

    use super::*;

    struct WorkspaceVariableStub;

    impl CommandBusAdapter for WorkspaceVariableStub {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            assert_eq!(command, ReadCommand::CurrentWorkspace);
            Ok(ReadCommandResult::CurrentWorkspace(
                CurrentWorkspaceResult {
                    workspace_id: "workspace-1".to_string(),
                    workspace_name: "Development".to_string(),
                    environment_type: "dev".to_string(),
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
        ) -> Result<unfour_core::models::ApiResponse, CommandBusAdapterError> {
            unreachable!("not used by workspace variable tests")
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<unfour_core::models::DatabaseConnection>, CommandBusAdapterError> {
            unreachable!("not used by workspace variable tests")
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            _connection_id: &str,
        ) -> Result<unfour_core::models::DatabaseSchema, CommandBusAdapterError> {
            unreachable!("not used by workspace variable tests")
        }

        fn execute_db_query(
            &self,
            _input: unfour_core::models::DatabaseQueryInput,
        ) -> Result<unfour_core::models::DatabaseQueryResult, CommandBusAdapterError> {
            unreachable!("not used by workspace variable tests")
        }

        fn list_workspace_variables(
            &self,
            workspace_id: &str,
        ) -> Result<Vec<WorkspaceVariable>, CommandBusAdapterError> {
            assert_eq!(workspace_id, "workspace-1");
            Ok(vec![
                variable("public", "BASE_URL", "https://example.test", false),
                variable("secret", "password", "plain-secret", false),
                variable("marked", "innocent", "marked-secret", true),
            ])
        }

        fn create_workspace_variable(
            &self,
            workspace_id: &str,
            input: WorkspaceVariableInput,
        ) -> Result<WorkspaceVariable, CommandBusAdapterError> {
            assert_eq!(workspace_id, "workspace-1");
            Ok(variable(
                "created",
                &input.key,
                &input.value,
                input.is_secret,
            ))
        }

        fn update_workspace_variable(
            &self,
            workspace_id: &str,
            variable_id: &str,
            input: WorkspaceVariableInput,
        ) -> Result<WorkspaceVariable, CommandBusAdapterError> {
            assert_eq!(workspace_id, "workspace-1");
            assert_eq!(variable_id, "marked");
            assert!(input.is_secret);
            assert!(input.is_enabled);
            assert_eq!(input.description.as_deref(), Some("kept"));
            assert_eq!(input.sort_order, 7);
            Ok(WorkspaceVariable {
                id: variable_id.to_string(),
                workspace_id: workspace_id.to_string(),
                key: input.key,
                value: input.value,
                is_secret: input.is_secret,
                is_enabled: input.is_enabled,
                description: input.description,
                sort_order: input.sort_order,
                created_at: String::new(),
                updated_at: String::new(),
                deleted_at: None,
                revision: 2,
            })
        }
    }

    fn variable(id: &str, key: &str, value: &str, is_secret: bool) -> WorkspaceVariable {
        WorkspaceVariable {
            id: id.to_string(),
            workspace_id: "workspace-1".to_string(),
            key: key.to_string(),
            value: value.to_string(),
            is_secret,
            is_enabled: true,
            description: if id == "marked" {
                Some("kept".to_string())
            } else {
                None
            },
            sort_order: if id == "marked" { 7 } else { 0 },
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
            revision: 1,
        }
    }

    #[test]
    fn list_masks_sensitive_keys_and_explicit_secrets() {
        let registry = ToolRegistry::with_command_bus(Arc::new(WorkspaceVariableStub));
        let result = registry
            .call("unfour.workspace.list_variables", json!({}))
            .unwrap();

        assert_eq!(result["isError"], false);
        assert_eq!(
            result["structuredContent"]["variables"][0]["value"],
            "https://example.test"
        );
        assert!(result["structuredContent"]["variables"][1]["value"]
            .as_str()
            .unwrap()
            .starts_with("[mask"));
        assert!(result["structuredContent"]["variables"][2]["value"]
            .as_str()
            .unwrap()
            .starts_with("[mask"));
        assert!(!result.to_string().contains("plain-secret"));
        assert!(!result.to_string().contains("marked-secret"));
    }

    #[test]
    fn create_routes_through_adapter_and_masks_result() {
        let registry = ToolRegistry::with_command_bus(Arc::new(WorkspaceVariableStub));
        let result = registry
            .call(
                "unfour.workspace.create_variable",
                json!({ "key": "TOKEN", "value": "raw-token", "isEnabled": true }),
            )
            .unwrap();

        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["variable"]["key"], "TOKEN");
        assert!(!result.to_string().contains("raw-token"));
    }

    #[test]
    fn update_preserves_omitted_optional_fields() {
        let registry = ToolRegistry::with_command_bus(Arc::new(WorkspaceVariableStub));
        let result = registry
            .call(
                "unfour.workspace.update_variable",
                json!({
                    "variableId": "marked",
                    "key": "innocent",
                    "value": "rotated-secret"
                }),
            )
            .unwrap();

        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["variable"]["isSecret"], true);
        assert_eq!(
            result["structuredContent"]["variable"]["description"],
            "kept"
        );
        assert_eq!(result["structuredContent"]["variable"]["sortOrder"], 7);
        assert!(!result.to_string().contains("rotated-secret"));
    }
}
