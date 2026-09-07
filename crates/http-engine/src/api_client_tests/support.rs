use super::super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_core::models::{ApiRequestInput, ApiSavedRequest};

pub(super) async fn service() -> ApiClientService {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory sqlite");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("run migrations");
    seed_workspace(&db, "workspace-a").await;
    seed_workspace(&db, "workspace-b").await;
    ApiClientService::new(db)
}

pub(super) async fn seed_workspace(db: &LocalDb, workspace_id: &str) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, last_opened_at, created_at, updated_at,
          revision, sync_status
        )
        VALUES (?1, ?1, 0, ?2, ?2, ?2, 1, 'local')
        "#,
    )
    .bind(workspace_id)
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert workspace");

    sqlx::query(
        r#"
        INSERT INTO workspace_settings (
          workspace_id, layout_json, created_at, updated_at,
          revision, sync_status
        )
        VALUES (?1, '{}', ?2, ?2, 1, 'local')
        "#,
    )
    .bind(workspace_id)
    .bind(now)
    .execute(db.pool())
    .await
    .expect("insert workspace settings");
}

pub(super) async fn save_in_collection(
    service: &ApiClientService,
    workspace_id: &str,
    name: &str,
    collection_id: Option<String>,
) -> ApiSavedRequest {
    service
        .save_request(ApiRequestInput {
            workspace_id: workspace_id.to_string(),
            name: Some(name.to_string()),
            parent_folder_id: None,
            collection_id,
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            multipart_parts: vec![],
            temporary_variables: vec![],
        })
        .await
        .expect("save request")
}
