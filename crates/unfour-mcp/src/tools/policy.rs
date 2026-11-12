use serde::Serialize;
use serde_json::{Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::ApiSavedRequest;

use crate::command_bus_adapter::CommandBusAdapter;

use super::ssh_risk::{build_ssh_exec_command, is_readonly_ssh_command};
use super::ToolCallError;

pub(super) use unfour_command_bus::CurrentWorkspaceResult as WorkspacePolicyContext;

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
    WorkspaceMutate,
    ApiRead,
    ApiSend,
    ApiMutate,
    DbConnectionMutate,
    DbSchemaRead,
    DbDataRead,
    DbDataWrite,
    SshConnect,
    SshExec,
    SshTaskManage,
    SecretUse,
    SecretReveal,
    DestructiveRun,
}

impl McpCapability {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceRead => "workspace:read",
            Self::WorkspaceMutate => "workspace:mutate",
            Self::ApiRead => "api:read",
            Self::ApiSend => "api:send",
            Self::ApiMutate => "api:mutate",
            Self::DbConnectionMutate => "db:connection:mutate",
            Self::DbSchemaRead => "db:schema:read",
            Self::DbDataRead => "db:data:read",
            Self::DbDataWrite => "db:data:write",
            Self::SshConnect => "ssh:connect",
            Self::SshExec => "ssh:exec",
            Self::SshTaskManage => "ssh:task:manage",
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

pub(crate) struct ToolPolicyEvaluation {
    pub workspace: WorkspacePolicyContext,
    pub resolved_policy: ResolvedMcpPolicy,
    pub risk: McpRisk,
    // Internal execution data can contain credentials. Keep it out of MCP results and logs.
    pub(super) saved_request: Option<ApiSavedRequest>,
}

impl ToolPolicyEvaluation {
    pub(super) fn saved_request(&self, id: &str) -> Result<&ApiSavedRequest, ToolCallError> {
        self.saved_request
            .as_ref()
            .filter(|request| {
                request.id == id && request.workspace_id == self.workspace.workspace_id
            })
            .ok_or_else(unexpected_result)
    }

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
    /// The *capability* required by the blocked action, e.g.
    /// `db:data:write` or `ssh:exec`. Distinct from `risk` / `risk_level`.
    pub capability: &'static str,
    /// The *category* (kind) of the blocked action: `read` / `write` /
    /// `execute` / `destructive` / `secret_reveal`. This is the kind of
    /// operation — NOT its severity. The severity bucket is `risk_level`.
    pub risk: &'static str,
    /// The *severity* of the blocked action, bucketed as `low` / `medium` /
    /// `high`. This is the same scale as MCP result `_meta.riskLevel` and is
    /// derived from `risk` exactly once here, so the two can never diverge.
    /// Do NOT re-derive severity from `risk` elsewhere.
    pub risk_level: &'static str,
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
    database_driver: Option<&str>,
) -> Option<(McpCapability, McpRisk)> {
    Some(match tool_name {
        "unfour.workspace.create" | "unfour.workspace.rename" | "unfour.db.save_saved_sql" => {
            (McpCapability::WorkspaceMutate, McpRisk::Write)
        }
        "unfour.db.list_saved_sql" | "unfour.db.get_saved_sql" => {
            (McpCapability::WorkspaceRead, McpRisk::Read)
        }
        "unfour.db.delete_saved_sql" => (McpCapability::WorkspaceMutate, McpRisk::Destructive),
        "unfour.db.list_catalogs" => (McpCapability::DbSchemaRead, McpRisk::Read),
        "unfour.api.list_folders" => (McpCapability::ApiRead, McpRisk::Read),
        "unfour.api.create_folder"
        | "unfour.api.rename_folder"
        | "unfour.api.move_folder"
        | "unfour.api.reorder_folders"
        | "unfour.api.move_request"
        | "unfour.api.duplicate_request" => (McpCapability::ApiMutate, McpRisk::Write),
        "unfour.api.delete_folder" => (McpCapability::ApiMutate, McpRisk::Destructive),
        "unfour.flow.list"
        | "unfour.flow.get"
        | "unfour.flow.list_runs"
        | "unfour.flow.get_run" => (McpCapability::WorkspaceRead, McpRisk::Read),
        "unfour.flow.save" => (McpCapability::WorkspaceMutate, McpRisk::Write),
        "unfour.flow.delete" => (McpCapability::WorkspaceMutate, McpRisk::Destructive),
        "unfour.flow.run" | "unfour.flow.cancel_run" => {
            (McpCapability::DestructiveRun, McpRisk::Execute)
        }
        "unfour.workspace.current"
        | "unfour.workspace.list"
        | "unfour.connection.list"
        | "unfour.activity.list"
        | "unfour.system.health"
        | "unfour.workspace.list_variables"
        | "unfour.ssh.list_history"
        | "unfour.db.list_history"
        | "unfour.ssh.get_host_key"
        | "unfour.ssh.list_tasks"
        | "unfour.ssh.get_task"
        | "unfour.ssh.list_task_runs"
        | "unfour.ssh.read_task_run_log" => (McpCapability::WorkspaceRead, McpRisk::Read),
        "unfour.workspace.create_variable"
        | "unfour.workspace.update_variable"
        | "unfour.workspace.replace_variables" => (McpCapability::WorkspaceMutate, McpRisk::Write),
        "unfour.workspace.delete_variable" => {
            (McpCapability::WorkspaceMutate, McpRisk::Destructive)
        }
        "unfour.api.list_collections"
        | "unfour.api.list_requests"
        | "unfour.api.get_request"
        | "unfour.api.list_history"
        | "unfour.api.get_history"
        | "unfour.api.list_environments" => (McpCapability::ApiRead, McpRisk::Read),
        "unfour.api.create_request"
        | "unfour.api.update_request"
        | "unfour.api.create_collection"
        | "unfour.api.update_collection"
        | "unfour.api.create_environment"
        | "unfour.api.update_environment"
        | "unfour.api.set_environment_variable" => (McpCapability::ApiMutate, McpRisk::Write),
        "unfour.api.delete_request"
        | "unfour.api.delete_collection"
        | "unfour.api.delete_environment"
        | "unfour.api.delete_environment_variable" => {
            (McpCapability::ApiMutate, McpRisk::Destructive)
        }
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
        "unfour.db.create_connection" | "unfour.db.update_connection" => {
            (McpCapability::DbConnectionMutate, McpRisk::Write)
        }
        "unfour.db.delete_connection" => (McpCapability::DbConnectionMutate, McpRisk::Destructive),
        "unfour.db.query_readonly" | "unfour.db.export_table" => {
            (McpCapability::DbDataRead, McpRisk::Read)
        }
        "unfour.db.explain" => (McpCapability::DbDataRead, McpRisk::Read),
        "unfour.db.execute" => {
            if arguments
                .and_then(|arguments| arguments.get("sql"))
                .and_then(Value::as_str)
                .is_some_and(|sql| is_readonly_sql(sql, database_driver))
            {
                (McpCapability::DbDataRead, McpRisk::Read)
            } else {
                (McpCapability::DbDataWrite, McpRisk::Write)
            }
        }
        "unfour.ssh.create_connection" | "unfour.ssh.update_connection" => {
            (McpCapability::SshConnect, McpRisk::Write)
        }
        "unfour.ssh.delete_connection" => (McpCapability::SshConnect, McpRisk::Destructive),
        "unfour.ssh.test_connection" => (McpCapability::SshExec, McpRisk::Read),
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
        "unfour.ssh.save_task" | "unfour.ssh.duplicate_task" | "unfour.ssh.reorder_tasks" => {
            (McpCapability::SshTaskManage, McpRisk::Write)
        }
        "unfour.ssh.delete_task" | "unfour.ssh.clear_task_runs" => {
            (McpCapability::SshTaskManage, McpRisk::Destructive)
        }
        "unfour.ssh.run_task" | "unfour.ssh.cancel_task_run" => {
            (McpCapability::SshExec, McpRisk::Execute)
        }
        _ => return None,
    })
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
            risk_level: risk.risk_level(),
        }),
        None => Ok(()),
    }
}

