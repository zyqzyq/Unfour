use super::*;

#[tokio::test]
async fn initial_schema_enforces_environment_name_uniqueness() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");
    seed_workspace(&db, "ws-env").await;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO api_environments (
          id, workspace_id, name, is_active, created_at, updated_at, revision, sync_status
        )
        VALUES ('env-1', 'ws-env', 'Dev', 0, ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert first environment");

    let duplicate = sqlx::query(
        r#"
        INSERT INTO api_environments (
          id, workspace_id, name, is_active, created_at, updated_at, revision, sync_status
        )
        VALUES ('env-2', 'ws-env', 'dev', 0, ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await;

    assert!(
        duplicate.is_err(),
        "environment names should be unique per workspace ignoring case"
    );
}

#[tokio::test]
async fn initial_schema_rejects_cross_workspace_request_locations() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");
    seed_workspace(&db, "ws-a").await;
    seed_workspace(&db, "ws-b").await;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO api_collections (
          id, workspace_id, name, created_at, updated_at, revision, sync_status
        )
        VALUES ('collection-a', 'ws-a', 'A', ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert collection");
    sqlx::query(
        r#"
        INSERT INTO api_collection_folders (
          id, workspace_id, collection_id, name, created_at, updated_at,
          revision, sync_status
        )
        VALUES ('folder-a', 'ws-a', 'collection-a', 'Folder', ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert folder");

    let wrong_workspace = sqlx::query(
        r#"
        INSERT INTO api_requests (
          id, workspace_id, name, collection_id, parent_folder_id, sort_order,
          auth_json, method, url, headers_json, query_json, body_kind,
          created_at, updated_at, revision, sync_status
        )
        VALUES (
          'request-b', 'ws-b', 'Wrong', 'collection-a', 'folder-a', 0,
          '{"type":"none"}', 'GET', 'https://example.test', '[]', '[]', 'json',
          ?1, ?1, 1, 'local'
        )
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await;

    assert!(
        wrong_workspace.is_err(),
        "request collection/folder references must stay inside the same workspace"
    );
}

#[tokio::test]
async fn initial_schema_nulls_saved_sql_connection_on_hard_delete() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");
    seed_workspace(&db, "ws-sql").await;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO connections (
          id, workspace_id, connection_type, name, created_at, updated_at, revision, sync_status
        )
        VALUES ('conn-1', 'ws-sql', 'database', 'DB', ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert connection");
    sqlx::query(
        r#"
        INSERT INTO database_connections (connection_id, driver, config_json)
        VALUES ('conn-1', 'sqlite', '{}')
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert database subtype");
    sqlx::query(
        r#"
        INSERT INTO saved_sql (
          id, workspace_id, connection_id, name, sql, created_at, updated_at,
          revision, sync_status
        )
        VALUES ('saved-1', 'ws-sql', 'conn-1', 'Saved', 'SELECT 1', ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert saved sql");

    sqlx::query("DELETE FROM connections WHERE id = 'conn-1'")
        .execute(db.pool())
        .await
        .expect("hard-delete connection");

    let connection_id: (Option<String>,) =
        sqlx::query_as("SELECT connection_id FROM saved_sql WHERE id = 'saved-1'")
            .fetch_one(db.pool())
            .await
            .expect("read saved sql");
    assert!(connection_id.0.is_none());
}

#[tokio::test]
async fn initial_schema_scopes_ssh_host_keys_to_workspace() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");
    seed_workspace(&db, "ws-a").await;
    seed_workspace(&db, "ws-b").await;
    let now = Utc::now().to_rfc3339();

    for workspace_id in ["ws-a", "ws-b"] {
        sqlx::query(
            r#"
            INSERT INTO ssh_host_keys (
              workspace_id, host, port, fingerprint, created_at
            )
            VALUES (?1, 'example.test', 22, ?2, ?3)
            "#,
        )
        .bind(workspace_id)
        .bind(format!("SHA256:{workspace_id}"))
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("same host can be trusted separately per workspace");
    }

    let duplicate = sqlx::query(
        r#"
        INSERT INTO ssh_host_keys (
          workspace_id, host, port, fingerprint, created_at
        )
        VALUES ('ws-a', 'example.test', 22, 'SHA256:dup', ?1)
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await;
    assert!(
        duplicate.is_err(),
        "same workspace/host/port should remain unique"
    );
}

#[tokio::test]
async fn initial_schema_rejects_invalid_workspace_policy_values() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");
    let now = Utc::now().to_rfc3339();

    let invalid_environment = sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, environment_type, mcp_policy,
          created_at, updated_at, revision, sync_status
        )
        VALUES ('bad-env', 'Bad', 0, 'stage', 'auto', ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await;

    let invalid_policy = sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, environment_type, mcp_policy,
          created_at, updated_at, revision, sync_status
        )
        VALUES ('bad-policy', 'Bad', 0, 'dev', 'unsafe', ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await;

    assert!(invalid_environment.is_err());
    assert!(invalid_policy.is_err());
}
