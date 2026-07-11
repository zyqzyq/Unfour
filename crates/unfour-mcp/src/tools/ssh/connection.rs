use serde_json::{json, Value};
use unfour_core::models::{SshConnectionInput, SshDiagnosticInput};

use crate::command_bus_adapter::CommandBusAdapter;

use super::super::policy::ToolPolicyEvaluation;
use super::super::{object_with_allowed_keys, ToolCallError};
use super::helpers::*;

pub(super) fn ssh_create_connection(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "name",
            "host",
            "port",
            "username",
            "authKind",
            "keyPath",
            "credentialRef",
            "secret",
        ],
    )?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let name = parse_required_string(&arguments, "name", "unfour.ssh.create_connection")?;
    let host = parse_required_string(&arguments, "host", "unfour.ssh.create_connection")?;
    let username = parse_required_string(&arguments, "username", "unfour.ssh.create_connection")?;
    let auth_kind = parse_required_string(&arguments, "authKind", "unfour.ssh.create_connection")?;
    let credential_ref = parse_optional_string(&arguments, "credentialRef")?;
    let secret = parse_optional_secret(&arguments, "secret")?;
    if credential_ref.is_some() && secret.is_some() {
        return Err(ToolCallError::InvalidArguments(
            "unfour.ssh.create_connection accepts either `secret` or `credentialRef`, not both"
                .to_string(),
        ));
    }
    let credential_source = if secret.is_some() {
        "created"
    } else if credential_ref.is_some() {
        "provided"
    } else {
        "none"
    };

    let connection = command_bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id,
            name,
            host,
            port: parse_optional_port(&arguments)?,
            username,
            auth_kind,
            key_path: parse_optional_string(&arguments, "keyPath")?,
            credential_ref,
            secret,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    Ok(json!({
        "connection": safe_connection_summary(&connection),
        "credentialStored": connection.credential_ref.is_some(),
        "credentialSource": credential_source,
        "source": "command-bus"
    }))
}

pub(super) fn ssh_run_diagnostic(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &["connectionId", "command", "workspaceId", "timeoutMs"],
    )?;
    let connection_id =
        parse_required_string(&arguments, "connectionId", "unfour.ssh.run_diagnostic")?;
    let command = parse_required_string(&arguments, "command", "unfour.ssh.run_diagnostic")?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let timeout_ms = parse_optional_timeout(&arguments)?;

    let result = command_bus
        .run_ssh_diagnostic(SshDiagnosticInput {
            workspace_id,
            connection_id,
            command,
            timeout_ms,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    Ok(json!({
        "connectionId": result.connection_id,
        "command": result.command,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exitStatus": result.exit_status,
        "truncated": result.truncated,
        "source": "command-bus"
    }))
}

pub(super) fn ssh_list_connections(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId"])?;
    let workspace = resolve_workspace(command_bus, &arguments)?;
    let connections = command_bus
        .list_ssh_connections(&workspace.workspace_id)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let connections = connections
        .iter()
        .map(|connection| {
            json!({
                "connectionId": connection.id,
                "id": connection.id,
                "name": connection.name,
                "host": connection.host,
                "port": connection.port,
                "username": connection.username,
                "environment": workspace.environment_type,
                "workspaceId": connection.workspace_id
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "connections": connections,
        "count": connections.len(),
        "source": "command-bus"
    }))
}
