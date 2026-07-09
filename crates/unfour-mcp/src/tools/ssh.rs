use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::{SshConnection, SshConnectionInput, SshDiagnosticInput};

use crate::command_bus_adapter::CommandBusAdapter;

use super::ssh_risk::{
    build_ssh_exec_command, classify_high_risk_command, is_readonly_ssh_command, is_sensitive_path,
    parse_optional_u64, redact_command_display, shell_quote,
};
use super::{
    confirmation::{ensure_confirmed_if_guarded, is_confirmed},
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
};
use super::policy::ToolPolicyEvaluation;

const MAX_DIAGNOSTIC_TIMEOUT_MS: u64 = 60_000;
const MAX_ONE_SHOT_COMMAND_CHARS: usize = 4096;
const DEFAULT_FILE_LIMIT: u64 = 20 * 1024;
const MAX_FILE_LIMIT: u64 = 128 * 1024;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.create_connection",
                title: "Create SSH Connection",
                description:
                    "Creates a saved SSH connection through the Unfour command bus. Optional secret input is stored in the OS credential store by the SSH engine and only a credential reference is persisted; the tool never returns the secret or credential reference.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "name": { "type": "string" },
                        "host": { "type": "string" },
                        "port": { "type": "integer", "minimum": 1, "maximum": 65535 },
                        "username": { "type": "string" },
                        "authKind": {
                            "type": "string",
                            "enum": ["password", "private-key", "none"]
                        },
                        "keyPath": { "type": "string" },
                        "credentialRef": { "type": "string" },
                        "secret": { "type": "string" }
                    },
                    "required": ["name", "host", "username", "authKind"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: ssh_create_connection,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.list_connections",
                title: "List SSH Connections",
                description:
                    "Lists saved SSH connections for a workspace through the Unfour command bus. Returns connection id, name, host, port, username, and environment; never returns passwords, private keys, passphrases, or credential references.",
                input_schema: json!({
                    "type": "object",
                    "properties": { "workspaceId": { "type": "string" } },
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: ssh_list_connections,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.run_diagnostic",
                title: "Run SSH Diagnostic Command",
                description:
                    "Runs a single read-only diagnostic command on a saved SSH connection through the Unfour command bus and returns captured stdout/stderr. Safe in dev/test/prod for allowlisted diagnostics. For broader command execution use unfour.ssh.exec, which applies environment policy and high-risk confirmation.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "command": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "timeoutMs": { "type": "integer" }
                    },
                    "required": ["connectionId", "command"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: ssh_run_diagnostic,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.exec",
                title: "Execute SSH Command",
                description:
                    "Executes one non-interactive SSH command on a saved connection. Use for dev/test repair loops after diagnostics identify a fix. Dev allows ordinary commands; test allows safe diagnostics and guarded commands; prod only allows read-only diagnostic commands. High-risk commands such as rm -rf, restart/shutdown, kill, docker/kubectl delete, and curl-pipe-shell require confirm with the returned confirmation_text.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "command": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "cwd": { "type": "string" },
                        "env": { "type": "object" },
                        "timeoutMs": { "type": "integer" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["connectionId", "command"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: ssh_exec,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.read_file",
                title: "Read SSH File",
                description:
                    "Reads a bounded remote file slice or tail through SSH. Useful for logs and config inspection. Dev/test/prod allow ordinary reads; output is capped and line-redacted by the SSH engine. Sensitive paths may still be blocked by connection or OS permissions.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "path": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "offset": { "type": "integer" },
                        "limit": { "type": "integer" },
                        "tailLines": { "type": "integer" },
                        "timeoutMs": { "type": "integer" }
                    },
                    "required": ["connectionId", "path"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: ssh_read_file,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.write_file",
                title: "Write SSH File",
                description:
                    "Writes or appends a remote file through SSH. Dev allows ordinary project paths; test and high-risk paths require confirmation; prod is blocked by policy. The returned result contains only path/mode/byte counts and command status, never file content.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "path": { "type": "string" },
                        "content": { "type": "string" },
                        "mode": { "type": "string", "enum": ["overwrite", "append", "create"] },
                        "workspaceId": { "type": "string" },
                        "timeoutMs": { "type": "integer" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["connectionId", "path", "content"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: ssh_write_file,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.patch_file",
                title: "Patch SSH File",
                description:
                    "Applies a small search/replace patch to a remote file through SSH and returns a diff summary without file content. Dev allows single-match project-file patches; test, system paths, or multi-match replacements require confirmation; prod is blocked by policy.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "path": { "type": "string" },
                        "search": { "type": "string" },
                        "replace": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "timeoutMs": { "type": "integer" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["connectionId", "path", "search", "replace"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: ssh_patch_file,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.ssh.list_dir",
                title: "List SSH Directory",
                description:
                    "Lists a bounded remote directory through SSH and returns structured entry summaries when the remote find utility is available. Safe in dev/test/prod.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "connectionId": { "type": "string" },
                        "path": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "limit": { "type": "integer" },
                        "timeoutMs": { "type": "integer" }
                    },
                    "required": ["connectionId", "path"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::remote_read(),
            },
            handler: ssh_list_dir,
        },
    ]
}

fn ssh_create_connection(
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

fn ssh_run_diagnostic(
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

fn ssh_list_connections(
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

fn ssh_exec(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "command",
            "workspaceId",
            "cwd",
            "env",
            "timeoutMs",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.ssh.exec")?;
    let raw_command = parse_required_string(&arguments, "command", "unfour.ssh.exec")?;
    let workspace = resolve_workspace(command_bus, &arguments)?;
    let timeout_ms = parse_optional_timeout(&arguments)?;
    let cwd = parse_optional_string(&arguments, "cwd")?;
    let command = build_ssh_exec_command(
        &raw_command,
        cwd.as_deref(),
        arguments.get("env").and_then(Value::as_object),
    );

    if let Some((code, reason)) = classify_high_risk_command(&command) {
        ensure_confirmed_if_guarded(evaluation,
            &arguments,
            code,
            reason,
            json!({
                "tool": "unfour.ssh.exec",
                "workspaceId": workspace.workspace_id,
                "connectionId": connection_id.clone(),
                "command": command.clone()
            }),
        )?;
    }

    let input = SshDiagnosticInput {
        workspace_id: workspace.workspace_id,
        connection_id,
        command: command.clone(),
        timeout_ms,
    };
    let result = if is_readonly_ssh_command(&command) {
        command_bus.run_ssh_diagnostic(input)
    } else {
        command_bus.run_ssh_command(input)
    }
    .map_err(|error| ToolCallError::Execution {
        code: error.code,
        message: error.message,
    })?;

    Ok(ssh_command_result(result, "command-bus"))
}

fn ssh_read_file(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "path",
            "workspaceId",
            "offset",
            "limit",
            "tailLines",
            "timeoutMs",
        ],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.ssh.read_file")?;
    let path = parse_required_string(&arguments, "path", "unfour.ssh.read_file")?;
    let workspace = resolve_workspace(command_bus, &arguments)?;
    let timeout_ms = parse_optional_timeout(&arguments)?;
    let limit = parse_optional_u64(&arguments, "limit")?
        .unwrap_or(DEFAULT_FILE_LIMIT)
        .clamp(1, MAX_FILE_LIMIT);
    let command = if let Some(tail_lines) = parse_optional_u64(&arguments, "tailLines")? {
        format!(
            "tail -n {} -- {}",
            tail_lines.clamp(1, 10_000),
            shell_quote(&path)
        )
    } else {
        let offset = parse_optional_u64(&arguments, "offset")?.unwrap_or(0);
        if offset > 0 {
            format!(
                "dd if={} bs=1 skip={} count={} 2>/dev/null",
                shell_quote(&path),
                offset,
                limit
            )
        } else {
            format!("head -c {} -- {}", limit, shell_quote(&path))
        }
    };

    let result = command_bus
        .run_ssh_command(SshDiagnosticInput {
            workspace_id: workspace.workspace_id,
            connection_id: connection_id.clone(),
            command,
            timeout_ms,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    Ok(json!({
        "connectionId": connection_id,
        "path": path,
        "content": result.stdout,
        "stderr": result.stderr,
        "exitCode": result.exit_status,
        "limit": limit,
        "truncated": result.truncated,
        "source": "command-bus"
    }))
}

fn ssh_write_file(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "path",
            "content",
            "mode",
            "workspaceId",
            "timeoutMs",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.ssh.write_file")?;
    let path = parse_required_string(&arguments, "path", "unfour.ssh.write_file")?;
    let content = parse_required_raw_string(&arguments, "content", "unfour.ssh.write_file", true)?;
    let mode = parse_optional_string(&arguments, "mode")?.unwrap_or_else(|| "overwrite".into());
    let workspace = resolve_workspace(command_bus, &arguments)?;
    let timeout_ms = parse_optional_timeout(&arguments)?;
    if workspace.environment_type == "test" || is_sensitive_path(&path) {
        ensure_confirmed_if_guarded(evaluation,
            &arguments,
            "SSH_WRITE_FILE",
            "Writing remote files in test or sensitive paths requires confirmation.",
            json!({
                "tool": "unfour.ssh.write_file",
                "workspaceId": workspace.workspace_id,
                "connectionId": connection_id.clone(),
                "path": path.clone(),
                "mode": mode.clone(),
                "contentBytes": content.len()
            }),
        )?;
    }

    let python_mode = match mode.as_str() {
        "append" => "ab",
        "create" => "xb",
        "overwrite" => "wb",
        _ => {
            return Err(ToolCallError::InvalidArguments(
                "argument `mode` must be one of: overwrite, append, create".to_string(),
            ))
        }
    };
    let command = python_write_file_command(&path, content.as_bytes(), python_mode)?;
    let result = command_bus
        .run_ssh_command(SshDiagnosticInput {
            workspace_id: workspace.workspace_id,
            connection_id: connection_id.clone(),
            command,
            timeout_ms,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    Ok(json!({
        "connectionId": connection_id,
        "path": path,
        "mode": mode,
        "bytes": content.len(),
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exitCode": result.exit_status,
        "truncated": result.truncated,
        "source": "command-bus"
    }))
}

fn ssh_patch_file(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "connectionId",
            "path",
            "search",
            "replace",
            "workspaceId",
            "timeoutMs",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.ssh.patch_file")?;
    let path = parse_required_string(&arguments, "path", "unfour.ssh.patch_file")?;
    let search = parse_required_raw_string(&arguments, "search", "unfour.ssh.patch_file", false)?;
    let replace = parse_required_raw_string(&arguments, "replace", "unfour.ssh.patch_file", true)?;
    let workspace = resolve_workspace(command_bus, &arguments)?;
    let timeout_ms = parse_optional_timeout(&arguments)?;
    let multi_match_payload = json!({
        "tool": "unfour.ssh.patch_file",
        "workspaceId": workspace.workspace_id.clone(),
        "connectionId": connection_id.clone(),
        "path": path.clone(),
        "searchBytes": search.len(),
        "replaceBytes": replace.len(),
        "replaceAllMatches": true
    });
    let allow_multiple_matches = is_confirmed(
        &arguments,
        "SSH_PATCH_MULTIPLE_MATCHES",
        &multi_match_payload,
    );
    if workspace.environment_type == "test" || is_sensitive_path(&path) {
        ensure_confirmed_if_guarded(evaluation,
            &arguments,
            "SSH_PATCH_FILE",
            "Patching remote files in test or sensitive paths requires confirmation.",
            json!({
                "tool": "unfour.ssh.patch_file",
                "workspaceId": workspace.workspace_id,
                "connectionId": connection_id.clone(),
                "path": path.clone(),
                "searchBytes": search.len(),
                "replaceBytes": replace.len()
            }),
        )?;
    }

    let command = python_patch_file_command(
        &path,
        &search,
        &replace,
        if allow_multiple_matches { "1" } else { "0" },
    )?;
    let result = command_bus
        .run_ssh_command(SshDiagnosticInput {
            workspace_id: workspace.workspace_id,
            connection_id: connection_id.clone(),
            command,
            timeout_ms,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let matches = parse_match_count(&result.stdout);
    if result.exit_status == Some(3) && matches.unwrap_or(0) > 1 {
        ensure_confirmed_if_guarded(evaluation,
            &arguments,
            "SSH_PATCH_MULTIPLE_MATCHES",
            "Patching multiple remote matches requires confirmation.",
            multi_match_payload,
        )?;
    }
    let replacements = if result.exit_status == Some(0) {
        if allow_multiple_matches {
            matches.unwrap_or(1)
        } else {
            1
        }
    } else {
        0
    };
    Ok(json!({
        "connectionId": connection_id,
        "path": path,
        "patched": result.exit_status == Some(0),
        "matches": matches,
        "diffSummary": {
            "searchBytes": search.len(),
            "replaceBytes": replace.len(),
            "replacements": replacements
        },
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exitCode": result.exit_status,
        "truncated": result.truncated,
        "source": "command-bus"
    }))
}

fn ssh_list_dir(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &["connectionId", "path", "workspaceId", "limit", "timeoutMs"],
    )?;
    let connection_id = parse_required_string(&arguments, "connectionId", "unfour.ssh.list_dir")?;
    let path = parse_required_string(&arguments, "path", "unfour.ssh.list_dir")?;
    let workspace = resolve_workspace(command_bus, &arguments)?;
    let timeout_ms = parse_optional_timeout(&arguments)?;
    let limit = parse_optional_u64(&arguments, "limit")?
        .unwrap_or(200)
        .clamp(1, 1_000);
    let command = format!(
        "find {} -maxdepth 1 -mindepth 1 -printf '%f\\t%y\\t%s\\t%TY-%Tm-%TdT%TH:%TM:%TS%Tz\\n' | head -n {}",
        shell_quote(&path),
        limit
    );
    let result = command_bus
        .run_ssh_command(SshDiagnosticInput {
            workspace_id: workspace.workspace_id,
            connection_id: connection_id.clone(),
            command,
            timeout_ms,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let entries = parse_find_entries(&result.stdout);
    Ok(json!({
        "connectionId": connection_id,
        "path": path,
        "entries": entries,
        "count": entries.len(),
        "stderr": result.stderr,
        "exitCode": result.exit_status,
        "truncated": result.truncated,
        "source": "command-bus"
    }))
}

#[derive(Debug, Clone)]
struct WorkspaceContext {
    workspace_id: String,
    environment_type: String,
}

fn resolve_workspace(
    command_bus: &dyn CommandBusAdapter,
    arguments: &Map<String, Value>,
) -> Result<WorkspaceContext, ToolCallError> {
    match parse_optional_string(arguments, "workspaceId")? {
        Some(id) => {
            let result = command_bus
                .execute_read(ReadCommand::ListWorkspaces)
                .map_err(|error| ToolCallError::Execution {
                    code: error.code,
                    message: error.message,
                })?;
            let ReadCommandResult::Workspaces(workspaces) = result else {
                return Err(ToolCallError::Execution {
                    code: "COMMAND_BUS_RESULT_MISMATCH",
                    message: "The command-bus returned an unexpected result.",
                });
            };
            let workspace = workspaces
                .workspaces
                .into_iter()
                .find(|workspace| workspace.id == id)
                .ok_or(ToolCallError::Execution {
                    code: "WORKSPACE_NOT_FOUND",
                    message: "The requested workspace was not found.",
                })?;
            Ok(WorkspaceContext {
                workspace_id: workspace.id,
                environment_type: workspace.environment_type,
            })
        }
        None => {
            let result = command_bus
                .execute_read(ReadCommand::CurrentWorkspace)
                .map_err(|error| ToolCallError::Execution {
                    code: error.code,
                    message: error.message,
                })?;
            let ReadCommandResult::CurrentWorkspace(workspace) = result else {
                return Err(ToolCallError::Execution {
                    code: "COMMAND_BUS_RESULT_MISMATCH",
                    message: "The command-bus returned an unexpected result.",
                });
            };
            Ok(WorkspaceContext {
                workspace_id: workspace.workspace_id,
                environment_type: workspace.environment_type,
            })
        }
    }
}

fn ssh_command_result(result: unfour_core::models::SshDiagnosticResult, source: &str) -> Value {
    json!({
        "connectionId": result.connection_id,
        "command": redact_command_display(&result.command),
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exitCode": result.exit_status,
        "exitStatus": result.exit_status,
        "truncated": result.truncated,
        "source": source
    })
}

fn safe_connection_summary(connection: &SshConnection) -> Value {
    json!({
        "connectionId": connection.id,
        "id": connection.id,
        "workspaceId": connection.workspace_id,
        "name": connection.name,
        "host": connection.host,
        "port": connection.port,
        "username": connection.username,
        "authKind": connection.auth_kind
    })
}

fn parse_optional_port(arguments: &Map<String, Value>) -> Result<Option<u16>, ToolCallError> {
    let Some(value) = parse_optional_u64(arguments, "port")? else {
        return Ok(None);
    };
    if !(1..=u16::MAX as u64).contains(&value) {
        return Err(ToolCallError::InvalidArguments(
            "argument `port` must be between 1 and 65535".to_string(),
        ));
    }
    Ok(Some(value as u16))
}

fn parse_optional_secret(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{key}` must be a string"
        ))),
    }
}

fn python_write_file_command(
    path: &str,
    content: &[u8],
    python_mode: &str,
) -> Result<String, ToolCallError> {
    let script = "from pathlib import Path; import sys; data=bytes.fromhex(sys.argv[3]); f=Path(sys.argv[1]).open(sys.argv[2]); f.write(data); f.close()";
    ensure_one_shot_command_length(format!(
        "python3 -c {} -- {} {} {}",
        shell_quote(script),
        shell_quote(path),
        shell_quote(python_mode),
        shell_quote(&hex_encode(content))
    ))
}

fn python_patch_file_command(
    path: &str,
    search: &str,
    replace: &str,
    allow_multiple: &str,
) -> Result<String, ToolCallError> {
    let script = "import sys; from pathlib import Path; p=Path(sys.argv[1]); text=p.read_text(); search=bytes.fromhex(sys.argv[2]).decode(); replace=bytes.fromhex(sys.argv[3]).decode(); allow=sys.argv[4]=='1'; count=text.count(search); print('__UNFOUR_MATCH_COUNT__', count); sys.exit(3) if count == 0 or (count > 1 and not allow) else None; replacements=count if allow else 1; p.write_text(text.replace(search, replace, replacements)); print('__UNFOUR_PATCHED__', replacements)";
    ensure_one_shot_command_length(format!(
        "python3 -c {} -- {} {} {} {}",
        shell_quote(script),
        shell_quote(path),
        shell_quote(&hex_encode(search.as_bytes())),
        shell_quote(&hex_encode(replace.as_bytes())),
        shell_quote(allow_multiple)
    ))
}

fn ensure_one_shot_command_length(command: String) -> Result<String, ToolCallError> {
    if command.chars().count() > MAX_ONE_SHOT_COMMAND_CHARS {
        return Err(ToolCallError::Execution {
            code: "SSH_COMMAND_TOO_LARGE",
            message: "The generated SSH command exceeds the one-shot command length limit.",
        });
    }
    Ok(command)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn parse_match_count(stdout: &str) -> Option<u64> {
    stdout.lines().find_map(|line| {
        line.strip_prefix("__UNFOUR_MATCH_COUNT__")
            .and_then(|rest| rest.trim().parse::<u64>().ok())
    })
}

fn parse_find_entries(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.to_string();
            let kind = match parts.next().unwrap_or_default() {
                "d" => "directory",
                "f" => "file",
                "l" => "symlink",
                other => other,
            };
            let size = parts
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            let modified_at = parts.next().unwrap_or_default();
            Some(json!({
                "name": name,
                "type": kind,
                "size": size,
                "modifiedAt": modified_at
            }))
        })
        .collect()
}

fn resolve_workspace_id(
    command_bus: &dyn CommandBusAdapter,
    arguments: &Map<String, Value>,
) -> Result<String, ToolCallError> {
    match parse_optional_string(arguments, "workspaceId")? {
        Some(id) => Ok(id),
        None => {
            let ws_result = command_bus
                .execute_read(ReadCommand::CurrentWorkspace)
                .map_err(|e| ToolCallError::Execution {
                    code: e.code,
                    message: e.message,
                })?;
            let ReadCommandResult::CurrentWorkspace(ws) = ws_result else {
                return Err(ToolCallError::Execution {
                    code: "COMMAND_BUS_RESULT_MISMATCH",
                    message: "The command-bus returned an unexpected result.",
                });
            };
            Ok(ws.workspace_id)
        }
    }
}

fn parse_optional_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a string",
            key
        ))),
    }
}

fn parse_required_string(
    arguments: &Map<String, Value>,
    key: &str,
    tool_name: &str,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        Some(Value::String(_)) => Err(ToolCallError::InvalidArguments(format!(
            "{} argument `{}` cannot be empty",
            tool_name, key
        ))),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "{} requires argument `{}`",
            tool_name, key
        ))),
    }
}

fn parse_required_raw_string(
    arguments: &Map<String, Value>,
    key: &str,
    tool_name: &str,
    allow_empty: bool,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(s)) if allow_empty || !s.is_empty() => Ok(s.clone()),
        Some(Value::String(_)) => Err(ToolCallError::InvalidArguments(format!(
            "{} argument `{}` cannot be empty",
            tool_name, key
        ))),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "{} requires argument `{}`",
            tool_name, key
        ))),
    }
}

fn parse_optional_timeout(arguments: &Map<String, Value>) -> Result<Option<u64>, ToolCallError> {
    match arguments.get("timeoutMs") {
        None => Ok(None),
        Some(Value::Number(n)) => {
            let ms = n.as_u64().ok_or_else(|| {
                ToolCallError::InvalidArguments(
                    "argument `timeoutMs` must be a positive number".to_string(),
                )
            })?;
            Ok(Some(ms.min(MAX_DIAGNOSTIC_TIMEOUT_MS)))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `timeoutMs` must be a number".to_string(),
        )),
    }
}

#[cfg(test)]
#[path = "ssh_tests.rs"]
mod tests;
