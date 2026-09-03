//! Isolated SQLite setup and common wire/domain fixtures for worker scenarios.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::atomic::AtomicUsize;
use unfour_core::models::{ApiRequestInput, KeyValue, WorkspaceVariableInput};

pub(crate) use chrono::Utc;
pub(crate) use std::sync::atomic::Ordering;
pub(crate) use std::sync::Arc;
pub(crate) use std::time::Duration;
pub(crate) use tokio::sync::Barrier;
pub(crate) use unfour_cloud_sync::{
    ChangesPage, DownloadDecision, RemoteChange, SnapshotItem, SnapshotPage, SyncDependencies,
    SyncEntityType, SyncError, SyncOperation, SyncRuntime, PAYLOAD_SCHEMA_VERSION,
    PROTOCOL_VERSION,
};
pub(crate) use unfour_command_bus::{CommandBus, CommandBusExtensions};
pub(crate) use unfour_local_storage::LocalDb;

mod transport;
pub(crate) use transport::MockTransport;

pub(crate) fn variable(
    id: Option<String>,
    key: &str,
    value: &str,
    secret: bool,
) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id,
        key: key.into(),
        value: value.into(),
        is_secret: secret,
        is_enabled: true,
        description: Some("description".into()),
        sort_order: 0,
    }
}

pub(crate) fn saved_api_request(
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<String>,
) -> ApiRequestInput {
    ApiRequestInput {
        workspace_id: workspace_id.into(),
        name: Some("List accounts".into()),
        parent_folder_id,
        collection_id: Some(collection_id.into()),
        auth_json: Some(r#"{"type":"bearer","token":"raw-auth-secret"}"#.into()),
        method: "GET".into(),
        url: "https://example.test/accounts?api_key=raw-url-secret".into(),
        headers: vec![KeyValue {
            key: "Authorization".into(),
            value: "Bearer raw-header-secret".into(),
            enabled: true,
        }],
        query: vec![KeyValue {
            key: "token".into(),
            value: "raw-query-secret".into(),
            enabled: true,
        }],
        body: Some(r#"{"token":"raw-body-secret"}"#.into()),
        body_kind: "json".into(),
        timeout_ms: Some(12_345),
        pre_request_script: Some("console.log('pre')".into()),
        post_response_script: Some("console.log('post')".into()),
        script_schema_version: 1,
        temporary_variables: vec![KeyValue {
            key: "runtime".into(),
            value: "raw-runtime-secret".into(),
            enabled: true,
        }],
    }
}

pub(crate) async fn database() -> LocalDb {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let db = LocalDb::from_pool(pool);
    db.migrate().await.unwrap();
    unfour_cloud_sync_storage::migrate(db.pool()).await.unwrap();
    enable_test_accounts(&db).await;
    db
}

pub(crate) async fn insert_binding(
    db: &LocalDb,
    account_id: &str,
    workspace_id: &str,
    cloud_workspace_id: &str,
    sync_enabled: bool,
    state: &str,
) {
    sqlx::query(
        r#"INSERT INTO cloud_sync_workspace_bindings (
             account_id, local_workspace_id, cloud_workspace_id,
             sync_enabled, state, initial_cursor, created_at, updated_at
           ) VALUES (?1, ?2, ?3, ?4, ?5, 0,
                     '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')"#,
    )
    .bind(account_id)
    .bind(workspace_id)
    .bind(cloud_workspace_id)
    .bind(sync_enabled)
    .bind(state)
    .execute(db.pool())
    .await
    .unwrap();
}

pub(crate) async fn concurrent_database() -> LocalDb {
    static DATABASE_ID: AtomicUsize = AtomicUsize::new(0);
    let id = DATABASE_ID.fetch_add(1, Ordering::SeqCst);
    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite:file:cloud-sync-worker-{id}?mode=memory&cache=shared"
    ))
    .unwrap()
    .create_if_missing(true)
    .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    let db = LocalDb::from_pool(pool);
    db.migrate().await.unwrap();
    unfour_cloud_sync_storage::migrate(db.pool()).await.unwrap();
    enable_test_accounts(&db).await;
    db
}

async fn enable_test_accounts(db: &LocalDb) {
    // Worker tests exercise active synchronization; production accounts still
    // use the persisted default-off setting created by account activation.
    for account_id in ["account-a", "account-b"] {
        sqlx::query(
            "INSERT INTO cloud_sync_account_settings (account_id, sync_enabled, updated_at) VALUES (?1, 1, '2026-07-29T00:00:00Z')",
        )
        .bind(account_id)
        .execute(db.pool())
        .await
        .unwrap();
    }
}

pub(crate) fn workspace_payload(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name, "environmentType": "dev", "mcpPolicy": "auto",
        "createdAt": "2026-07-28T00:00:00Z", "updatedAt": "2026-07-28T00:00:00Z",
        "deletedAt": null
    })
}

pub(crate) fn variable_payload(value: &str) -> serde_json::Value {
    serde_json::json!({
        "key": "KEY", "value": value, "isSecret": false, "isEnabled": true,
        "description": null, "sortOrder": 0, "createdAt": "2026-07-28T00:00:00Z",
        "updatedAt": "2026-07-28T00:00:00Z", "deletedAt": null
    })
}

pub(crate) fn remote_variable_change(
    workspace_id: &str,
    cursor: i64,
    operation_id: &str,
    entity_id: &str,
    key: &str,
    value: &str,
) -> RemoteChange {
    let mut payload = variable_payload(value);
    payload["key"] = serde_json::json!(key);
    RemoteChange {
        cursor,
        operation_id: operation_id.into(),
        entity_type: SyncEntityType::WorkspaceVariable,
        entity_id: entity_id.into(),
        parent_entity_id: Some(workspace_id.into()),
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: 1,
        payload: Some(payload),
        deleted_at: None,
    }
}

pub(crate) fn assert_payload_keys(payload: &serde_json::Value, expected: &[&str]) {
    let mut actual = payload
        .as_object()
        .expect("canonical payload object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, expected);
}
