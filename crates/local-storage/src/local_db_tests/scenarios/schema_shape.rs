use super::*;

#[tokio::test]
async fn initial_schema_includes_saved_sql_soft_delete_fields() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('saved_sql')")
        .fetch_all(db.pool())
        .await
        .expect("list columns");
    let names: Vec<&str> = columns.iter().map(|(name,)| name.as_str()).collect();
    assert!(names.contains(&"deleted_at"), "deleted_at added");
    assert!(names.contains(&"revision"), "revision added");
    assert!(names.contains(&"sync_status"), "sync_status added");
    assert!(names.contains(&"remote_id"), "remote_id added");
}

#[tokio::test]
async fn initial_schema_omits_legacy_folder_columns() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let req_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('api_requests')")
            .fetch_all(db.pool())
            .await
            .expect("list api_requests columns");
    let req_names: Vec<&str> = req_cols.iter().map(|(n,)| n.as_str()).collect();
    assert!(
        !req_names.contains(&"folder_path"),
        "folder_path should be dropped from api_requests"
    );

    let coll_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('api_collections')")
            .fetch_all(db.pool())
            .await
            .expect("list api_collections columns");
    let coll_names: Vec<&str> = coll_cols.iter().map(|(n,)| n.as_str()).collect();
    assert!(
        !coll_names.contains(&"folders_json"),
        "folders_json should be dropped from api_collections"
    );
}

#[tokio::test]
async fn migrate_is_idempotent() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");
    db.migrate()
        .await
        .expect("second migration should succeed without error");
    db.migrate()
        .await
        .expect("third migration should succeed without error");
}

#[tokio::test]
async fn initial_schema_has_api_request_collection_tree_columns() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('api_requests')")
            .fetch_all(db.pool())
            .await
            .expect("list columns");
    assert!(
        columns.iter().any(|(name,)| name == "parent_folder_id"),
        "api_requests should have parent_folder_id column"
    );
    assert!(
        columns.iter().any(|(name,)| name == "collection_id"),
        "api_requests should have collection_id column"
    );
    assert!(
        columns.iter().any(|(name,)| name == "sort_order"),
        "api_requests should have sort_order column"
    );
}

#[tokio::test]
async fn initial_schema_has_api_collection_folders_table() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('api_collection_folders')")
            .fetch_all(db.pool())
            .await
            .expect("list columns");
    let names: Vec<&str> = columns.iter().map(|(name,)| name.as_str()).collect();

    assert!(
        [
            "id",
            "workspace_id",
            "collection_id",
            "parent_folder_id",
            "name",
            "sort_order",
            "created_at",
            "updated_at",
            "deleted_at",
        ]
        .iter()
        .all(|expected| names.contains(expected)),
        "api_collection_folders should have stable folder tree columns"
    );
}

#[tokio::test]
async fn initial_schema_workspace_policy_defaults_are_valid() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");
    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, created_at, updated_at, revision, sync_status
        )
        VALUES ('default-ws', 'Default', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 1, 'local')
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert workspace with defaults");

    let row: (String, String) = sqlx::query_as(
        "SELECT environment_type, mcp_policy FROM workspaces WHERE id = 'default-ws'",
    )
    .fetch_one(db.pool())
    .await
    .expect("read workspace policy fields");
    assert_eq!(row.0, "dev");
    assert_eq!(row.1, "auto");
}

#[tokio::test]
async fn ssh_tasks_schema_separates_syncable_templates_from_local_state() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'ssh_task%' ORDER BY name",
    )
    .fetch_all(db.pool())
    .await
    .expect("list SSH task tables");
    assert_eq!(
        tables,
        vec![
            ("ssh_task".to_string(),),
            ("ssh_task_local_binding".to_string(),),
            ("ssh_task_run".to_string(),),
            ("ssh_task_step".to_string(),),
        ]
    );

    for table in ["ssh_task", "ssh_task_step", "ssh_task_run"] {
        let query = format!("SELECT name FROM pragma_table_info('{table}')");
        let columns: Vec<(String,)> = sqlx::query_as(&query)
            .fetch_all(db.pool())
            .await
            .expect("list SSH task columns");
        assert!(
            columns.iter().any(|(name,)| name == "workspace_id"),
            "{table} must be workspace scoped"
        );
        let id: (String, i64) = sqlx::query_as(&format!(
            "SELECT type, pk FROM pragma_table_info('{table}') WHERE name = 'id'"
        ))
        .fetch_one(db.pool())
        .await
        .expect("read stable id column");
        assert_eq!(id, ("TEXT".to_string(), 1), "{table} uses a stable text id");
    }

    let task_columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('ssh_task')")
            .fetch_all(db.pool())
            .await
            .expect("list SSH task columns");
    let task_names = task_columns
        .iter()
        .map(|(name,)| name.as_str())
        .collect::<Vec<_>>();
    assert!(task_names.contains(&"deleted_at"));
    assert!(!task_names.contains(&"default_connection_id"));

    let step_columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('ssh_task_step')")
            .fetch_all(db.pool())
            .await
            .expect("list SSH task step columns");
    let step_names = step_columns
        .iter()
        .map(|(name,)| name.as_str())
        .collect::<Vec<_>>();
    assert!(step_names.contains(&"config_version"));
    assert!(step_names.contains(&"deleted_at"));

    let binding_columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('ssh_task_local_binding')")
            .fetch_all(db.pool())
            .await
            .expect("list SSH task local binding columns");
    let binding_names = binding_columns
        .iter()
        .map(|(name,)| name.as_str())
        .collect::<Vec<_>>();
    assert!(binding_names.contains(&"default_connection_id"));
    assert!(binding_names.contains(&"last_used_connection_id"));

    let run_columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('ssh_task_run')")
            .fetch_all(db.pool())
            .await
            .expect("list SSH task run columns");
    let run_names = run_columns
        .iter()
        .map(|(name,)| name.as_str())
        .collect::<Vec<_>>();
    for transient in ["inputs", "stdout", "stderr", "transfer_progress"] {
        assert!(
            !run_names.contains(&transient),
            "transient field {transient} must not be persisted"
        );
    }
}
