mod activity;
mod api;
mod confirmation;
mod database;
mod policy;
mod real;
mod ssh;
mod system;

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::response::{
    structured_confirmation_required, structured_policy_error, structured_tool_error,
    structured_tool_result,
};

use self::confirmation::ConfirmationRequired;
use self::policy::{evaluate_tool_policy, McpPolicyDenial};

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

    /// Mutates local Unfour metadata only.
    pub(super) const fn local_write() -> Self {
        Self {
            read_only_hint: false,
            destructive_hint: false,
            idempotent_hint: false,
            open_world_hint: false,
        }
    }

    /// Mutates local Unfour metadata in a way that removes or hides records.
    pub(super) const fn local_write_destructive() -> Self {
        Self {
            read_only_hint: false,
            destructive_hint: true,
            idempotent_hint: false,
            open_world_hint: false,
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
pub(crate) enum ToolCallError {
    UnknownTool(String),
    InvalidArguments(String),
    ConfirmationRequired(ConfirmationRequired),
    PolicyBlocked(McpPolicyDenial),
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

    pub(crate) fn call(&self, name: &str, arguments: Value) -> Result<Value, ToolCallError> {
        let started = std::time::Instant::now();
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.definition.name == name)
            .ok_or_else(|| ToolCallError::UnknownTool(name.to_string()))?;
        let policy = match evaluate_tool_policy(self.command_bus.as_ref(), name, &arguments) {
            Ok(policy) => policy,
            Err(error) => {
                return policy_or_execution_error(name, started.elapsed().as_millis(), error);
            }
        };
        let result = (tool.handler)(self.command_bus.as_ref(), arguments);

        match result {
            Ok(value) => Ok(structured_tool_result(
                name,
                &policy.workspace.environment_type,
                policy.risk.risk_level(),
                started.elapsed().as_millis(),
                value,
            )),
            Err(ToolCallError::ConfirmationRequired(confirmation)) => {
                Ok(structured_confirmation_required(
                    name,
                    &policy.workspace.environment_type,
                    confirmation.risk_level,
                    started.elapsed().as_millis(),
                    serde_json::to_value(confirmation).map_err(|_| ToolCallError::Execution {
                        code: "TOOL_RESULT_SERIALIZATION_FAILED",
                        message: "The tool result could not be serialized.",
                    })?,
                ))
            }
            Err(ToolCallError::PolicyBlocked(denial)) => Ok(structured_policy_error(
                name,
                &denial.environment_type,
                policy.risk.risk_level(),
                started.elapsed().as_millis(),
                serde_json::to_value(denial.clone()).map_err(|_| ToolCallError::Execution {
                    code: "TOOL_RESULT_SERIALIZATION_FAILED",
                    message: "The tool result could not be serialized.",
                })?,
            )),
            Err(ToolCallError::Execution { code, message }) => Ok(structured_tool_error(
                name,
                &policy.workspace.environment_type,
                policy.risk.risk_level(),
                started.elapsed().as_millis(),
                code,
                message,
            )),
            Err(error) => Err(error),
        }
    }
}

fn policy_or_execution_error(
    tool_name: &str,
    duration_ms: u128,
    error: ToolCallError,
) -> Result<Value, ToolCallError> {
    match error {
        ToolCallError::PolicyBlocked(denial) => Ok(structured_policy_error(
            tool_name,
            &denial.environment_type,
            denial_risk_level(&denial),
            duration_ms,
            serde_json::to_value(denial.clone()).map_err(|_| ToolCallError::Execution {
                code: "TOOL_RESULT_SERIALIZATION_FAILED",
                message: "The tool result could not be serialized.",
            })?,
        )),
        ToolCallError::Execution { code, message } => Ok(structured_tool_error(
            tool_name,
            "unknown",
            "medium",
            duration_ms,
            code,
            message,
        )),
        other => Err(other),
    }
}

fn denial_risk_level(denial: &McpPolicyDenial) -> &'static str {
    match denial.risk {
        "read" => "low",
        "write" | "execute" => "medium",
        _ => "high",
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
#[path = "tools_tests/mod.rs"]
mod tools_tests;
