use crate::AppState;
use tauri::State;
use unfour_core::{
    models::{
        CredentialCreateInput, CredentialDeleteInput, CredentialInspectInput, CredentialMetadata,
        CredentialRotateInput,
    },
    AppResult,
};

use super::trace_command;

#[tauri::command]
pub async fn connection_secret_reveal(
    workspace_id: String,
    connection_id: String,
    connection_type: String,
    credential_ref: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    state
        .command_bus
        .reveal_connection_secret(workspace_id, connection_id, connection_type, credential_ref)
        .await
}

#[tauri::command]
pub async fn credential_create(
    input: CredentialCreateInput,
    state: State<'_, AppState>,
) -> AppResult<CredentialMetadata> {
    trace_command(
        "credential_create",
        state.command_bus.create_credential(input),
    )
    .await
}

#[tauri::command]
pub async fn credential_delete(
    input: CredentialDeleteInput,
    state: State<'_, AppState>,
) -> AppResult<()> {
    trace_command(
        "credential_delete",
        state.command_bus.delete_credential(input),
    )
    .await
}

#[tauri::command]
pub async fn credential_inspect(
    input: CredentialInspectInput,
    state: State<'_, AppState>,
) -> AppResult<CredentialMetadata> {
    trace_command(
        "credential_inspect",
        state.command_bus.inspect_credential(input),
    )
    .await
}

#[tauri::command]
pub async fn credential_rotate(
    input: CredentialRotateInput,
    state: State<'_, AppState>,
) -> AppResult<CredentialMetadata> {
    trace_command(
        "credential_rotate",
        state.command_bus.rotate_credential(input),
    )
    .await
}