pub(super) fn evaluate_tool_policy(
    command_bus: &dyn CommandBusAdapter,
    tool_name: &str,
    arguments: &Value,
) -> Result<ToolPolicyEvaluation, ToolCallError> {
    let Some(arguments) = arguments.as_object() else {
        let (capability, risk) = classify_mcp_action(tool_name, None, None, None)
            .ok_or_else(unclassified_policy_error)?;
        let workspace = active_workspace_context(command_bus)?;
        let resolved_policy = resolve_mcp_policy(&workspace);
        check_mcp_permission(&workspace, capability, risk).map_err(ToolCallError::PolicyBlocked)?;
        return Ok(ToolPolicyEvaluation {
            workspace,
            resolved_policy,
            risk,
            saved_request: None,
        });
    };
    // Creation is authorized only by the active workspace, never by a caller-
    // selected writable workspace. The handler rejects all extra fields.
    let mut workspace_id = if tool_name == "unfour.workspace.create" {
        None
    } else {
        optional_id(arguments, "workspaceId")?
    };
    let uses_saved_request = matches!(
        tool_name,
        "unfour.api.get_request" | "unfour.api.send_request" | "unfour.api.update_request"
    ) || (tool_name == "unfour.api.delete_request"
        && workspace_id.is_none());
    let saved_request = if uses_saved_request {
        optional_id(arguments, "requestId")?
            .map(|id| read_saved_request(command_bus, id))
            .transpose()?
    } else {
        None
    };
    if let Some(request) = &saved_request {
        if workspace_id.is_some_and(|id| id != request.workspace_id) {
            return Err(ToolCallError::Execution {
                code: "NOT_FOUND",
                message: "The requested API request was not found in this workspace.",
            });
        }
        workspace_id = Some(&request.workspace_id);
    }

    let workspace = match workspace_id {
        Some(workspace_id) => workspace_context_by_id(command_bus, workspace_id)?,
        None => active_workspace_context(command_bus)?,
    };
    let database_driver =
        database_driver_hint(command_bus, tool_name, arguments, &workspace.workspace_id);
    let scripted_replay = tool_name == "unfour.api.send_request"
        && saved_request.as_ref().is_some_and(|request| {
            [&request.pre_request_script, &request.post_response_script]
                .into_iter()
                .flatten()
                .any(|script| !script.trim().is_empty())
        });
    let (capability, risk) = if scripted_replay {
        (McpCapability::ApiMutate, McpRisk::Write)
    } else {
        let api_method = saved_request
            .as_ref()
            .map(|request| request.method.as_str())
            .or_else(|| arguments.get("method").and_then(Value::as_str));
        classify_mcp_action(
            tool_name,
            Some(arguments),
            api_method,
            database_driver.as_deref(),
        )
        .ok_or_else(unclassified_policy_error)?
    };
    let resolved_policy = resolve_mcp_policy(&workspace);
    check_mcp_permission(&workspace, capability, risk).map_err(ToolCallError::PolicyBlocked)?;
    Ok(ToolPolicyEvaluation {
        workspace,
        resolved_policy,
        risk,
        saved_request,
    })
}

