use crate::AppState;
use tauri::State;
use unfour_core::{
    models::{
        SftpCancelTransferInput, SftpDeleteInput, SftpDirectoryListing, SftpFileEntry,
        SftpOpenResult, SftpPathInput, SftpRenameInput, SftpSessionInput, SftpTransferInput,
        SftpTransferState, SshCloseInput, SshConnectInput, SshConnection, SshConnectionInput,
        SshHostFingerprintInfo, SshHostKeyInput, SshKnownHostsExportInput,
        SshKnownHostsExportResult, SshKnownHostsImportInput, SshKnownHostsImportResult,
        SshLogExport, SshLogExportInput, SshReconnectCancelInput, SshResizeInput, SshSessionEvent,
        SshSessionInput, SshSessionSummary, SshTask, SshTaskCancelInput, SshTaskCleanupInput,
        SshTaskCleanupResult, SshTaskDetail, SshTaskRun, SshTaskRunInput, SshTaskSaveInput,
        SshTestResult,
    },
    AppResult,
};

use super::trace_command;

#[tauri::command]
pub async fn ssh_tasks_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SshTask>> {
    state.command_bus.list_ssh_tasks(workspace_id).await
}

#[tauri::command]
pub async fn ssh_task_get(
    workspace_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> AppResult<SshTaskDetail> {
    state.command_bus.get_ssh_task(workspace_id, task_id).await
}

#[tauri::command]
pub async fn ssh_task_save(
    input: SshTaskSaveInput,
    state: State<'_, AppState>,
) -> AppResult<SshTaskDetail> {
    state.command_bus.save_ssh_task(input).await
}

#[tauri::command]
pub async fn ssh_task_duplicate(
    workspace_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> AppResult<SshTaskDetail> {
    state
        .command_bus
        .duplicate_ssh_task(workspace_id, task_id)
        .await
}

#[tauri::command]
pub async fn ssh_task_delete(
    workspace_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .command_bus
        .delete_ssh_task(workspace_id, task_id)
        .await
}

#[tauri::command]
pub async fn ssh_task_run(
    input: SshTaskRunInput,
    state: State<'_, AppState>,
) -> AppResult<SshTaskRun> {
    trace_command("ssh_task_run", state.command_bus.run_ssh_task(input)).await
}

#[tauri::command]
pub async fn ssh_task_run_cancel(
    input: SshTaskCancelInput,
    state: State<'_, AppState>,
) -> AppResult<SshTaskRun> {
    state.command_bus.cancel_ssh_task_run(input).await
}

#[tauri::command]
pub async fn ssh_task_runs_list(
    workspace_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SshTaskRun>> {
    state
        .command_bus
        .list_ssh_task_runs(workspace_id, task_id)
        .await
}

#[tauri::command]
pub async fn ssh_task_run_log_read(
    workspace_id: String,
    run_id: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    state
        .command_bus
        .read_ssh_task_run_log(workspace_id, run_id)
        .await
}

#[tauri::command]
pub async fn ssh_task_runs_clear(
    input: SshTaskCleanupInput,
    state: State<'_, AppState>,
) -> AppResult<SshTaskCleanupResult> {
    state.command_bus.clear_ssh_task_runs(input).await
}

#[tauri::command]
pub async fn ssh_register_task_run_channel(
    channel: tauri::ipc::Channel<serde_json::Value>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    if let Ok(mut guard) = state.task_run_channel.lock() {
        *guard = Some(channel);
    }
    Ok(())
}

#[tauri::command]
pub async fn ssh_connections_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SshConnection>> {
    state.command_bus.list_ssh_connections(workspace_id).await
}

#[tauri::command]
pub async fn ssh_connection_save(
    input: SshConnectionInput,
    state: State<'_, AppState>,
) -> AppResult<SshConnection> {
    state.command_bus.save_ssh_connection(input).await
}

#[tauri::command]
pub async fn ssh_connection_test(
    input: SshConnectionInput,
    state: State<'_, AppState>,
) -> AppResult<SshTestResult> {
    state.command_bus.test_ssh_connection(input).await
}

#[tauri::command]
pub async fn ssh_connection_delete(
    workspace_id: String,
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SshConnection>> {
    state
        .command_bus
        .delete_ssh_connection(workspace_id, connection_id)
        .await
}

#[tauri::command]
pub async fn ssh_session_connect(
    input: SshConnectInput,
    state: State<'_, AppState>,
) -> AppResult<SshSessionSummary> {
    trace_command(
        "ssh_session_connect",
        state.command_bus.connect_ssh_session(input),
    )
    .await
}

#[tauri::command]
pub async fn ssh_sessions_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SshSessionSummary>> {
    state.command_bus.list_ssh_sessions(workspace_id).await
}

#[tauri::command]
pub async fn ssh_session_history(
    input: SshCloseInput,
    state: State<'_, AppState>,
) -> AppResult<Vec<SshSessionEvent>> {
    state.command_bus.ssh_session_history(input).await
}

#[tauri::command]
pub async fn ssh_session_input(
    input: SshSessionInput,
    state: State<'_, AppState>,
) -> AppResult<SshSessionEvent> {
    state.command_bus.send_ssh_input(input).await
}

/// Register the frontend's IPC channel as the live terminal-output sink. Live
/// SSH output then streams over this channel instead of the Tauri event system,
/// which stalls under the high-rate emit burst of a full-screen redraw on
/// WebView2/Windows. The most recently registered channel wins.
#[tauri::command]
pub async fn ssh_register_terminal_channel(
    channel: tauri::ipc::Channel<serde_json::Value>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    if let Ok(mut guard) = state.terminal_channel.lock() {
        *guard = Some(channel);
    }
    Ok(())
}

#[tauri::command]
pub async fn ssh_register_sftp_transfer_channel(
    channel: tauri::ipc::Channel<serde_json::Value>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    if let Ok(mut guard) = state.sftp_transfer_channel.lock() {
        *guard = Some(channel);
    }
    Ok(())
}

#[tauri::command]
pub async fn ssh_sftp_open(
    input: SftpSessionInput,
    state: State<'_, AppState>,
) -> AppResult<SftpOpenResult> {
    trace_command("ssh_sftp_open", state.command_bus.open_ssh_sftp(input)).await
}

#[tauri::command]
pub async fn ssh_sftp_list_directory(
    input: SftpPathInput,
    state: State<'_, AppState>,
) -> AppResult<SftpDirectoryListing> {
    state.command_bus.list_ssh_sftp_directory(input).await
}

#[tauri::command]
pub async fn ssh_sftp_stat(
    input: SftpPathInput,
    state: State<'_, AppState>,
) -> AppResult<SftpFileEntry> {
    state.command_bus.stat_ssh_sftp_path(input).await
}

#[tauri::command]
pub async fn ssh_sftp_create_directory(
    input: SftpPathInput,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.command_bus.create_ssh_sftp_directory(input).await
}

#[tauri::command]
pub async fn ssh_sftp_rename(input: SftpRenameInput, state: State<'_, AppState>) -> AppResult<()> {
    state.command_bus.rename_ssh_sftp_path(input).await
}

#[tauri::command]
pub async fn ssh_sftp_delete(input: SftpDeleteInput, state: State<'_, AppState>) -> AppResult<()> {
    state.command_bus.delete_ssh_sftp_path(input).await
}

#[tauri::command]
pub async fn ssh_sftp_download(
    input: SftpTransferInput,
    state: State<'_, AppState>,
) -> AppResult<SftpTransferState> {
    state.command_bus.download_ssh_sftp_file(input).await
}

#[tauri::command]
pub async fn ssh_sftp_upload(
    input: SftpTransferInput,
    state: State<'_, AppState>,
) -> AppResult<SftpTransferState> {
    state.command_bus.upload_ssh_sftp_file(input).await
}

#[tauri::command]
pub async fn ssh_sftp_cancel_transfer(
    input: SftpCancelTransferInput,
    state: State<'_, AppState>,
) -> AppResult<SftpTransferState> {
    state.command_bus.cancel_ssh_sftp_transfer(input).await
}

#[tauri::command]
pub async fn ssh_sftp_transfers_list(
    input: SftpSessionInput,
    state: State<'_, AppState>,
) -> AppResult<Vec<SftpTransferState>> {
    state.command_bus.list_ssh_sftp_transfers(input).await
}

#[tauri::command]
pub async fn ssh_session_resize(
    input: SshResizeInput,
    state: State<'_, AppState>,
) -> AppResult<SshSessionEvent> {
    trace_command(
        "ssh_session_resize",
        state.command_bus.resize_ssh_session(input),
    )
    .await
}

#[tauri::command]
pub async fn ssh_session_close(
    input: SshCloseInput,
    state: State<'_, AppState>,
) -> AppResult<SshSessionSummary> {
    trace_command(
        "ssh_session_close",
        state.command_bus.close_ssh_session(input),
    )
    .await
}

#[tauri::command]
pub async fn ssh_session_reconnect_cancel(
    input: SshReconnectCancelInput,
    state: State<'_, AppState>,
) -> AppResult<SshSessionSummary> {
    state.command_bus.cancel_ssh_reconnect(input).await
}

#[tauri::command]
pub async fn ssh_session_log_export(
    input: SshLogExportInput,
    state: State<'_, AppState>,
) -> AppResult<SshLogExport> {
    state.command_bus.export_ssh_log(input).await
}

#[tauri::command]
pub async fn ssh_host_key_get(
    input: SshHostKeyInput,
    state: State<'_, AppState>,
) -> AppResult<Option<SshHostFingerprintInfo>> {
    state.command_bus.get_ssh_host_fingerprint(input).await
}

#[tauri::command]
pub async fn ssh_host_key_reset(
    input: SshHostKeyInput,
    state: State<'_, AppState>,
) -> AppResult<bool> {
    state.command_bus.reset_ssh_host_fingerprint(input).await
}

#[tauri::command]
pub async fn ssh_host_key_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SshHostFingerprintInfo>> {
    state
        .command_bus
        .list_all_ssh_fingerprints(workspace_id)
        .await
}

#[tauri::command]
pub async fn ssh_known_hosts_import(
    input: SshKnownHostsImportInput,
    state: State<'_, AppState>,
) -> AppResult<SshKnownHostsImportResult> {
    state.command_bus.import_ssh_known_hosts(input).await
}

#[tauri::command]
pub async fn ssh_known_hosts_export(
    input: SshKnownHostsExportInput,
    state: State<'_, AppState>,
) -> AppResult<SshKnownHostsExportResult> {
    state.command_bus.export_ssh_known_hosts(input).await
}
