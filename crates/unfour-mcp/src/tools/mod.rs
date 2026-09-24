mod activity;
mod api;
mod confirmation;
mod connection_diagnostics;
mod database;
mod flow;
mod policy;
mod real;
mod ssh;
mod ssh_risk;
mod system;
mod workspace;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::response::{
    structured_confirmation_required, structured_policy_error, structured_tool_error,
    structured_tool_error_with_details, structured_tool_result,
};

use self::confirmation::ConfirmationRequired;
use self::policy::{evaluate_tool_policy, McpPolicyDenial, ToolPolicyEvaluation};

type ToolHandler =
    fn(&dyn CommandBusAdapter, &ToolPolicyEvaluation, Value) -> Result<Value, ToolCallError>;

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
    /// Underscore alias to canonical dotted name. Built once at registration.
    /// Canonical names are never inserted as keys.
    aliases: HashMap<String, &'static str>,
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
    ExecutionWithDetails {
        code: &'static str,
        message: &'static str,
        details: Value,
    },
}

impl ToolRegistry {
    pub fn with_command_bus(command_bus: Arc<dyn CommandBusAdapter>) -> Self {
        let mut tools = real::registered_tools();
        tools.extend(workspace::registered_tools());
        tools.extend(api::registered_tools());
        tools.extend(database::registered_tools());
        tools.extend(system::registered_tools());
        tools.extend(activity::registered_tools());
        tools.extend(flow::registered_tools());
        tools.extend(ssh::registered_tools());
        tools.extend(connection_diagnostics::registered_tools());
        let aliases = build_tool_alias_index(tools.iter().map(|tool| tool.definition.name))
            .unwrap_or_else(|error| panic!("MCP tool registry rejected: {error}"));

        Self {
            tools,
            aliases,
            command_bus,
        }
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| tool.definition.clone())
            .collect()
    }

    /// Resolve a requested tool name to its canonical dotted name.
    ///
    /// Canonical names match first. Underscore aliases match only the map built
    /// at registration. Any other string is `UNKNOWN_TOOL`.
    pub(crate) fn resolve_tool_name<'a>(
        &'a self,
        requested: &'a str,
    ) -> Result<&'a str, ToolCallError> {
        if self
            .tools
            .iter()
            .any(|tool| tool.definition.name == requested)
        {
            return Ok(requested);
        }
        self.aliases
            .get(requested)
            .copied()
            .ok_or_else(|| ToolCallError::UnknownTool(requested.to_string()))
    }

    pub(crate) fn call(&self, name: &str, arguments: Value) -> Result<Value, ToolCallError> {
        let started = std::time::Instant::now();
        let name = self.resolve_tool_name(name)?;
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
        let result = if crate::call_control::is_cancelled() {
            Err(ToolCallError::Execution {
                code: "MCP_CALL_CANCELLED",
                message: "The MCP call was cancelled before execution.",
            })
        } else {
            (tool.handler)(self.command_bus.as_ref(), &policy, arguments)
        };

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
                denial.risk_level,
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
            Err(ToolCallError::ExecutionWithDetails {
                code,
                message,
                details,
            }) => Ok(structured_tool_error_with_details(
                name,
                &policy.workspace.environment_type,
                policy.risk.risk_level(),
                started.elapsed().as_millis(),
                code,
                message,
                details,
            )),
            Err(error) => Err(error),
        }
    }
}

/// Stable underscore alias for a dotted canonical tool name.
/// `unfour.system.health` becomes `unfour_system_health`.
pub(super) fn underscore_tool_alias(canonical: &str) -> String {
    canonical.replace('.', "_")
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ToolNameIndexError {
    DuplicateCanonical {
        name: String,
    },
    AliasCollision {
        alias: String,
        existing: String,
        incoming: String,
    },
}

impl std::fmt::Display for ToolNameIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateCanonical { name } => {
                write!(formatter, "duplicate canonical MCP tool name `{name}`")
            }
            Self::AliasCollision {
                alias,
                existing,
                incoming,
            } => write!(
                formatter,
                "MCP tool alias `{alias}` collides between `{existing}` and `{incoming}`"
            ),
        }
    }
}

/// Build `alias -> canonical` from registered canonical names.
///
/// Duplicate canonical names and any alias that matches another canonical name
/// or another alias fail here. Names without a dot do not get a separate alias.
pub(super) fn build_tool_alias_index<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> Result<HashMap<String, &'a str>, ToolNameIndexError> {
    let names = names.into_iter().collect::<Vec<_>>();
    let mut canonical = HashSet::with_capacity(names.len());
    for name in &names {
        if !canonical.insert(*name) {
            return Err(ToolNameIndexError::DuplicateCanonical {
                name: (*name).to_string(),
            });
        }
    }

    let mut aliases: HashMap<String, &'a str> = HashMap::new();
    for name in names {
        let alias = underscore_tool_alias(name);
        if alias == name {
            continue;
        }
        if canonical.contains(alias.as_str()) {
            return Err(ToolNameIndexError::AliasCollision {
                existing: alias.clone(),
                incoming: name.to_string(),
                alias,
            });
        }
        if let Some(existing) = aliases.get(&alias) {
            return Err(ToolNameIndexError::AliasCollision {
                alias,
                existing: (*existing).to_string(),
                incoming: name.to_string(),
            });
        }
        aliases.insert(alias, name);
    }
    Ok(aliases)
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
            denial.risk_level,
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
        ToolCallError::ExecutionWithDetails {
            code,
            message,
            details,
        } => Ok(structured_tool_error_with_details(
            tool_name,
            "unknown",
            "medium",
            duration_ms,
            code,
            message,
            details,
        )),
        other => Err(other),
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
#[allow(deprecated)]
#[path = "tools_tests/mod.rs"]
mod tools_tests;
