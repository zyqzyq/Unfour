use super::*;

#[tokio::test]
async fn initial_schema_keeps_api_history_local_only() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('api_history')")
            .fetch_all(db.pool())
            .await
            .expect("list columns");
    let names: Vec<&str> = columns.iter().map(|(name,)| name.as_str()).collect();

    assert!(!names.contains(&"revision"), "revision should be dropped");
    assert!(
        !names.contains(&"sync_status"),
        "sync_status should be dropped"
    );
    assert!(!names.contains(&"remote_id"), "remote_id should be dropped");
    assert!(
        !names.contains(&"deleted_at"),
        "deleted_at should be dropped"
    );
    assert!(names.contains(&"created_at"), "created_at retained");
    assert!(names.contains(&"updated_at"), "updated_at retained");
}

#[tokio::test]
async fn ssh_command_history_reserves_execution_metadata_and_scope_indexes() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('ssh_command_history')")
            .fetch_all(db.pool())
            .await
            .expect("list ssh command history columns");
    let names = columns
        .iter()
        .map(|(name,)| name.as_str())
        .collect::<Vec<_>>();
    for required in [
        "id",
        "workspace_id",
        "connection_id",
        "session_id",
        "command",
        "cwd",
        "exit_code",
        "duration_ms",
        "redacted",
        "executed_at",
    ] {
        assert!(names.contains(&required), "missing {required}");
    }

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'ssh_command_history'",
    )
    .fetch_all(db.pool())
    .await
    .expect("list ssh command history indexes");
    assert!(indexes
        .iter()
        .any(|(name,)| name == "idx_ssh_command_history_workspace_executed"));
    assert!(indexes
        .iter()
        .any(|(name,)| name == "idx_ssh_command_history_connection_executed"));
}

#[tokio::test]
async fn initial_schema_creates_single_active_environment_index() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO workspaces (id, name, is_default, created_at, updated_at, revision, sync_status)
        VALUES ('ws-active', 'Active', 0, ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert workspace");

    sqlx::query(
        r#"
        INSERT INTO api_environments (id, workspace_id, name, is_active, created_at, updated_at, revision, sync_status)
        VALUES ('env-1', 'ws-active', 'Env 1', 1, ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert first active env");

    // A second active environment in the same workspace must be rejected.
    let err = sqlx::query(
        r#"
        INSERT INTO api_environments (id, workspace_id, name, is_active, created_at, updated_at, revision, sync_status)
        VALUES ('env-2', 'ws-active', 'Env 2', 1, ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect_err("second active env should violate unique index");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("unique") || msg.contains("constraint"),
        "expected unique constraint violation, got: {msg}"
    );
}

#[tokio::test]
async fn initial_schema_includes_api_collection_folder_sync_fields() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('api_collection_folders')")
            .fetch_all(db.pool())
            .await
            .expect("list columns");
    let names: Vec<&str> = columns.iter().map(|(name,)| name.as_str()).collect();

    assert!(names.contains(&"revision"), "revision added");
    assert!(names.contains(&"sync_status"), "sync_status added");
    assert!(names.contains(&"remote_id"), "remote_id added");
}
