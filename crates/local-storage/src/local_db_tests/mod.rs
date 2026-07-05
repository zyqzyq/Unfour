use super::*;
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

async fn test_db() -> LocalDb {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory sqlite");
    LocalDb::from_pool(pool)
}

fn temp_db_path() -> PathBuf {
    let unique = format!(
        "unfour-local-storage-test-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

async fn seed_workspace(db: &LocalDb, workspace_id: &str) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, environment_type, mcp_policy,
          created_at, updated_at, revision, sync_status
        )
        VALUES (?1, ?1, 0, 'dev', 'auto', ?2, ?2, 1, 'local')
        "#,
    )
    .bind(workspace_id)
    .bind(now)
    .execute(db.pool())
    .await
    .expect("seed workspace");
}

#[path = "scenarios/connection_paths.rs"]
mod connection_paths;
#[path = "scenarios/connection_subtypes.rs"]
mod connection_subtypes;
#[path = "scenarios/history_and_indexes.rs"]
mod history_and_indexes;
#[path = "scenarios/migrations.rs"]
mod migrations;
#[path = "scenarios/schema_shape.rs"]
mod schema_shape;
#[path = "scenarios/workspace_constraints.rs"]
mod workspace_constraints;