fn read_saved_request(
    command_bus: &dyn CommandBusAdapter,
    request_id: &str,
) -> Result<ApiSavedRequest, ToolCallError> {
    let result = command_bus
        .execute_read(ReadCommand::ApiGetRequest {
            request_id: request_id.to_owned(),
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    match result {
        ReadCommandResult::ApiRequest(detail) if detail.request.id == request_id => {
            Ok(detail.request)
        }
        _ => Err(unexpected_result()),
    }
}

fn optional_id<'a>(
    arguments: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(id)) if id.is_empty() => Ok(None),
        Some(Value::String(id)) if !id.trim().is_empty() => Ok(Some(id.trim())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a non-blank ID"
        ))),
    }
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

    Ok(workspace)
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
        workspace_root: None,
        mode: "local".into(),
        source: result.source,
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

fn database_driver_hint(
    command_bus: &dyn CommandBusAdapter,
    tool_name: &str,
    arguments: &Map<String, Value>,
    workspace_id: &str,
) -> Option<String> {
    if tool_name != "unfour.db.execute" {
        return None;
    }
    let sql = arguments.get("sql").and_then(Value::as_str)?;
    // A statement proven read-only in every dialect does not need the driver.
    // Dialect-specific syntax is proven only against the saved connection.
    if unfour_command_bus::sql_is_proven_read_for_driver(sql, None) {
        return None;
    }
    let connection_id = parse_optional_string(arguments, "connectionId")?;
    command_bus
        .list_db_connections(workspace_id)
        .ok()?
        .into_iter()
        .find(|connection| {
            connection.id == connection_id
                && connection.workspace_id == workspace_id
                && connection.deleted_at.is_none()
        })
        .map(|connection| connection.driver)
}

fn is_readonly_sql(sql: &str, driver: Option<&str>) -> bool {
    unfour_command_bus::sql_is_proven_read_for_driver(sql, None)
        || driver.is_some_and(|driver| {
            unfour_command_bus::sql_is_proven_read_for_driver(sql, Some(driver))
        })
}

fn unexpected_result() -> ToolCallError {
    ToolCallError::Execution {
        code: "COMMAND_BUS_RESULT_MISMATCH",
        message: "The command-bus returned an unexpected result.",
    }
}

fn unclassified_policy_error() -> ToolCallError {
    ToolCallError::Execution {
        code: "MCP_TOOL_POLICY_UNCLASSIFIED",
        message: "The MCP tool has no explicit policy classification and was denied.",
    }
}

#[cfg(test)]
#[path = "policy_tests/mod.rs"]
mod policy_tests;
