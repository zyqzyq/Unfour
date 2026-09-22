mod schemas;

use serde::Serialize;
use serde_json::{json, Map, Value};
use unfour_core::models::{FlowDefinition, FlowInitiator, FlowRunInput};

use super::{
    confirmation::ensure_confirmed, object_with_allowed_keys, policy::ToolPolicyEvaluation,
    RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition, ToolHandler,
};
use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    let entries: [(&str, &str, ToolHandler, ToolAnnotations); 7] = [
        ("list", "List saved Flow summaries; use get for the complete definition.", list, ToolAnnotations::local_read()),
        ("get", "Read a saved Flow definition for use in Desktop Canvas or MCP.", get, ToolAnnotations::local_read()),
        ("save", "Save a FlowDefinition using existing validation and revision checks. Empty id creates; updates require the current revision.", save, ToolAnnotations::local_write()),
        ("run", "Start a Flow with initiator=mcp. Requires payload-bound confirmation even with full_access. Read-only/disabled policy blocks execution. Cancellation does not undo effects.", run, ToolAnnotations::remote_action()),
        ("cancel_run", "Request cancellation of a running Flow; remote effects are not rolled back.", cancel, ToolAnnotations::remote_action()),
        ("list_runs", "Read lightweight FlowRunSummary history without loading run details.", list_runs, ToolAnnotations::local_read()),
        ("get_run", "Read the complete redacted Flow run history shared with Desktop.", get_run, ToolAnnotations::local_read()),
    ];
    entries
        .into_iter()
        .map(|(name, description, handler, annotations)| {
            let name = match name {
                "list" => "unfour.flow.list",
                "get" => "unfour.flow.get",
                "save" => "unfour.flow.save",
                "run" => "unfour.flow.run",
                "cancel_run" => "unfour.flow.cancel_run",
                "list_runs" => "unfour.flow.list_runs",
                _ => "unfour.flow.get_run",
            };
            RegisteredTool {
                definition: ToolDefinition {
                    name,
                    title: name,
                    description,
                    input_schema: schemas::input(name),
                    output_schema: schemas::output(name),
                    annotations,
                },
                handler,
            }
        })
        .collect()
}

fn error(e: CommandBusAdapterError) -> ToolCallError {
    ToolCallError::Execution {
        code: e.code,
        message: e.message,
    }
}
fn encode(value: impl Serialize) -> Result<Value, ToolCallError> {
    // FlowService already redacts definitions and persisted run inputs/resources/outputs.
    // Do not apply generic key masking to typed fields such as input-definition `secret`.
    serde_json::to_value(value).map_err(|_| ToolCallError::Execution {
        code: "TOOL_RESULT_SERIALIZATION_FAILED",
        message: "The Flow result could not be serialized.",
    })
}
fn args(value: Value, keys: &[&str]) -> Result<Map<String, Value>, ToolCallError> {
    object_with_allowed_keys(value, keys)
}
fn id(a: &Map<String, Value>, key: &str) -> Result<String, ToolCallError> {
    a.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty() && v.trim() == *v)
        .map(str::to_owned)
        .ok_or_else(|| {
            ToolCallError::InvalidArguments(format!(
                "{key} must be a non-empty string without surrounding whitespace"
            ))
        })
}
fn workspace(a: &Map<String, Value>, p: &ToolPolicyEvaluation) -> Result<String, ToolCallError> {
    if a.contains_key("workspaceId") {
        id(a, "workspaceId")
    } else {
        Ok(p.workspace.workspace_id.clone())
    }
}
fn list(
    b: &dyn CommandBusAdapter,
    p: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    let a = args(v, &["workspaceId"])?;
    Ok(json!({"flows": encode(b.list_flows(&workspace(&a,p)?).map_err(error)?)?}))
}
fn get(
    b: &dyn CommandBusAdapter,
    p: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    let a = args(v, &["workspaceId", "flowId"])?;
    Ok(json!({"flow": encode(b.get_flow(&workspace(&a,p)?, &id(&a,"flowId")?).map_err(error)?)?}))
}
fn save(
    b: &dyn CommandBusAdapter,
    p: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    let a = args(v, &["workspaceId", "definition"])?;
    let definition: FlowDefinition = serde_json::from_value(
        a.get("definition").cloned().unwrap_or(Value::Null),
    )
    .map_err(|_| ToolCallError::InvalidArguments("definition must be a FlowDefinition".into()))?;
    // Never allow a nested workspaceId to bypass the policy evaluated on the outer workspace.
    if definition.workspace_id != workspace(&a, p)? {
        return Err(ToolCallError::InvalidArguments(
            "definition.workspaceId must match the selected workspace".into(),
        ));
    }
    Ok(json!({"flow": encode(b.save_flow(definition).map_err(error)?)?}))
}
fn run(
    b: &dyn CommandBusAdapter,
    p: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        v,
        &[
            "workspaceId",
            "flowId",
            "environmentId",
            "inputs",
            "secretInputNames",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    for key in ["confirmationText", "confirmation_text"] {
        if a.get(key).is_some_and(|v| !v.is_string()) {
            return Err(ToolCallError::InvalidArguments(format!(
                "{key} must be a string"
            )));
        }
    }
    if a.get("confirm").is_some_and(|v| !v.is_boolean()) {
        return Err(ToolCallError::InvalidArguments(
            "confirm must be boolean".into(),
        ));
    }
    let mut input: FlowRunInput = serde_json::from_value(json!({
        "workspaceId": workspace(&a,p)?, "flowId": id(&a,"flowId")?,
        "environmentId": a.get("environmentId"), "inputs": a.get("inputs").cloned().unwrap_or(json!({})),
        "secretInputNames": a.get("secretInputNames").cloned().unwrap_or(json!([])),
        "initiator": "mcp", "confirmEffects": false
    })).map_err(|_| ToolCallError::InvalidArguments("invalid FlowRunInput fields".into()))?;
    if !input.inputs.is_object() {
        return Err(ToolCallError::InvalidArguments(
            "inputs must be an object".into(),
        ));
    }
    let definition = b
        .get_flow(&input.workspace_id, &input.flow_id)
        .map_err(error)?;
    ensure_confirmed(
        &a,
        "FLOW_RUN",
        "Flow may perform remote side effects; cancellation does not undo them.",
        json!({"definition": definition, "input": input}),
    )?;
    input.initiator = FlowInitiator::Mcp;
    input.confirm_effects = true;
    Ok(json!({"run": encode(b.run_flow(input, definition.revision).map_err(error)?)?}))
}
fn list_runs(
    b: &dyn CommandBusAdapter,
    p: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    let a = args(v, &["workspaceId", "flowId"])?;
    Ok(
        json!({"runs": encode(b.list_flow_runs(&workspace(&a,p)?, &id(&a,"flowId")?).map_err(error)?)?}),
    )
}
fn get_run(
    b: &dyn CommandBusAdapter,
    p: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    let a = args(v, &["workspaceId", "runId"])?;
    Ok(json!({"run": encode(b.get_flow_run(&workspace(&a,p)?, &id(&a,"runId")?).map_err(error)?)?}))
}
fn cancel(
    b: &dyn CommandBusAdapter,
    p: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    let a = args(v, &["workspaceId", "runId"])?;
    Ok(
        json!({"run": encode(b.cancel_flow_run(&workspace(&a,p)?, &id(&a,"runId")?).map_err(error)?)?}),
    )
}
