use serde::Serialize;
use serde_json::{Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};

use crate::command_bus_adapter::CommandBusAdapter;

use super::ToolCallError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspacePolicyContext {
    pub workspace_id: String,
    pub workspace_name: String,
    pub environment_type: String,
    pub mcp_policy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResolvedMcpPolicy {
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
        "unfour.api.send_request" => {
            if api_method
                .map(|method| method.eq_ignore_ascii_case("GET"))
                .unwrap_or(false)
            {
                (McpCapability::ApiSend, McpRisk::Read)
            } else {
                (McpCapability::ApiMutate, McpRisk::Write)
            }
        }
        "unfour.db.list_connections"
        | "unfour.db.list_tables"
        | "unfour.db.describe_table"
        | "unfour.db.test_connection" => (McpCapability::DbSchemaRead, McpRisk::Read),
        "unfour.db.query_readonly" => (McpCapability::DbDataRead, McpRisk::Read),
        "unfour.ssh.run_diagnostic" => (McpCapability::SshExec, McpRisk::Execute),
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
    } else if matches!(capability, McpCapability::DestructiveRun)
        || matches!(risk, McpRisk::Destructive)
    {
        Some(
            "Blocked by workspace policy. Destructive MCP actions are disabled by default."
                .to_string(),
        )
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
            ResolvedMcpPolicy::Guarded if !is_read_allowed_in_read_only(capability, risk) => {
                Some("Blocked by workspace policy. Guarded MCP actions require confirmation, which is not available yet.".to_string())
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

pub(super) fn check_tool_policy(
    command_bus: &dyn CommandBusAdapter,
    tool_name: &str,
    arguments: &Value,
) -> Result<(), ToolCallError> {
    let Some(arguments) = arguments.as_object() else {
        return Ok(());
    };
    let mut workspace_id = parse_optional_string(arguments, "workspaceId");
    let mut api_method = None;

    if matches!(
        tool_name,
        "unfour.api.get_request" | "unfour.api.send_request"
    ) {
        let Some(request_id) = parse_optional_string(arguments, "requestId") else {
            return Ok(());
        };
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
        api_method = Some(result.request.method);
    }

    let (capability, risk) = classify_mcp_action(tool_name, api_method.as_deref());
    let workspace = match workspace_id {
        Some(workspace_id) => workspace_context_by_id(command_bus, &workspace_id)?,
        None => active_workspace_context(command_bus)?,
    };

    check_mcp_permission(&workspace, capability, risk).map_err(ToolCallError::PolicyBlocked)
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
            | McpCapability::SecretUse
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
    use super::*;

    fn workspace(environment_type: &str, mcp_policy: &str) -> WorkspacePolicyContext {
        WorkspacePolicyContext {
            workspace_id: "ws-1".to_string(),
            workspace_name: "Production".to_string(),
            environment_type: environment_type.to_string(),
            mcp_policy: mcp_policy.to_string(),
        }
    }

    #[test]
    fn resolve_mcp_policy_maps_auto_from_environment() {
        assert_eq!(
            resolve_mcp_policy(&workspace("dev", "auto")),
            ResolvedMcpPolicy::FullAccess
        );
        assert_eq!(
            resolve_mcp_policy(&workspace("test", "auto")),
            ResolvedMcpPolicy::Guarded
        );
        assert_eq!(
            resolve_mcp_policy(&workspace("prod", "auto")),
            ResolvedMcpPolicy::ReadOnly
        );
    }

    #[test]
    fn resolve_mcp_policy_uses_explicit_policy_over_environment() {
        assert_eq!(
            resolve_mcp_policy(&workspace("prod", "disabled")),
            ResolvedMcpPolicy::Disabled
        );
        assert_eq!(
            resolve_mcp_policy(&workspace("prod", "full_access")),
            ResolvedMcpPolicy::FullAccess
        );
    }

    #[test]
    fn check_mcp_permission_enforces_read_only_boundaries() {
        let workspace = workspace("prod", "auto");

        assert!(
            check_mcp_permission(&workspace, McpCapability::WorkspaceRead, McpRisk::Read).is_ok()
        );
        assert!(
            check_mcp_permission(&workspace, McpCapability::DbSchemaRead, McpRisk::Read).is_ok()
        );

        for (capability, risk) in [
            (McpCapability::DbDataWrite, McpRisk::Write),
            (McpCapability::SshExec, McpRisk::Execute),
            (McpCapability::DestructiveRun, McpRisk::Destructive),
        ] {
            let denial = check_mcp_permission(&workspace, capability, risk).unwrap_err();
            assert!(denial.blocked);
            assert_eq!(denial.workspace_id, "ws-1");
            assert_eq!(denial.environment_type, "prod");
            assert_eq!(denial.resolved_policy, "read_only");
            assert_eq!(denial.capability, capability.as_str());
            assert_eq!(denial.risk, risk.as_str());
        }
    }

    #[test]
    fn check_mcp_permission_never_allows_secret_reveal_or_destructive_full_access() {
        let workspace = workspace("dev", "auto");

        assert!(check_mcp_permission(&workspace, McpCapability::ApiSend, McpRisk::Write).is_ok());
        assert!(
            check_mcp_permission(&workspace, McpCapability::DbDataWrite, McpRisk::Write).is_ok()
        );
        assert!(check_mcp_permission(&workspace, McpCapability::SshExec, McpRisk::Execute).is_ok());

        assert!(check_mcp_permission(
            &workspace,
            McpCapability::SecretReveal,
            McpRisk::SecretReveal
        )
        .is_err());
        assert!(check_mcp_permission(
            &workspace,
            McpCapability::DestructiveRun,
            McpRisk::Destructive
        )
        .is_err());
    }
}
