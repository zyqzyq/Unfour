use super::super::*;
use super::support::{service_with_workspace, sqlite_fixture, sqlite_input};
use std::fs;
use uuid::Uuid;

#[tokio::test]
async fn query_history_is_workspace_scoped_ordered_limited_and_clearable() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");
    let other_workspace_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, last_opened_at, created_at, updated_at,
          revision, sync_status
        )
        VALUES (?1, 'Other Workspace', 0, ?2, ?2, ?2, 1, 'local')
        "#,
    )
    .bind(&other_workspace_id)
    .bind(&now)
    .execute(service.db.pool())
    .await
    .expect("insert other workspace");

    service
        .record_query_history(DbQueryHistoryRecordInput {
            id: "history-old".to_string(),
            workspace_id: workspace_id.clone(),
            connection_id: Some(connection.id.clone()),
            connection_name: "Local SQLite".to_string(),
            sql: "select 1".to_string(),
            status: "success".to_string(),
            classification: Some("read".to_string()),
            row_count: Some(1),
            affected_rows: Some(0),
            duration_ms: Some(3),
            error: None,
            executed_at: "2026-01-01T00:00:00Z".to_string(),
        })
        .await
        .expect("record old history");
    service
        .record_query_history(DbQueryHistoryRecordInput {
            id: "history-new".to_string(),
            workspace_id: workspace_id.clone(),
            connection_id: Some(connection.id.clone()),
            connection_name: "Local SQLite".to_string(),
            sql: "select 2".to_string(),
            status: "success".to_string(),
            classification: Some("read".to_string()),
            row_count: Some(2),
            affected_rows: Some(0),
            duration_ms: Some(5),
            error: None,
            executed_at: "2026-01-01T00:00:02Z".to_string(),
        })
        .await
        .expect("record new history");
    service
        .record_query_history(DbQueryHistoryRecordInput {
            id: "history-other".to_string(),
            workspace_id: other_workspace_id.clone(),
            connection_id: None,
            connection_name: "Other SQLite".to_string(),
            sql: "select other".to_string(),
            status: "failed".to_string(),
            classification: None,
            row_count: None,
            affected_rows: None,
            duration_ms: None,
            error: Some("syntax error".to_string()),
            executed_at: "2026-01-01T00:00:03Z".to_string(),
        })
        .await
        .expect("record other workspace history");

    let listed = service
        .list_query_history(workspace_id.clone(), Some(10))
        .await
        .expect("list history");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, "history-new");
    assert_eq!(listed[0].row_count, Some(2));
    assert_eq!(listed[1].id, "history-old");

    let limited = service
        .list_query_history(workspace_id.clone(), Some(1))
        .await
        .expect("list limited history");
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].id, "history-new");

    service
        .clear_query_history(workspace_id.clone())
        .await
        .expect("clear workspace history");
    let cleared = service
        .list_query_history(workspace_id, Some(10))
        .await
        .expect("list cleared history");
    assert!(cleared.is_empty());

    let other = service
        .list_query_history(other_workspace_id, Some(10))
        .await
        .expect("list other workspace history");
    assert_eq!(other.len(), 1);
    assert_eq!(other[0].id, "history-other");
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn saved_sql_crud_is_workspace_scoped_and_validated() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");

    let created = service
        .save_sql(SavedSqlInput {
            id: None,
            workspace_id: workspace_id.clone(),
            connection_id: Some(connection.id.clone()),
            name: "Recent users".to_string(),
            sql: "SELECT * FROM users".to_string(),
        })
        .await
        .expect("create saved sql");
    assert_eq!(created.name, "Recent users");
    assert_eq!(
        created.connection_id.as_deref(),
        Some(connection.id.as_str())
    );

    let updated = service
        .save_sql(SavedSqlInput {
            id: Some(created.id.clone()),
            workspace_id: workspace_id.clone(),
            connection_id: None,
            name: "Active users".to_string(),
            sql: "SELECT * FROM users WHERE active".to_string(),
        })
        .await
        .expect("update saved sql");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.name, "Active users");
    assert!(updated.connection_id.is_none());

    let listed = service
        .list_saved_sql(workspace_id.clone())
        .await
        .expect("list saved sql");
    assert_eq!(listed.len(), 1);

    // Blank name and blank SQL are rejected.
    assert!(matches!(
        service
            .save_sql(SavedSqlInput {
                id: None,
                workspace_id: workspace_id.clone(),
                connection_id: None,
                name: "   ".to_string(),
                sql: "SELECT 1".to_string(),
            })
            .await,
        Err(AppError::Validation(_))
    ));

    let remaining = service
        .delete_saved_sql(workspace_id.clone(), created.id.clone())
        .await
        .expect("delete saved sql");
    assert!(remaining.is_empty());

    assert!(matches!(
        service.delete_saved_sql(workspace_id, created.id).await,
        Err(AppError::NotFound(_))
    ));
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn connection_crud_is_workspace_scoped_and_soft_deletes() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;

    let created = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");
    assert_eq!(created.name, "Local fixture");
    assert_eq!(created.driver, "sqlite");
    assert!(created.credential_ref.is_none());

    let listed = service
        .list_connections(workspace_id.clone())
        .await
        .expect("list connections");
    assert_eq!(listed.len(), 1);

    let updated = service
        .save_connection(DatabaseConnectionInput {
            id: Some(created.id.clone()),
            name: "Renamed fixture".to_string(),
            ..sqlite_input(&workspace_id, &path)
        })
        .await
        .expect("update connection");
    assert_eq!(updated.name, "Renamed fixture");
    assert_eq!(updated.revision, created.revision + 1);

    let saved_sql = service
        .save_sql(SavedSqlInput {
            id: None,
            workspace_id: workspace_id.clone(),
            connection_id: Some(created.id.clone()),
            name: "Connection query".to_string(),
            sql: "SELECT 1".to_string(),
        })
        .await
        .expect("save connection-bound sql");
    assert_eq!(
        saved_sql.connection_id.as_deref(),
        Some(created.id.as_str())
    );

    let after_delete = service
        .delete_connection(workspace_id.clone(), created.id)
        .await
        .expect("delete connection");
    assert!(after_delete.is_empty());

    let listed = service
        .list_connections(workspace_id.clone())
        .await
        .expect("list after delete");
    assert!(listed.is_empty());

    let saved_after_delete = service
        .list_saved_sql(workspace_id)
        .await
        .expect("list saved sql after connection delete");
    assert_eq!(saved_after_delete.len(), 1);
    assert!(saved_after_delete[0].connection_id.is_none());
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn connection_storage_columns_and_advanced_config_round_trip() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let replacement_path = sqlite_fixture().await;
    let replacement_path_string = replacement_path.to_string_lossy().to_string();

    let created = service
        .save_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: workspace_id.clone(),
            name: "Structured PG".to_string(),
            driver: "postgres".to_string(),
            host: Some(" pg.internal ".to_string()),
            port: Some(5433),
            database: Some(" app ".to_string()),
            username: Some(" deploy ".to_string()),
            ssl_mode: Some("REQUIRE".to_string()),
            sqlite_path: None,
            credential_ref: Some("unfour:ws:database-password:abc".to_string()),
            read_only: true,
        })
        .await
        .expect("save postgres connection");

    assert_eq!(created.host.as_deref(), Some("pg.internal"));
    assert_eq!(created.database.as_deref(), Some("app"));
    assert_eq!(created.username.as_deref(), Some("deploy"));
    assert_eq!(created.ssl_mode.as_deref(), Some("require"));
    assert!(created.read_only);

    let stored: (
        String,
        Option<String>,
        Option<i64>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        bool,
        String,
        Option<String>,
    ) = sqlx::query_as(
        r#"
        SELECT
          c.connection_type, c.host, c.port, sub.driver, sub.database_name,
          sub.username, sub.ssl_mode, sub.read_only, sub.config_json, c.credential_ref
        FROM connections c
        INNER JOIN database_connections sub ON sub.connection_id = c.id
        WHERE c.id = ?1
        "#,
    )
    .bind(&created.id)
    .fetch_one(service.db.pool())
    .await
    .expect("load persisted database connection");
    assert_eq!(stored.0, "database");
    assert_eq!(stored.1.as_deref(), Some("pg.internal"));
    assert_eq!(stored.2, Some(5433));
    assert_eq!(stored.3, "postgres");
    assert_eq!(stored.4.as_deref(), Some("app"));
    assert_eq!(stored.5.as_deref(), Some("deploy"));
    assert_eq!(stored.6.as_deref(), Some("require"));
    assert!(stored.7);
    assert_eq!(stored.8, "{}");
    assert_eq!(stored.9.as_deref(), Some("unfour:ws:database-password:abc"));
    assert!(!stored.8.contains("database-password"));
    assert!(!stored.8.contains("deploy"));

    let updated = service
        .save_connection(DatabaseConnectionInput {
            id: Some(created.id.clone()),
            workspace_id: workspace_id.clone(),
            name: "Structured SQLite".to_string(),
            driver: "sqlite".to_string(),
            host: None,
            port: None,
            database: None,
            username: None,
            ssl_mode: None,
            sqlite_path: Some(replacement_path_string.clone()),
            credential_ref: None,
            read_only: false,
        })
        .await
        .expect("update to sqlite connection");
    assert_eq!(updated.driver, "sqlite");
    assert_eq!(
        updated.sqlite_path.as_deref(),
        Some(replacement_path_string.as_str())
    );
    assert!(updated.host.is_none());
    assert!(updated.credential_ref.is_none());

    let updated_stored: (Option<String>, Option<i64>, String, String, Option<String>) =
        sqlx::query_as(
            r#"
            SELECT c.host, c.port, sub.driver, sub.config_json, c.credential_ref
            FROM connections c
            INNER JOIN database_connections sub ON sub.connection_id = c.id
            WHERE c.id = ?1
            "#,
        )
        .bind(&created.id)
        .fetch_one(service.db.pool())
        .await
        .expect("load updated persisted database connection");
    assert!(updated_stored.0.is_none());
    assert!(updated_stored.1.is_none());
    assert_eq!(updated_stored.2, "sqlite");
    assert!(updated_stored.3.contains("sqlitePath"));
    assert!(updated_stored.4.is_none());

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(replacement_path);
}

#[tokio::test]
async fn connection_config_json_missing_optional_fields_uses_defaults() {
    let (service, workspace_id) = service_with_workspace().await;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO connections (
          id, workspace_id, connection_type, name, host, port,
          created_at, updated_at, revision, sync_status
        )
        VALUES ('manual-db', ?1, 'database', 'Manual DB', 'db.internal', 5432, ?2, ?2, 1, 'local')
        "#,
    )
    .bind(&workspace_id)
    .bind(&now)
    .execute(service.db.pool())
    .await
    .expect("insert manual parent row");
    sqlx::query(
        r#"
        INSERT INTO database_connections (
          connection_id, driver, database_name, username, ssl_mode, read_only, config_json
        )
        VALUES ('manual-db', 'postgres', 'app', 'deploy', NULL, 0, '{}')
        "#,
    )
    .execute(service.db.pool())
    .await
    .expect("insert manual database subtype row");

    let listed = service
        .list_connections(workspace_id)
        .await
        .expect("list manual connection");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "manual-db");
    assert_eq!(listed[0].driver, "postgres");
    assert_eq!(listed[0].database.as_deref(), Some("app"));
    assert!(listed[0].sqlite_path.is_none());
    assert!(!listed[0].read_only);
}
