use super::metadata_support::*;
use super::policy::ToolPolicyEvaluation;
use super::{RegisteredTool, ToolAnnotations, ToolCallError};
use crate::command_bus_adapter::CommandBusAdapter;
use serde_json::{json, Value};
use unfour_core::models::Workspace;

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    let output = result_schema(json!({"workspace":result_schema(json!({
        "id":{"type":"string"},"name":{"type":"string"},
        "environmentType":{"type":"string"},"mcpPolicy":{"type":"string"}
    }))}));
    vec![
        tool("unfour.workspace.create", "Creates a local workspace without activating it. Requires the current workspace to allow MCP writes. Inherits its environment and MCP policy; no policy override, cloud binding, or resource copying is accepted.",
            json!({"name":{"type":"string","minLength":1}}), &["name"], output.clone(), ToolAnnotations::local_write(), create),
        tool("unfour.workspace.rename", "Renames the explicitly selected workspace through CommandBus. Supply expectedName from workspace.list to prevent overwriting a concurrent rename. Does not change environment, policy or active workspace.",
            json!({"workspaceId":{"type":"string","minLength":1},"name":{"type":"string","minLength":1},"expectedName":{"type":"string","minLength":1}}),
            &["workspaceId","name","expectedName"], output, ToolAnnotations::local_write(), rename),
    ]
}
fn projection(w: Workspace) -> Value {
    json!({"workspace":{"id":w.id,"name":safe_label(&w.name),"environmentType":w.environment_type,"mcpPolicy":w.mcp_policy}})
}
fn create(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &["name"])?;
    bus.create_workspace_inactive(
        required(&a, "name")?,
        e.workspace.workspace_id.clone(),
        e.workspace.environment_type.clone(),
        e.workspace.mcp_policy.clone(),
    )
    .map(projection)
    .map_err(error)
}
fn rename(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &["workspaceId", "name", "expectedName"])?;
    required(&a, "workspaceId")?;
    bus.rename_workspace(
        e.workspace.workspace_id.clone(),
        required(&a, "name")?,
        required(&a, "expectedName")?,
    )
    .map(projection)
    .map_err(error)
}
