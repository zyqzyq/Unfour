use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use unfour_core::AppResult;

/// How long a connection waits for a held lock before returning
/// `SQLITE_BUSY`. The desktop app and satellite processes (e.g. the MCP server)
/// can open the same database file concurrently, so a non-zero busy timeout
/// avoids spurious "database is locked" failures.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct LocalDb {
    pool: SqlitePool,
}

impl LocalDb {
    /// Connect using Unfour's stable product data directory.
    ///
    /// Path resolution deliberately lives in `unfour-paths`, not in Tauri path
    /// APIs, so the desktop app and satellite processes share the same SQLite
    /// file regardless of the bundle identifier.
    pub async fn connect_default() -> AppResult<Self> {
        let paths = unfour_paths::initialize_unfour_storage()?;
        Self::connect_path(paths.database_path).await
    }

    pub fn default_database_path() -> AppResult<PathBuf> {
        Ok(unfour_paths::default_database_path()?)
    }

    pub async fn connect_existing_default_read_only() -> AppResult<Self> {
        let paths = unfour_paths::initialize_unfour_storage()?;
        Self::connect_existing_read_only_path(paths.database_path).await
    }

    pub async fn connect_existing_default() -> AppResult<Self> {
        let paths = unfour_paths::initialize_unfour_storage()?;
        Self::connect_existing_path(paths.database_path).await
    }

    pub async fn connect_path(path: impl AsRef<Path>) -> AppResult<Self> {
        let db_path = path.as_ref();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .busy_timeout(BUSY_TIMEOUT)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    pub async fn connect_existing_read_only_path(path: impl AsRef<Path>) -> AppResult<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(false)
            .read_only(true)
            .busy_timeout(BUSY_TIMEOUT)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    pub async fn connect_existing_path(path: impl AsRef<Path>) -> AppResult<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(false)
            .busy_timeout(BUSY_TIMEOUT)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> AppResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(sqlx::Error::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
              id, workspace_id, name, created_at, updated_at, revision, sync_status
            )
            VALUES ('conn-1', 'ws-sql', 'DB', ?1, ?1, 1, 'local')
            "#,
        )
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("insert connection");
        sqlx::query(
            r#"INSERT INTO database_connections (connection_id, config_json) VALUES ('conn-1', '{}')"#,
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

    #[tokio::test]
    async fn initial_schema_uses_connection_subtypes() {
        let db = test_db().await;
        db.migrate().await.expect("run migrations");

        // Parent must no longer carry kind / config_json.
        let columns: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('connections')")
                .fetch_all(db.pool())
                .await
                .expect("list columns");
        let names: Vec<&str> = columns.iter().map(|(name,)| name.as_str()).collect();
        assert!(!names.contains(&"kind"), "kind dropped from connections");
        assert!(
            !names.contains(&"config_json"),
            "config_json dropped from connections"
        );
        assert!(names.contains(&"credential_ref"), "credential_ref retained");

        // Subtype tables exist with the expected shape.
        let ssh_cols: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('ssh_connections')")
                .fetch_all(db.pool())
                .await
                .expect("list ssh_connections columns");
        let ssh_names: Vec<&str> = ssh_cols.iter().map(|(n,)| n.as_str()).collect();
        assert!(ssh_names.contains(&"connection_id"));
        assert!(ssh_names.contains(&"config_json"));

        let db_cols: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('database_connections')")
                .fetch_all(db.pool())
                .await
                .expect("list database_connections columns");
        let db_names: Vec<&str> = db_cols.iter().map(|(n,)| n.as_str()).collect();
        assert!(db_names.contains(&"connection_id"));
        assert!(db_names.contains(&"config_json"));
    }

    #[tokio::test]
    async fn initial_schema_reads_connection_subtypes() {
        // Exercises the post-split shape end-to-end: insert a parent row plus
        // a subtype row, then read both back.
        let db = test_db().await;
        db.migrate().await.expect("run migrations");

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO workspaces (id, name, is_default, created_at, updated_at, revision, sync_status)
            VALUES ('ws-conn', 'Conn', 0, ?1, ?1, 1, 'local')
            "#,
        )
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("insert workspace");

        sqlx::query(
            r#"
            INSERT INTO connections (id, workspace_id, name, credential_ref, created_at, updated_at, revision, sync_status)
            VALUES ('c-ssh-2', 'ws-conn', 'ssh-2', NULL, ?1, ?1, 1, 'local')
            "#,
        )
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("insert new-style parent row");

        sqlx::query(
            r#"INSERT INTO ssh_connections (connection_id, config_json) VALUES ('c-ssh-2', '{"host":"h2"}')"#,
        )
        .execute(db.pool())
        .await
        .expect("insert ssh subtype row");

        let row: (String,) = sqlx::query_as(
            "SELECT config_json FROM ssh_connections WHERE connection_id = 'c-ssh-2'",
        )
        .fetch_one(db.pool())
        .await
        .expect("read subtype row");
        assert!(row.0.contains("h2"));
    }

    #[tokio::test]
    async fn initial_schema_includes_saved_sql_soft_delete_fields() {
        let db = test_db().await;
        db.migrate().await.expect("run migrations");

        let columns: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('saved_sql')")
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
}
