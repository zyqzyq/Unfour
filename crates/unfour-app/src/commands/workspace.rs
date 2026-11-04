use crate::AppState;
use tauri::State;
use unfour_core::{
    models::{
        Workspace, WorkspaceEnvironment, WorkspaceEnvironmentVariable, WorkspaceLayout,
        WorkspaceState, WorkspaceVariable, WorkspaceVariableInput,
    },
    AppResult,
};

#[tauri::command]
pub async fn workspace_list(state: State<'_, AppState>) -> AppResult<WorkspaceState> {
    state.command_bus.list_workspaces().await
}

#[tauri::command]
pub async fn workspace_create(
    name: String,
    environment_type: Option<String>,
    mcp_policy: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Workspace> {
    state
        .command_bus
        .create_workspace_with_options(name, environment_type, mcp_policy)
        .await
}

#[tauri::command]
pub async fn workspace_update_environment(
    workspace_id: String,
    environment_type: String,
    expected: Option<Workspace>,
    state: State<'_, AppState>,
) -> AppResult<Workspace> {
    state
        .command_bus
        .update_workspace_environment_checked(workspace_id, environment_type, expected)
        .await
}

#[tauri::command]
pub async fn workspace_update_mcp_policy(
    workspace_id: String,
    mcp_policy: String,
    expected: Option<Workspace>,
    state: State<'_, AppState>,
) -> AppResult<Workspace> {
    state
        .command_bus
        .update_workspace_mcp_policy_checked(workspace_id, mcp_policy, expected)
        .await
}

#[tauri::command]
pub async fn workspace_set_default(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceState> {
    state.command_bus.set_default_workspace(workspace_id).await
}

#[tauri::command]
pub async fn workspace_rename(
    workspace_id: String,
    name: String,
    expected_name: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Workspace> {
    state
        .command_bus
        .rename_workspace_checked(workspace_id, name, expected_name)
        .await
}

#[tauri::command]
pub async fn workspace_delete(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceState> {
    state.command_bus.delete_workspace(workspace_id).await
}

#[tauri::command]
pub async fn workspace_set_active(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceState> {
    state.command_bus.set_active_workspace(workspace_id).await
}

#[tauri::command]
pub async fn workspace_layout_get(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceLayout> {
    state.command_bus.workspace_layout(workspace_id).await
}

#[tauri::command]
pub async fn workspace_layout_update(
    workspace_id: String,
    layout: WorkspaceLayout,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceLayout> {
    state
        .command_bus
        .workspace_layout_update(workspace_id, layout)
        .await
}

#[tauri::command]
pub async fn workspace_variables_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceVariable>> {
    state
        .command_bus
        .workspace_variables_list(workspace_id)
        .await
}

#[tauri::command]
pub async fn workspace_variables_replace(
    workspace_id: String,
    variables: Vec<WorkspaceVariableInput>,
    expected: Option<Vec<WorkspaceVariable>>,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceVariable>> {
    state
        .command_bus
        .workspace_variables_replace_checked(workspace_id, variables, expected)
        .await
}

#[tauri::command]
pub async fn workspace_variable_create(
    workspace_id: String,
    input: WorkspaceVariableInput,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceVariable> {
    state
        .command_bus
        .workspace_variable_create(workspace_id, input)
        .await
}

#[tauri::command]
pub async fn workspace_variable_update(
    workspace_id: String,
    variable_id: String,
    input: WorkspaceVariableInput,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceVariable> {
    state
        .command_bus
        .workspace_variable_update(workspace_id, variable_id, input)
        .await
}

#[tauri::command]
pub async fn workspace_variable_delete(
    workspace_id: String,
    variable_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceVariable>> {
    state
        .command_bus
        .workspace_variable_delete(workspace_id, variable_id)
        .await
}

#[tauri::command]
pub async fn workspace_environments_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceEnvironment>> {
    state
        .command_bus
        .workspace_environments_list(workspace_id)
        .await
}

#[tauri::command]
pub async fn workspace_environment_create(
    workspace_id: String,
    name: String,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceEnvironment> {
    workspace_environment_create_via_bus(&state.command_bus, workspace_id, name).await
}

async fn workspace_environment_create_via_bus(
    command_bus: &unfour_command_bus::CommandBus,
    workspace_id: String,
    name: String,
) -> AppResult<WorkspaceEnvironment> {
    command_bus
        .workspace_environment_create(workspace_id, name)
        .await
}

#[tauri::command]
pub async fn workspace_environment_update(
    workspace_id: String,
    environment_id: String,
    name: String,
    variables: Vec<WorkspaceVariableInput>,
    expected: Option<WorkspaceEnvironment>,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceEnvironment> {
    state
        .command_bus
        .workspace_environment_update_checked(
            workspace_id,
            environment_id,
            name,
            variables,
            expected,
        )
        .await
}

#[tauri::command]
pub async fn workspace_environment_update_metadata(
    workspace_id: String,
    environment_id: String,
    name: String,
    sort_order: i64,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceEnvironment> {
    state
        .command_bus
        .workspace_environment_update_metadata(workspace_id, environment_id, name, sort_order)
        .await
}

#[tauri::command]
pub async fn workspace_environments_reorder(
    workspace_id: String,
    environment_ids: Vec<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceEnvironment>> {
    state
        .command_bus
        .workspace_environments_reorder(workspace_id, environment_ids)
        .await
}

#[tauri::command]
pub async fn workspace_environment_delete(
    workspace_id: String,
    environment_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceEnvironment>> {
    state
        .command_bus
        .workspace_environment_delete(workspace_id, environment_id)
        .await
}

#[tauri::command]
pub async fn workspace_environment_set_active(
    workspace_id: String,
    environment_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceEnvironment>> {
    state
        .command_bus
        .workspace_environment_set_active(workspace_id, environment_id)
        .await
}

#[tauri::command]
pub async fn workspace_environment_variable_create(
    workspace_id: String,
    environment_id: String,
    input: WorkspaceVariableInput,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceEnvironmentVariable> {
    state
        .command_bus
        .workspace_environment_variable_create(workspace_id, environment_id, input)
        .await
}

#[tauri::command]
pub async fn workspace_environment_variable_update(
    workspace_id: String,
    environment_id: String,
    variable_id: String,
    input: WorkspaceVariableInput,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceEnvironmentVariable> {
    state
        .command_bus
        .workspace_environment_variable_update(workspace_id, environment_id, variable_id, input)
        .await
}

#[tauri::command]
pub async fn workspace_environment_variables_replace(
    workspace_id: String,
    environment_id: String,
    variables: Vec<WorkspaceVariableInput>,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceEnvironmentVariable>> {
    state
        .command_bus
        .workspace_environment_variables_replace(workspace_id, environment_id, variables)
        .await
}

#[tauri::command]
pub async fn workspace_environment_variable_delete(
    workspace_id: String,
    environment_id: String,
    variable_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<WorkspaceEnvironmentVariable>> {
    state
        .command_bus
        .workspace_environment_variable_delete(workspace_id, environment_id, variable_id)
        .await
}

#[tauri::command]
pub async fn workspace_variables_resolve(
    workspace_id: String,
    active_environment_id: Option<String>,
    input: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    state
        .command_bus
        .workspace_variables_resolve(workspace_id, active_environment_id, input)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use unfour_command_bus::{CommandBus, ReadCommand, ReadCommandResult};

    #[test]
    fn tauri_workspace_write_adapter_uses_command_bus_coordinator() {
        tauri::async_runtime::block_on(async {
            let bus = CommandBus::ephemeral().await.expect("create command bus");
            let state = bus.list_workspaces().await.expect("list workspaces");
            workspace_environment_create_via_bus(
                &bus,
                state.active_workspace_id.clone(),
                "Tauri path".to_string(),
            )
            .await
            .expect("create environment through Tauri adapter helper");

            let activity = bus
                .execute_read(ReadCommand::ListActivity {
                    workspace_id: Some(state.active_workspace_id),
                    limit: Some(10),
                })
                .await
                .expect("read activity");
            let ReadCommandResult::Activity(activity) = activity else {
                panic!("expected activity result");
            };
            assert!(activity
                .activity
                .iter()
                .any(|item| item.action == "workspace.environment.create"));
        });
    }
}
