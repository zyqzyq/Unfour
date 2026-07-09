use serde::Serialize;
use serde_json::{Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};

use crate::command_bus_adapter::CommandBusAdapter;

use super::ssh_risk::{build_ssh_exec_command, is_readonly_ssh_command};
use super::ToolCallError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspacePolicyContext {
    pub workspace_id: String,
    pub workspace_name: String,
    pub environment_type: String,
    pub mcp_policy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedMcpPolicy {
    Disabled,
    ReadOnly,
    Guarded,
    FullAccess,
}

impl ResolvedMcpPolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ReadOnly => "read_only",
            Self::Guarded => "guarded",
            Self::FullAccess => "full_access",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum McpCapability {
    WorkspaceRead,
    ApiRead,
    ApiSend,
    ApiMutate,
    DbConnectionMutate,
    DbSchemaRead,
    DbDataRead,
    DbDataWrite,
    SshConnect,
    SshExec,
    SecretUse,
    SecretReveal,
    DestructiveRun,
}

impl McpCapability {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceRead => "workspace:read",
            Self::ApiRead => "api:read",
            Self::ApiSend => "api:send",
            Self::ApiMutate => "api:mutate",
            Self::DbConnectionMutate => "db:connection:mutate",
            Self::DbSchemaRead => "db:schema:read",
            Self::DbDataRead => "db:data:read",
            Self::DbDataWrite => "db:data:write",
            Self::SshConnect => "ssh:connect",
            Self::SshExec => "ssh:exec",
            Self::SecretUse => "secret:use",
            Self::SecretReveal => "secret:reveal",
            Self::DestructiveRun => "destructive:run",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum McpRisk {
    Read,
    Write,
    Execute,
    Destructive,
    SecretReveal,
}

impl McpRisk {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Execute => "execute",
            Self::Destructive => "destructive",
            Self::SecretReveal => "secret_reveal",
        }
    }

    pub(super) fn risk_level(self) -> &'static str {
        match self {
            Self::Read => "low",
            Self::Write | Self::Execute => "medium",
            Self::Destructive | Self::SecretReveal => "high",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolPolicyEvaluation {
    pub workspace: WorkspacePolicyContext,
    pub resolved_policy: ResolvedMcpPolicy,
    pub capability: McpCapability,
    pub risk: McpRisk,
}

impl ToolPolicyEvaluation {
    /// Whether a risky / mutating action must be explicitly confirmed by the
    /// caller before execution under the active MCP policy.
    ///
    /// Only the `guarded` tier requires confirmation. `full_access` trusts the
    /// calling agent to perform risky actions without a confirmation prompt,
    /// and `read_only` / `disabled` never reach mutating handlers because they
    /// are blocked earlier by `check_mcp_permission`. This is what gives the
    /// four policy tiers distinct runtime behavior: `guarded` and `full_access`
    /// are no longer collapsed into the same unconditional-confirmation path.
    pub(crate) fn requires_confirmation(&self) -> bool {
        self.resolved_policy == ResolvedMcpPolicy::Guarded
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPolicyDenial {
    pub blocked: bool,
    pub reason: String,
    pub error: PolicyError,
    pub workspace_id: String,
    pub workspace_name: String,
    pub environment_type: String,
    pub mcp_policy: String,
    pub resolved_policy: String,
    pub capability: &'static str,
    pub risk: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyError {
    pub code: &'static str,
    pub message: String,
}

pub(super) fn resolve_mcp_policy(workspace: &WorkspacePolicyContext) -> ResolvedMcpPolicy {
    match workspace.mcp_policy.as_str() {
        "disabled" => ResolvedMcpPolicy::Disabled,
        "read_only" => ResolvedMcpPolicy::ReadOnly,
        "guarded" => ResolvedMcpPolicy::Guarded,
        "full_access" => ResolvedMcpPolicy::FullAccess,
        "auto" => match workspace.environment_type.as_str() {
            "dev" => ResolvedMcpPolicy::FullAccess,
            "test" => ResolvedMcpPolicy::Guarded,
            "prod" => ResolvedMcpPolicy::ReadOnly,
            _ => ResolvedMcpPolicy::ReadOnly,
        },
        _ => ResolvedMcpPolicy::Disabled,
    }
}

pub(super) fn classify_mcp_action(
    tool_name: &str,
    arguments: Option<&Map<String, Value>>,
    api_method: Option<&str>,
) -> (McpCapability, McpRisk) {
    match tool_name {
        "unfour.workspace.current"
        | "unfour.workspace.list"
        | "unfour.connection.list"
        | "unfour.activity.list"
        | "unfour.system.health" => (McpCapability::WorkspaceRead, McpRisk::Read),
        "unfour.api.list_collections"
        | "unfour.api.list_requests"
        | "unfour.api.get_request"
        | "unfour.api.list_history"
        | "unfour.api.get_history"
        | "unfour.api.list_environments" => (McpCapability::ApiRead, McpRisk::Read),
        "unfour.api.create_request"
        | "unfour.api.update_request"
        | "unfour.api.delete_request"
        | "unfour.api.create_collection"
        | "unfour.api.update_collection"
        | "unfour.api.delete_collection" => (McpCapability::ApiMutate, McpRisk::Write),
        "unfour.api.send_request" => {
            if api_method.map(is_readonly_http_method).unwrap_or(false) {
                (McpCapability::ApiSend, McpRisk::Read)
            } else {
                (McpCapability::ApiMutate, McpRisk::Write)
            }
        }
        "unfour.db.list_connections"
        | "unfour.db.list_tables"
        | "unfour.db.describe_table"
        | "unfour.db.test_connection" => (McpCapability::DbSchemaRead, McpRisk::Read),
        "unfour.db.create_connection" => (McpCapability::DbConnectionMutate, McpRisk::Write),
        "unfour.db.query_readonly" => (McpCapability::DbDataRead, McpRisk::Read),
        "unfour.db.explain" => (McpCapability::DbDataRead, McpRisk::Read),
        "unfour.db.execute" => {
            if arguments
                .and_then(|arguments| arguments.get("sql"))
                .and_then(Value::as_str)
                .map(is_readonly_sql)
                .unwrap_or(false)
            {
                (McpCapability::DbDataRead, McpRisk::Read)
            } else {
                (McpCapability::DbDataWrite, McpRisk::Write)
            }
        }
        "unfour.ssh.create_connection" => (McpCapability::SshConnect, McpRisk::Write),
        "unfour.ssh.list_connections" => (McpCapability::WorkspaceRead, McpRisk::Read),
        "unfour.ssh.run_diagnostic" | "unfour.ssh.read_file" | "unfour.ssh.list_dir" => {
            (McpCapability::SshExec, McpRisk::Read)
        }
        "unfour.ssh.exec" => {
            if arguments
                .and_then(|arguments| arguments.get("command"))
                .and_then(Value::as_str)
                .map(|command| {
                    let cwd = arguments
                        .and_then(|arguments| arguments.get("cwd"))
                        .and_then(Value::as_str)
                        .filter(|cwd| !cwd.is_empty());
                    let env = arguments
                        .and_then(|arguments| arguments.get("env"))
                        .and_then(Value::as_object);
                    build_ssh_exec_command(command, cwd, env)
                })
                .as_deref()
                .map(is_readonly_ssh_command)
                .unwrap_or(false)
            {
                (McpCapability::SshExec, McpRisk::Read)
            } else {
                (McpCapability::SshExec, McpRisk::Execute)
            }
        }
        "unfour.ssh.write_file" | "unfour.ssh.patch_file" => {
            (McpCapability::SshExec, McpRisk::Write)
        }
        _ => (McpCapability::WorkspaceRead, McpRisk::Read),
    }
}

pub(super) fn check_mcp_permission(
    workspace: &WorkspacePolicyContext,
    capability: McpCapability,
    risk: McpRisk,
) -> Result<(), McpPolicyDenial> {
    let resolved = resolve_mcp_policy(workspace);
    let reason = if matches!(capability, McpCapability::SecretReveal)
        || matches!(risk, McpRisk::SecretReveal)
    {
        Some("Blocked by workspace policy. MCP cannot reveal secrets in any workspace.".to_string())
    } else {
        match resolved {
            ResolvedMcpPolicy::Disabled => {
                Some("Blocked by workspace policy. MCP is disabled for this workspace.".to_string())
            }
            ResolvedMcpPolicy::ReadOnly if !is_read_allowed_in_read_only(capability, risk) => {
                if workspace.environment_type == "prod" && workspace.mcp_policy == "auto" {
                    Some("Blocked by workspace policy. Production workspace only allows read-only MCP actions by default.".to_string())
                } else {
                    Some("Blocked by workspace policy. This workspace only allows read-only MCP actions.".to_string())
                }
            }
            _ => None,
        }
    };

    match reason {
        Some(reason) => Err(McpPolicyDenial {
            blocked: true,
            error: PolicyError {
                code: "WORKSPACE_POLICY_BLOCKED",
                message: reason.clone(),
            },
            reason,
            workspace_id: workspace.workspace_id.clone(),
            workspace_name: workspace.workspace_name.clone(),
            environment_type: workspace.environment_type.clone(),
            mcp_policy: workspace.mcp_policy.clone(),
            resolved_policy: resolved.as_str().to_string(),
            capability: capability.as_str(),
            risk: risk.as_str(),
        }),
        None => Ok(()),
    }
}

#[allow(dead_code)]
pub(super) fn check_tool_policy(
    command_bus: &dyn CommandBusAdapter,
    tool_name: &str,
    arguments: &Value,
) -> Result<(), ToolCallError> {
    evaluate_tool_policy(command_bus, tool_name, arguments).map(|_| ())
}

pub(super) fn evaluate_tool_policy(
    command_bus: &dyn CommandBusAdapter,
    tool_name: &str,
    arguments: &Value,
) -> Result<ToolPolicyEvaluation, ToolCallError> {
    let Some(arguments) = arguments.as_object() else {
        let workspace = active_workspace_context(command_bus)?;
        let (capability, risk) = classify_mcp_action(tool_name, None, None);
        let resolved_policy = resolve_mcp_policy(&workspace);
        check_mcp_permission(&workspace, capability, risk).map_err(ToolCallError::PolicyBlocked)?;
        return Ok(ToolPolicyEvaluation {
            workspace,
            resolved_policy,
            capability,
            risk,
        });
    };
    let mut workspace_id = parse_optional_string(arguments, "workspaceId");
    let mut api_method = parse_optional_string(arguments, "method");

    if matches!(
        tool_name,
        "unfour.api.get_request" | "unfour.api.send_request" | "unfour.api.update_request"
    ) {
        if let Some(request_id) = parse_optional_string(arguments, "requestId") {
            let result = command_bus
                .execute_read(ReadCommand::ApiGetRequest { request_id })
                .map_err(|error| ToolCallError::Execution {
                    code: error.code,
                    message: error.message,
                })?;
            let ReadCommandResult::ApiRequest(result) = result else {
                return Err(unexpected_result());
            };
            workspace_id = Some(result.request.workspace_id);
            if api_method.is_none() {
                api_method = Some(result.request.method);
            }
        }
    }

    let (capability, risk) = classify_mcp_action(tool_name, Some(arguments), api_method.as_deref());
    let workspace = match workspace_id {
        Some(workspace_id) => workspace_context_by_id(command_bus, &workspace_id)?,
        None => active_workspace_context(command_bus)?,
    };

    let resolved_policy = resolve_mcp_policy(&workspace);
    check_mcp_permission(&workspace, capability, risk).map_err(ToolCallError::PolicyBlocked)?;
    Ok(ToolPolicyEvaluation {
        workspace,
        resolved_policy,
        capability,
        risk,
    })
}

fn active_workspace_context(
    command_bus: &dyn CommandBusAdapter,
) -> Result<WorkspacePolicyContext, ToolCallError> {
    let result = command_bus
        .execute_read(ReadCommand::CurrentWorkspace)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let ReadCommandResult::CurrentWorkspace(workspace) = result else {
        return Err(unexpected_result());
    };

    Ok(WorkspacePolicyContext {
        workspace_id: workspace.workspace_id,
        workspace_name: workspace.workspace_name,
        environment_type: workspace.environment_type,
        mcp_policy: workspace.mcp_policy,
    })
}

fn workspace_context_by_id(
    command_bus: &dyn CommandBusAdapter,
    workspace_id: &str,
) -> Result<WorkspacePolicyContext, ToolCallError> {
    let result = command_bus
        .execute_read(ReadCommand::ListWorkspaces)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let ReadCommandResult::Workspaces(result) = result else {
        return Err(unexpected_result());
    };
    let workspace = result
        .workspaces
        .into_iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or(ToolCallError::Execution {
            code: "WORKSPACE_NOT_FOUND",
            message: "The requested workspace was not found.",
        })?;

    Ok(WorkspacePolicyContext {
        workspace_id: workspace.id,
        workspace_name: workspace.name,
        environment_type: workspace.environment_type,
        mcp_policy: workspace.mcp_policy,
    })
}

fn parse_optional_string(arguments: &Map<String, Value>, key: &str) -> Option<String> {
    match arguments.get(key) {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.trim().to_string()),
        _ => None,
    }
}

fn is_read_allowed_in_read_only(capability: McpCapability, risk: McpRisk) -> bool {
    if !matches!(risk, McpRisk::Read) {
        return false;
    }

    matches!(
        capability,
        McpCapability::WorkspaceRead
            | McpCapability::ApiRead
            | McpCapability::ApiSend
            | McpCapability::DbSchemaRead
            | McpCapability::DbDataRead
            | McpCapability::SshExec
            | McpCapability::SecretUse
    )
}

fn is_readonly_http_method(method: &str) -> bool {
    matches!(
        method.trim().to_ascii_uppercase().as_str(),
        "GET" | "HEAD" | "OPTIONS"
    )
}

fn is_readonly_sql(sql: &str) -> bool {
    let keyword = sql
        .trim()
        .trim_start_matches(';')
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        keyword.as_str(),
        "select" | "with" | "show" | "describe" | "desc" | "explain" | "pragma"
    )
}

fn unexpected_result() -> ToolCallError {
    ToolCallError::Execution {
        code: "COMMAND_BUS_RESULT_MISMATCH",
        message: "The command-bus returned an unexpected result.",
    }
}

#[cfg(test)]
#[path = "policy_tests/mod.rs"]
mod policy_tests;
