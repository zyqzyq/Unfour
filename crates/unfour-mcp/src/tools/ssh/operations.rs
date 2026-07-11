use serde_json::{json, Value};
use unfour_core::models::SshDiagnosticInput;

use crate::command_bus_adapter::CommandBusAdapter;

use super::super::policy::ToolPolicyEvaluation;
use super::super::ssh_risk::{
    build_ssh_exec_command, classify_high_risk_command, is_readonly_ssh_command, is_sensitive_path,
    parse_optional_u64, shell_quote,
};
use super::super::{
    confirmation::{ensure_confirmed_if_guarded, is_confirmed},
    object_with_allowed_keys, ToolCallError,
};
use super::helpers::*;
use super::{DEFAULT_FILE_LIMIT, MAX_FILE_LIMIT};

pub(super) fn ssh_exec(
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
        ensure_confirmed_if_guarded(
            evaluation,
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

pub(super) fn ssh_read_file(
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

pub(super) fn ssh_write_file(
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
        ensure_confirmed_if_guarded(
            evaluation,
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

pub(super) fn ssh_patch_file(
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
        ensure_confirmed_if_guarded(
            evaluation,
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
        ensure_confirmed_if_guarded(
            evaluation,
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

pub(super) fn ssh_list_dir(
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
