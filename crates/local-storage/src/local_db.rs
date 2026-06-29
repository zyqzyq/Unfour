use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use unfour_core::{AppError, AppResult};

const DB_FILENAME: &str = "unfour.sqlite";

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
    pub async fn connect(app: &AppHandle) -> AppResult<Self> {
        let app_data_dir = app.path().app_data_dir()?;
        Self::connect_path(app_data_dir.join(DB_FILENAME)).await
    }

    pub async fn connect_app_data(identifier: &str) -> AppResult<Self> {
        Self::connect_path(app_data_path(identifier)?).await
    }

    pub async fn connect_existing_app_data_read_only(identifier: &str) -> AppResult<Self> {
        Self::connect_existing_read_only_path(app_data_path(identifier)?.join(DB_FILENAME)).await
    }

    pub async fn connect_existing_app_data(identifier: &str) -> AppResult<Self> {
        Self::connect_existing_path(app_data_path(identifier)?.join(DB_FILENAME)).await
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

fn app_data_path(identifier: &str) -> AppResult<PathBuf> {
    dirs::data_dir()
        .map(|dir| dir.join(identifier))
        .ok_or_else(|| AppError::Config("app data directory is not available".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[tokio::test]
    async fn migrate_creates_all_tables() {
        let db = test_db().await;
        db.migrate().await.expect("first migration");

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
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let db = test_db().await;
        db.migrate().await.expect("first migration");
        db.migrate()
            .await
            .expect("second migration should succeed without error");
        db.migrate()
            .await
            .expect("third migration should succeed without error");
    }

    #[tokio::test]
    async fn migrate_ensures_folder_path_column() {
        let db = test_db().await;
        db.migrate().await.expect("migration");

        let columns: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('api_requests')")
                .fetch_all(db.pool())
                .await
                .expect("list columns");
        assert!(
            columns.iter().any(|(name,)| name == "folder_path"),
            "api_requests should have folder_path column"
        );
        assert!(
            columns.iter().any(|(name,)| name == "collection_id"),
            "api_requests should have collection_id column"
        );
    }

    #[tokio::test]
    async fn migrate_ensures_collection_folders_json_column() {
        let db = test_db().await;
        db.migrate().await.expect("migration");

        let columns: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('api_collections')")
                .fetch_all(db.pool())
                .await
                .expect("list columns");
        assert!(
            columns.iter().any(|(name,)| name == "folders_json"),
            "api_collections should have folders_json column"
        );
    }

    #[tokio::test]
    async fn connect_existing_read_only_path_reads_existing_database_without_creating() {
        let path = temp_db_path();
        let db = LocalDb::connect_path(&path).await.expect("create db");
        db.migrate().await.expect("migrate db");
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
}
