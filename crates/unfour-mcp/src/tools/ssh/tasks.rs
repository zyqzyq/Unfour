use serde_json::{json, Value};
use unfour_core::models::{
    SshTaskCancelInput, SshTaskCleanupInput, SshTaskRunInput, SshTaskSaveInput,
    SshTasksReorderInput,
};

use crate::command_bus_adapter::CommandBusAdapter;

use super::super::confirmation::ensure_confirmed_if_guarded;
use super::super::policy::ToolPolicyEvaluation;
use super::super::{
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
};
use super::task_parse::*;
use super::task_schemas::{id_schema, run_schema, save_schema, workspace_schema};
use super::task_serialize::*;

const MAX_TASK_LOG_CHARS: usize = 128 * 1024;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        tool(
            "unfour.ssh.list_tasks",
            "List SSH Tasks",
            "Lists saved SSH task summaries for a workspace.",
            workspace_schema(),
            ToolAnnotations::local_read(),
            list_tasks,
        ),
        tool(
            "unfour.ssh.get_task",
            "Get SSH Task",
            "Returns one SSH task with steps and local connection binding. Sensitive fields inside step configuration are masked.",
            id_schema("taskId", false),
            ToolAnnotations::local_read(),
            get_task,
        ),
        tool(
            "unfour.ssh.save_task",
            "Save SSH Task",
            "Creates or updates a saved SSH task through the command bus. On update, omitted description and defaultConnectionId keep their current values; pass null for defaultConnectionId to clear it. Step types and configuration are validated by the SSH engine.",
            save_schema(),
            ToolAnnotations::local_write(),
            save_task,
        ),
        tool(
            "unfour.ssh.duplicate_task",
            "Duplicate SSH Task",
            "Duplicates a saved SSH task and its steps through the command bus.",
            id_schema("taskId", false),
            ToolAnnotations::local_write(),
            duplicate_task,
        ),
        tool(
            "unfour.ssh.reorder_tasks",
            "Reorder SSH Tasks",
            "Reorders all active SSH tasks in a workspace. taskIds must contain the exact active task set.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "taskIds": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["taskIds"],
                "additionalProperties": false
            }),
            ToolAnnotations::local_write(),
            reorder_tasks,
        ),
        tool(
            "unfour.ssh.delete_task",
            "Delete SSH Task",
            "Soft-deletes a saved SSH task. Guarded policy requires content-bound confirmation.",
            id_schema("taskId", true),
            ToolAnnotations::local_write_destructive(),
            delete_task,
        ),
        tool(
            "unfour.ssh.run_task",
            "Run SSH Task",
            "Starts a saved SSH task on its selected saved connection. Production/read-only policy blocks execution; guarded policy requires content-bound confirmation because a task may execute commands or transfer files.",
            run_schema(),
            ToolAnnotations::remote_action(),
            run_task,
        ),
        tool(
            "unfour.ssh.cancel_task_run",
            "Cancel SSH Task Run",
            "Cancels an active SSH task run through the command bus.",
            id_schema("runId", false),
            ToolAnnotations::remote_action(),
            cancel_task_run,
        ),
        tool(
            "unfour.ssh.list_task_runs",
            "List SSH Task Runs",
            "Lists saved run summaries for one SSH task without exposing local log paths.",
            id_schema("taskId", false),
            ToolAnnotations::local_read(),
            list_task_runs,
        ),
        tool(
            "unfour.ssh.read_task_run_log",
            "Read SSH Task Run Log",
            "Reads a capped SSH task run log. The SSH engine redacts configured secret inputs and the MCP response omits the local log path.",
            id_schema("runId", false),
            ToolAnnotations::local_read(),
            read_task_run_log,
        ),
        tool(
            "unfour.ssh.clear_task_runs",
            "Clear SSH Task Runs",
            "Deletes saved SSH task run records and local log files for one task or the workspace. Guarded policy requires content-bound confirmation.",
            json!({
                "type": "object",
                "properties": {
                    "workspaceId": { "type": "string" },
                    "taskId": { "type": "string" },
                    "confirm": { "type": "boolean" },
                    "confirmationText": { "type": "string" },
                    "confirmation_text": { "type": "string" }
                },
                "additionalProperties": false
            }),
            ToolAnnotations::local_write_destructive(),
            clear_task_runs,
        ),
    ]
}

fn tool(
    name: &'static str,
    title: &'static str,
    description: &'static str,
    input_schema: Value,
    annotations: ToolAnnotations,
    handler: super::super::ToolHandler,
) -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name,
            title,
            description,
            input_schema,
            output_schema: json!({ "type": "object" }),
            annotations,
        },
        handler,
    }
}

fn list_tasks(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    object_with_allowed_keys(arguments, &["workspaceId"])?;
    let tasks = command_bus
        .list_ssh_tasks(&evaluation.workspace.workspace_id)
        .map_err(execution_error)?;
    Ok(task_list_result(&tasks))
}

fn get_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "taskId"])?;
    let task_id = required_string(&arguments, "taskId")?;
    let detail = command_bus
        .get_ssh_task(&evaluation.workspace.workspace_id, &task_id)
        .map_err(execution_error)?;
    Ok(json!({ "task": safe_task_detail(&detail), "source": "command-bus" }))
}

