//! No credential injection command. Only opaque handles cross this adapter.
use crate::AppState;
use tauri::State;
use unfour_command_bus::TemporarySummary;
use unfour_core::{
    models::{SshResizeInput, SshSessionEvent, SshSessionInput},
    AppResult,
};

#[tauri::command]
pub fn ssh_temporary_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<TemporarySummary>> {
    state.command_bus.list_temporary_ssh(&workspace_id)
}
#[tauri::command]
pub async fn ssh_temporary_claim(
    workspace_id: String,
    handle: String,
    state: State<'_, AppState>,
) -> AppResult<TemporarySummary> {
    state
        .command_bus
        .claim_temporary_ssh(&workspace_id, &handle)
        .await
}
#[tauri::command]
pub async fn ssh_temporary_start(
    workspace_id: String,
    handle: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .command_bus
        .start_temporary_ssh(&workspace_id, &handle)
}
#[tauri::command]
pub fn ssh_temporary_trust(
    workspace_id: String,
    handle: String,
    fingerprint: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .command_bus
        .trust_temporary_ssh(&workspace_id, &handle, &fingerprint)
}
#[tauri::command]
pub fn ssh_temporary_cancel(
    workspace_id: String,
    handle: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .command_bus
        .cancel_temporary_ssh(&workspace_id, &handle)
}
#[tauri::command]
pub fn ssh_temporary_close_workspace(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .command_bus
        .close_temporary_ssh_workspace(&workspace_id)
}
#[tauri::command]
pub fn ssh_temporary_output(
    workspace_id: String,
    handle: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SshSessionEvent>> {
    state
        .command_bus
        .temporary_ssh_output(&workspace_id, &handle)
}
#[tauri::command]
pub async fn ssh_temporary_input(
    input: SshSessionInput,
    state: State<'_, AppState>,
) -> AppResult<SshSessionEvent> {
    state.command_bus.temporary_ssh_input(input).await
}
#[tauri::command]
pub async fn ssh_temporary_resize(
    input: SshResizeInput,
    state: State<'_, AppState>,
) -> AppResult<SshSessionEvent> {
    state.command_bus.temporary_ssh_resize(input).await
}
