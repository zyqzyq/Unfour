use super::*;

pub(super) fn api_set_environment_variable(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "environmentId",
            "key",
            "value",
            "enabled",
            "isSecret",
            "description",
        ],
    )?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let environment_id = parse_required_string(
        &arguments,
        "environmentId",
        "unfour.api.set_environment_variable",
    )?;
    let key = parse_required_string(&arguments, "key", "unfour.api.set_environment_variable")?;
    let value =
        parse_required_value_string(&arguments, "value", "unfour.api.set_environment_variable")?;
    let environment = get_workspace_environment(command_bus, &workspace_id, &environment_id)?;
    let existing = environment
        .variables
        .iter()
        .find(|variable| variable.key.eq_ignore_ascii_case(&key));
    let description = match parse_description_override(&arguments)? {
        Some(description) => description,
        None => existing.and_then(|variable| variable.description.clone()),
    };
    let is_secret = parse_optional_bool(&arguments, "isSecret")?.unwrap_or_else(|| {
        existing
            .map(|variable| variable.is_secret)
            .unwrap_or_else(|| is_sensitive_key(&key))
    });
    let is_enabled = parse_optional_bool(&arguments, "enabled")?.unwrap_or(true);
    let sort_order = existing
        .map(|variable| variable.sort_order)
        .unwrap_or_else(|| {
            environment
                .variables
                .iter()
                .map(|variable| variable.sort_order)
                .max()
                .unwrap_or(-1)
                .saturating_add(1)
        });
    let input = WorkspaceVariableInput {
        id: existing.map(|variable| variable.id.clone()),
        key,
        value,
        is_secret,
        is_enabled,
        description,
        sort_order,
    };
    let (variable, created) = match existing {
        Some(existing) => (
            command_bus.update_api_environment_variable(
                &workspace_id,
                &environment_id,
                &existing.id,
                input,
            ),
            false,
        ),
        None => (
            command_bus.create_api_environment_variable(&workspace_id, &environment_id, input),
            true,
        ),
    };
    let variable = variable.map_err(|error| ToolCallError::Execution {
        code: error.code,
        message: error.message,
    })?;

    Ok(json!({
        "variable": safe_environment_variable(&variable),
        "created": created,
        "source": "command-bus"
    }))
}

pub(super) fn api_delete_environment_variable(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "environmentId",
            "key",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let environment_id = parse_required_string(
        &arguments,
        "environmentId",
        "unfour.api.delete_environment_variable",
    )?;
    let key = parse_required_string(&arguments, "key", "unfour.api.delete_environment_variable")?;
    let environment = get_workspace_environment(command_bus, &workspace_id, &environment_id)?;
    let variable = environment
        .variables
        .iter()
        .find(|variable| variable.key.eq_ignore_ascii_case(&key))
        .ok_or(ToolCallError::Execution {
            code: "NOT_FOUND",
            message: "The requested API environment variable was not found.",
        })?;
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "API_DELETE_ENVIRONMENT_VARIABLE",
        "Deleting an API environment variable removes local metadata. Confirmation is required.",
        json!({
            "tool": "unfour.api.delete_environment_variable",
            "workspaceId": workspace_id,
            "environmentId": environment_id,
            "key": key
        }),
    )?;
    command_bus
        .delete_api_environment_variable(&workspace_id, &environment_id, &variable.id)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    Ok(json!({
        "deleted": true,
        "environmentId": environment_id,
        "key": key,
        "source": "command-bus"
    }))
}

fn get_workspace_environment(
    command_bus: &dyn CommandBusAdapter,
    workspace_id: &str,
    environment_id: &str,
) -> Result<WorkspaceEnvironment, ToolCallError> {
    command_bus
        .list_workspace_environments(workspace_id)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?
        .into_iter()
        .find(|environment| environment.id == environment_id)
        .ok_or(ToolCallError::Execution {
            code: "NOT_FOUND",
            message: "The requested API environment was not found in this workspace.",
        })
}

fn parse_required_value_string(
    arguments: &Map<String, Value>,
    key: &str,
    tool_name: &str,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(value)) => Ok(value.clone()),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "{} requires string argument `{}`",
            tool_name, key
        ))),
    }
}

fn parse_description_override(
    arguments: &Map<String, Value>,
) -> Result<Option<Option<String>>, ToolCallError> {
    match arguments.get("description") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(description)) => Ok(Some(Some(description.clone()))),
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `description` must be a string or null".to_string(),
        )),
    }
}

fn safe_environment_variable(variable: &WorkspaceEnvironmentVariable) -> Value {
    let value = if variable.is_secret || is_sensitive_key(&variable.key) {
        mask_secret(&variable.value)
    } else {
        variable.value.clone()
    };
    json!({
        "id": variable.id,
        "environmentId": variable.environment_id,
        "key": variable.key,
        "value": value,
        "enabled": variable.is_enabled,
        "isSecret": variable.is_secret,
        "description": variable.description
    })
}
