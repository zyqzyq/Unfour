use crate::AppState;
use tauri::State;
use unfour_core::{
    models::{
        DatabaseBrowseInput, DatabaseBrowseResult, DatabaseConnection, DatabaseConnectionInput,
        DatabaseQueryInput, DatabaseQueryResult, DatabaseRowMutationInput,
        DatabaseRowMutationResult, DatabaseSchema, DatabaseTableStructure,
        DatabaseTableStructureInput, DatabaseTestResult, DbQueryHistoryEntry,
        DbQueryHistoryRecordInput, SavedSql, SavedSqlInput,
    },
    AppResult,
};

use super::trace_command;

#[tauri::command]
pub async fn database_connections_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<DatabaseConnection>> {
    state
        .command_bus
        .list_database_connections(workspace_id)
        .await
}

#[tauri::command]
pub async fn database_connection_save(
    input: DatabaseConnectionInput,
    state: State<'_, AppState>,
) -> AppResult<DatabaseConnection> {
    state.command_bus.save_database_connection(input).await
}

#[tauri::command]
pub async fn database_connection_delete(
    workspace_id: String,
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<DatabaseConnection>> {
    state
        .command_bus
        .delete_database_connection(workspace_id, connection_id)
        .await
}

#[tauri::command]
pub async fn database_connection_test(
    workspace_id: String,
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<DatabaseTestResult> {
    trace_command(
        "database_connection_test",
        state
            .command_bus
            .test_database_connection(workspace_id, connection_id),
    )
    .await
}

#[tauri::command]
pub async fn database_connection_test_input(
    input: DatabaseConnectionInput,
    secret: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<DatabaseTestResult> {
    trace_command(
        "database_connection_test_input",
        state
            .command_bus
            .test_database_connection_input(input, secret),
    )
    .await
}

#[tauri::command]
pub async fn database_schema_get(
    workspace_id: String,
    connection_id: String,
    catalog: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<DatabaseSchema> {
    state
        .command_bus
        .database_schema(workspace_id, connection_id, catalog)
        .await
}

#[tauri::command]
pub async fn database_catalogs_list(
    workspace_id: String,
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<String>> {
    state
        .command_bus
        .database_catalogs(workspace_id, connection_id)
        .await
}

#[tauri::command]
pub async fn database_query_execute(
    input: DatabaseQueryInput,
    state: State<'_, AppState>,
) -> AppResult<DatabaseQueryResult> {
    trace_command(
        "database_query_execute",
        state.command_bus.execute_database_query(input),
    )
    .await
}

#[tauri::command]
pub async fn database_query_history_record(
    input: DbQueryHistoryRecordInput,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.command_bus.record_database_query_history(input).await
}

#[tauri::command]
pub async fn database_query_history_list(
    workspace_id: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<Vec<DbQueryHistoryEntry>> {
    state
        .command_bus
        .list_database_query_history(workspace_id, limit)
        .await
}

#[tauri::command]
pub async fn database_query_history_clear(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .command_bus
        .clear_database_query_history(workspace_id)
        .await
}

#[tauri::command]
pub async fn database_saved_sql_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SavedSql>> {
    state.command_bus.list_saved_sql(workspace_id).await
}

#[tauri::command]
pub async fn database_saved_sql_save(
    input: SavedSqlInput,
    state: State<'_, AppState>,
) -> AppResult<SavedSql> {
    state.command_bus.save_saved_sql(input).await
}

#[tauri::command]
pub async fn database_saved_sql_delete(
    workspace_id: String,
    id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<SavedSql>> {
    state.command_bus.delete_saved_sql(workspace_id, id).await
}

#[tauri::command]
pub async fn database_table_browse(
    input: DatabaseBrowseInput,
    state: State<'_, AppState>,
) -> AppResult<DatabaseBrowseResult> {
    state.command_bus.browse_database_table(input).await
}

#[tauri::command]
pub async fn database_table_structure(
    input: DatabaseTableStructureInput,
    state: State<'_, AppState>,
) -> AppResult<DatabaseTableStructure> {
    state.command_bus.database_table_structure(input).await
}

#[tauri::command]
pub async fn database_row_mutate(
    input: DatabaseRowMutationInput,
    state: State<'_, AppState>,
) -> AppResult<DatabaseRowMutationResult> {
    state.command_bus.mutate_database_row(input).await
}
