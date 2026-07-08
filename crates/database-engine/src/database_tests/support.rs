use super::super::*;
use sqlx::Executor;
use std::path::PathBuf;
use uuid::Uuid;

pub(super) async fn service_with_workspace() -> (DatabaseService, String) {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory app db");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("run migrations");

    let workspace_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, last_opened_at, created_at, updated_at,
          revision, sync_status
        )
        VALUES (?1, 'Test Workspace', 1, ?2, ?2, ?2, 1, 'local')
        "#,
    )
    .bind(&workspace_id)
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert workspace");

    let secret_store = SecretStore::in_memory("unfour-test");
    let service = DatabaseService::new(db).with_secret_store(secret_store);

    (service, workspace_id)
}

pub(super) async fn sqlite_fixture() -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("unfour-database-service-{}.sqlite", Uuid::new_v4()));
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect fixture sqlite");

    pool.execute(
        r#"
        CREATE TABLE deploys (
          id INTEGER PRIMARY KEY,
          service TEXT NOT NULL,
          version TEXT NOT NULL
        )
        "#,
    )
    .await
    .expect("create deploys");
    pool.execute(
        r#"
        CREATE TABLE empty_deploys (
          id INTEGER PRIMARY KEY,
          service TEXT NOT NULL
        )
        "#,
    )
    .await
    .expect("create empty deploys");
    pool.execute(
        "INSERT INTO deploys (service, version) VALUES ('api', '1.0.0'), ('worker', '1.0.1')",
    )
    .await
    .expect("insert deploys");
    pool.close().await;

    path
}

pub(super) fn sqlite_input(workspace_id: &str, path: &PathBuf) -> DatabaseConnectionInput {
    DatabaseConnectionInput {
        id: None,
        workspace_id: workspace_id.to_string(),
        name: "Local fixture".to_string(),
        driver: "sqlite".to_string(),
        host: None,
        port: None,
        database: None,
        username: None,
        ssl_mode: None,
        sqlite_path: Some(path.to_string_lossy().to_string()),
        credential_ref: Some("  ".to_string()),
        read_only: false,
    }
}

pub(super) fn postgres_input(workspace_id: &str) -> DatabaseConnectionInput {
    DatabaseConnectionInput {
        id: None,
        workspace_id: workspace_id.to_string(),
        name: "PG test".to_string(),
        driver: "postgres".to_string(),
        host: Some("127.0.0.1".to_string()),
        port: Some(5432),
        database: Some("testdb".to_string()),
        username: Some("testuser".to_string()),
        ssl_mode: None,
        sqlite_path: None,
        credential_ref: None,
        read_only: false,
    }
}

pub(super) fn mysql_input(
    workspace_id: &str,
    credential_ref: Option<String>,
) -> DatabaseConnectionInput {
    DatabaseConnectionInput {
        id: None,
        workspace_id: workspace_id.to_string(),
        name: "MySQL test".to_string(),
        driver: "mysql".to_string(),
        host: Some("127.0.0.1".to_string()),
        port: Some(9),
        database: Some("app".to_string()),
        username: Some("testuser".to_string()),
        ssl_mode: None,
        sqlite_path: None,
        credential_ref,
        read_only: false,
    }
}
