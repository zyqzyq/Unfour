use super::*;

#[tokio::test]
async fn migrate_creates_all_tables() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let tables: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(db.pool())
            .await
            .expect("list tables");
    let names: Vec<&str> = tables.iter().map(|(n,)| n.as_str()).collect();
    assert!(names.contains(&"workspaces"));
    assert!(names.contains(&"api_requests"));
    assert!(names.contains(&"api_history"));
    assert!(names.contains(&"db_query_history"));
    assert!(names.contains(&"saved_sql"));
    assert!(names.contains(&"connections"));
    assert!(names.contains(&"activity_events"));
    assert!(names.contains(&"app_settings"));
    assert!(names.contains(&"workspace_settings"));
    assert!(names.contains(&"ssh_terminal_history"));
    assert!(names.contains(&"api_collections"));
    assert!(names.contains(&"api_collection_folders"));
    assert!(names.contains(&"ssh_connections"));
    assert!(names.contains(&"database_connections"));
}

#[tokio::test]
async fn migrate_records_timestamped_core_migrations() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let versions: Vec<(i64,)> =
        sqlx::query_as("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(db.pool())
            .await
            .expect("list migration versions");

    assert_eq!(
        versions,
        vec![
            (CORE_INITIAL_MIGRATION_VERSION,),
            (CORE_FOLDER_CYCLES_MIGRATION_VERSION,),
            (CORE_CONSTRAINT_HARDENING_MIGRATION_VERSION,),
        ],
        "only timestamped core migrations should run"
    );
}

#[tokio::test]
async fn migrate_ignores_legacy_local_numbered_records() {
    let db = test_db().await;
    create_migration_table(db.pool()).await;
    record_migration(db.pool(), 1, "initial schema").await;
    record_migration(db.pool(), 2, "prevent folder cycles").await;

    db.migrate()
        .await
        .expect("run migrations with legacy local-number records");
    db.migrate()
        .await
        .expect("rerun migrations with legacy local-number records");

    let versions: Vec<(i64,)> =
        sqlx::query_as("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(db.pool())
            .await
            .expect("list migration versions");

    assert_eq!(
        versions,
        vec![
            (1,),
            (2,),
            (CORE_INITIAL_MIGRATION_VERSION,),
            (CORE_FOLDER_CYCLES_MIGRATION_VERSION,),
            (CORE_CONSTRAINT_HARDENING_MIGRATION_VERSION,),
        ],
        "legacy pre-timestamp records should be ignored, not removed"
    );
}

#[tokio::test]
async fn migrate_ignores_foreign_pro_migration_records() {
    const PRO_MIGRATION_VERSION: i64 = 20260707130000;

    let db = test_db().await;
    db.migrate().await.expect("run core migrations");
    sqlx::query(
        r#"
        CREATE TABLE pro_sync_mappings (
          id TEXT PRIMARY KEY,
          local_entity_type TEXT NOT NULL,
          local_entity_id TEXT NOT NULL,
          remote_entity_id TEXT NOT NULL,
          updated_at TEXT NOT NULL
        )
        "#,
    )
    .execute(db.pool())
    .await
    .expect("create pro table");
    record_migration(db.pool(), PRO_MIGRATION_VERSION, "pro initial schema").await;

    db.migrate()
        .await
        .expect("base should ignore foreign pro migration records");

    assert!(table_exists(db.pool(), "workspaces").await);
    assert!(
        table_exists(db.pool(), "pro_sync_mappings").await,
        "base migration must not delete pro-owned tables"
    );
}

async fn create_migration_table(pool: &sqlx::SqlitePool) {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
          version BIGINT PRIMARY KEY,
          description TEXT NOT NULL,
          installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
          success BOOLEAN NOT NULL,
          checksum BLOB NOT NULL,
          execution_time BIGINT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .expect("create sqlx migrations table");
}

async fn record_migration(pool: &sqlx::SqlitePool, version: i64, description: &str) {
    create_migration_table(pool).await;
    sqlx::query(
        r#"
        INSERT INTO _sqlx_migrations (
          version, description, success, checksum, execution_time
        )
        VALUES (?1, ?2, TRUE, ?3, 0)
        "#,
    )
    .bind(version)
    .bind(description)
    .bind(vec![0_u8])
    .execute(pool)
    .await
    .expect("record migration");
}

async fn table_exists(pool: &sqlx::SqlitePool, table_name: &str) -> bool {
    let exists: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .bind(table_name)
            .fetch_optional(pool)
            .await
            .expect("check table");
    exists.is_some()
}
