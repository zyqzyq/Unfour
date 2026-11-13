use crate::AppState;
use tauri::State;
use unfour_command_bus::{RedisConnection, RedisConnectionInput, RedisKeyInfo, RedisScanPage};
use unfour_core::AppResult;

#[tauri::command]
pub async fn redis_connections_list(
    workspace_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<RedisConnection>> {
    state
        .command_bus
        .list_redis_connections(&workspace_id)
        .await
}

#[tauri::command]
pub async fn redis_connection_save(
    input: RedisConnectionInput,
    secret: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<RedisConnection> {
    state.command_bus.save_redis_connection(input, secret).await
}

#[tauri::command]
pub async fn redis_connection_delete(
    workspace_id: String,
    connection_id: String,
    expected_revision: i64,
    state: State<'_, AppState>,
) -> AppResult<Vec<RedisConnection>> {
    state
        .command_bus
        .delete_redis_connection(workspace_id, connection_id, expected_revision)
        .await
}

#[tauri::command]
pub async fn redis_connection_test_input(
    input: RedisConnectionInput,
    secret: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .command_bus
        .test_redis_connection_input(input, secret)
        .await
}

#[tauri::command]
pub async fn redis_key_inspect(
    workspace_id: String,
    connection_id: String,
    key_base64: String,
    field_base64: Option<String>,
    expected_revision: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<RedisKeyInfo> {
    state
        .command_bus
        .inspect_redis_key(
            &workspace_id,
            &connection_id,
            &key_base64,
            field_base64.as_deref(),
            expected_revision,
        )
        .await
}

#[tauri::command]
pub async fn redis_keys_scan(
    workspace_id: String,
    connection_id: String,
    cursor: String,
    pattern: String,
    expected_revision: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<RedisScanPage> {
    state
        .command_bus
        .scan_redis_keys(
            &workspace_id,
            &connection_id,
            &cursor,
            &pattern,
            expected_revision,
        )
        .await
}
