use crate::AppState;
use tauri::State;
use unfour_core::{models::*, AppResult};

#[tauri::command]
pub async fn flow_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<FlowDefinition>> {
    state.command_bus.list_flows(workspace_id).await
}

#[tauri::command]
pub async fn flow_get(
    workspace_id: String,
    flow_id: String,
    state: State<'_, AppState>,
) -> AppResult<FlowDefinition> {
    state.command_bus.get_flow(workspace_id, flow_id).await
}

#[tauri::command]
pub async fn flow_delete(
    workspace_id: String,
    flow_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.command_bus.delete_flow(workspace_id, flow_id).await
}

#[tauri::command]
pub async fn flow_runs_list(
    workspace_id: String,
    flow_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<FlowRunSummary>> {
    state
        .command_bus
        .list_flow_runs(workspace_id, flow_id)
        .await
}

#[tauri::command]
pub async fn flow_run_get(
    workspace_id: String,
    run_id: String,
    state: State<'_, AppState>,
) -> AppResult<FlowRun> {
    state.command_bus.get_flow_run(workspace_id, run_id).await
}

#[tauri::command]
pub async fn flow_run_cancel(
    workspace_id: String,
    run_id: String,
    state: State<'_, AppState>,
) -> AppResult<FlowRun> {
    state
        .command_bus
        .cancel_flow_run(workspace_id, run_id)
        .await
}

#[tauri::command]
pub async fn flow_save(
    input: FlowDefinition,
    state: State<'_, AppState>,
) -> AppResult<FlowDefinition> {
    state.command_bus.save_flow(input).await
}

#[tauri::command]
pub async fn flow_run(input: FlowRunInput, state: State<'_, AppState>) -> AppResult<FlowRun> {
    state.command_bus.run_flow(input).await
}
