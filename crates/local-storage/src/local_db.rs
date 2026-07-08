use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use unfour_core::AppResult;

/// How long a connection waits for a held lock before returning
/// `SQLITE_BUSY`. The desktop app and satellite processes (e.g. the MCP server)
/// can open the same database file concurrently, so a non-zero busy timeout
/// avoids spurious "database is locked" failures.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
const CORE_INITIAL_MIGRATION_VERSION: i64 = 20260707110000;
#[cfg(test)]
const CORE_FOLDER_CYCLES_MIGRATION_VERSION: i64 = 20260707123000;
#[cfg(test)]
const CORE_CONSTRAINT_HARDENING_MIGRATION_VERSION: i64 = 20260708113000;

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
        let started = Instant::now();
        let safe_path = unfour_diag::safe_path_display(db_path);
        unfour_diag::log_operation_event(
            "sqlite_init_started",
            "local_storage",
            "connect_path",
            "started",
            None,
            None,
            serde_json::json!({ "database_path": safe_path }),
        );
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
            .await;

        match pool {
            Ok(pool) => {
                unfour_diag::log_operation_event(
                    "sqlite_init_completed",
                    "local_storage",
                    "connect_path",
                    "ok",
                    Some(started.elapsed().as_millis()),
                    None,
                    serde_json::json!({ "database_path": safe_path }),
                );
                Ok(Self { pool })
            }
            Err(error) => {
                unfour_diag::log_operation_event(
                    "sqlite_init_failed",
                    "local_storage",
                    "connect_path",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some("DATABASE_ERROR"),
                    serde_json::json!({ "database_path": safe_path }),
                );
                Err(error.into())
            }
        }
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
        let started = Instant::now();
        unfour_diag::log_operation_event(
            "migration_started",
            "local_storage",
            "migrate",
            "started",
            None,
            None,
            serde_json::json!({}),
        );
        let result = core_migrator()
            .run(&self.pool)
            .await
            .map_err(sqlx::Error::from);

        match result {
            Ok(()) => {
                unfour_diag::log_operation_event(
                    "migration_completed",
                    "local_storage",
                    "migrate",
                    "ok",
                    Some(started.elapsed().as_millis()),
                    None,
                    serde_json::json!({}),
                );
                Ok(())
            }
            Err(error) => {
                unfour_diag::log_operation_event(
                    "migration_failed",
                    "local_storage",
                    "migrate",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some("DATABASE_ERROR"),
                    serde_json::json!({}),
                );
                Err(error.into())
            }
        }
    }
}

fn core_migrator() -> Migrator {
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator
}

#[cfg(test)]
#[path = "local_db_tests/mod.rs"]
mod local_db_tests;
