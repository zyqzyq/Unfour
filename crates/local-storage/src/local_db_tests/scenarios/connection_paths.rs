use super::*;

#[tokio::test]
async fn connect_existing_read_only_path_reads_existing_database_without_creating() {
    let path = temp_db_path();
    let db = LocalDb::connect_path(&path).await.expect("create db");
    db.migrate().await.expect("run migrations");
    drop(db);

    let read_only = LocalDb::connect_existing_read_only_path(&path)
        .await
        .expect("open read-only db");
    let tables: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(read_only.pool())
            .await
            .expect("list tables");

    assert!(tables.iter().any(|(name,)| name == "workspaces"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn default_database_path_comes_from_unfour_paths() {
    let expected = unfour_paths::resolve_unfour_paths()
        .expect("resolve paths")
        .database_path;

    assert_eq!(
        LocalDb::default_database_path().expect("default database path"),
        expected
    );
}
