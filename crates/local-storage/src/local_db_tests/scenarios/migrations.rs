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
async fn migrate_records_single_squashed_migration() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let versions: Vec<(i64,)> =
        sqlx::query_as("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(db.pool())
            .await
            .expect("list migration versions");

    assert_eq!(
        versions,
        vec![(1,)],
        "only the squashed initial migration should run"
    );
}