fn save_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "taskId",
            "name",
            "description",
            "defaultConnectionId",
            "steps",
        ],
    )?;
    let task_id = optional_string(&arguments, "taskId")?;
    let existing = match &task_id {
        Some(id) => Some(
            command_bus
                .get_ssh_task(&evaluation.workspace.workspace_id, id)
                .map_err(execution_error)?,
        ),
        None => None,
    };
    let steps = parse_steps(arguments.get("steps"))?;
    let input = SshTaskSaveInput {
        id: task_id,
        workspace_id: evaluation.workspace.workspace_id.clone(),
        name: required_string(&arguments, "name")?,
        description: parse_save_description(&arguments, existing.as_ref())?,
        default_connection_id: parse_save_default_connection_id(&arguments, existing.as_ref())?,
        steps,
    };
    let detail = command_bus.save_ssh_task(input).map_err(execution_error)?;
    Ok(json!({ "task": safe_task_detail(&detail), "source": "command-bus" }))
}

fn duplicate_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "taskId"])?;
    let task_id = required_string(&arguments, "taskId")?;
    let detail = command_bus
        .duplicate_ssh_task(&evaluation.workspace.workspace_id, &task_id)
        .map_err(execution_error)?;
    Ok(json!({ "task": safe_task_detail(&detail), "source": "command-bus" }))
}

fn reorder_tasks(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "taskIds"])?;
    let task_ids = parse_string_array(arguments.get("taskIds"), "taskIds")?;
    let tasks = command_bus
        .reorder_ssh_tasks(SshTasksReorderInput {
            workspace_id: evaluation.workspace.workspace_id.clone(),
            task_ids,
        })
        .map_err(execution_error)?;
    Ok(task_list_result(&tasks))
}

fn delete_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &confirmation_keys("taskId"))?;
    let task_id = required_string(&arguments, "taskId")?;
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "SSH_DELETE_TASK",
        "Deleting an SSH task hides its saved workflow and requires confirmation under guarded policy.",
        json!({
            "tool": "unfour.ssh.delete_task",
            "workspaceId": evaluation.workspace.workspace_id,
            "taskId": task_id
        }),
    )?;
    command_bus
        .delete_ssh_task(&evaluation.workspace.workspace_id, &task_id)
        .map_err(execution_error)?;
    Ok(json!({ "deleted": true, "taskId": task_id, "source": "command-bus" }))
}

fn run_task(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "taskId",
            "connectionId",
            "inputs",
            "secretInputNames",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let task_id = required_string(&arguments, "taskId")?;
    let connection_id = optional_string(&arguments, "connectionId")?;
    let inputs = parse_inputs(arguments.get("inputs"))?;
    let secret_input_names = match arguments.get("secretInputNames") {
        None => Vec::new(),
        value => parse_string_array(value, "secretInputNames")?,
    };
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "SSH_RUN_TASK",
        "Running a saved SSH task may execute multiple remote commands or file transfers and requires confirmation under guarded policy.",
        json!({
            "tool": "unfour.ssh.run_task",
            "workspaceId": evaluation.workspace.workspace_id,
            "taskId": task_id,
            "connectionId": connection_id,
            "inputs": inputs,
            "secretInputNames": secret_input_names
        }),
    )?;
    let run = command_bus
        .run_ssh_task(SshTaskRunInput {
            workspace_id: evaluation.workspace.workspace_id.clone(),
            task_id,
            connection_id,
            inputs,
            secret_input_names,
        })
        .map_err(execution_error)?;
    Ok(json!({ "run": safe_run(&run), "source": "command-bus" }))
}

fn cancel_task_run(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "runId"])?;
    let run = command_bus
        .cancel_ssh_task_run(SshTaskCancelInput {
            workspace_id: evaluation.workspace.workspace_id.clone(),
            run_id: required_string(&arguments, "runId")?,
        })
        .map_err(execution_error)?;
    Ok(json!({ "run": safe_run(&run), "source": "command-bus" }))
}

fn list_task_runs(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "taskId"])?;
    let task_id = required_string(&arguments, "taskId")?;
    let runs = command_bus
        .list_ssh_task_runs(&evaluation.workspace.workspace_id, &task_id)
        .map_err(execution_error)?;
    Ok(json!({
        "runs": runs.iter().map(safe_run).collect::<Vec<_>>(),
        "count": runs.len(),
        "source": "command-bus"
    }))
}

fn read_task_run_log(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "runId"])?;
    let run_id = required_string(&arguments, "runId")?;
    let log = command_bus
        .read_ssh_task_run_log(&evaluation.workspace.workspace_id, &run_id)
        .map_err(execution_error)?;
    let original_chars = log.chars().count();
    let truncated = original_chars > MAX_TASK_LOG_CHARS;
    let content = if truncated {
        log.chars().take(MAX_TASK_LOG_CHARS).collect::<String>()
    } else {
        log
    };
    Ok(json!({
        "runId": run_id,
        "content": content,
        "truncated": truncated,
        "returnedChars": content.chars().count(),
        "source": "command-bus"
    }))
}

fn clear_task_runs(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "taskId",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let task_id = optional_string(&arguments, "taskId")?;
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "SSH_CLEAR_TASK_RUNS",
        "Clearing SSH task runs deletes local run records and log files and requires confirmation under guarded policy.",
        json!({
            "tool": "unfour.ssh.clear_task_runs",
            "workspaceId": evaluation.workspace.workspace_id,
            "taskId": task_id
        }),
    )?;
    let result = command_bus
        .clear_ssh_task_runs(SshTaskCleanupInput {
            workspace_id: evaluation.workspace.workspace_id.clone(),
            task_id,
        })
        .map_err(execution_error)?;
    Ok(json!({
        "deletedRuns": result.deleted_runs,
        "deletedLogs": result.deleted_logs,
        "source": "command-bus"
    }))
}
