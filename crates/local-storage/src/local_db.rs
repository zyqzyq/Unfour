use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use unfour_core::{AppError, AppResult};

const DB_FILENAME: &str = "unfour-workspace.sqlite";

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
        for statement in MIGRATIONS {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        self.ensure_api_request_folder_path().await?;
        self.ensure_host_key_columns().await?;

        Ok(())
    }

    async fn ensure_api_request_folder_path(&self) -> AppResult<()> {
        let columns =
            sqlx::query_as::<_, (String,)>("SELECT name FROM pragma_table_info('api_requests')")
                .fetch_all(&self.pool)
                .await?;
        if !columns.iter().any(|(name,)| name == "folder_path") {
            sqlx::query("ALTER TABLE api_requests ADD COLUMN folder_path TEXT")
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }
}

fn app_data_path(identifier: &str) -> AppResult<PathBuf> {
    dirs::data_dir()
        .map(|dir| dir.join(identifier))
        .ok_or_else(|| AppError::Config("app data directory is not available".to_string()))
}

const MIGRATIONS: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS app_settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL,
      updated_at TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS workspaces (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      is_default INTEGER NOT NULL DEFAULT 0,
      last_opened_at TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      deleted_at TEXT,
      revision INTEGER NOT NULL DEFAULT 1,
      sync_status TEXT NOT NULL DEFAULT 'local',
      remote_id TEXT
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS workspace_settings (
      workspace_id TEXT PRIMARY KEY,
      layout_json TEXT NOT NULL DEFAULT '{}',
      env_json TEXT NOT NULL DEFAULT '{}',
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      deleted_at TEXT,
      revision INTEGER NOT NULL DEFAULT 1,
      sync_status TEXT NOT NULL DEFAULT 'local',
      remote_id TEXT,
      FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS api_requests (
      id TEXT PRIMARY KEY,
      workspace_id TEXT NOT NULL,
      name TEXT NOT NULL,
      method TEXT NOT NULL,
      url TEXT NOT NULL,
      headers_json TEXT NOT NULL DEFAULT '[]',
      query_json TEXT NOT NULL DEFAULT '[]',
      body TEXT,
      body_kind TEXT NOT NULL DEFAULT 'json',
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      deleted_at TEXT,
      revision INTEGER NOT NULL DEFAULT 1,
      sync_status TEXT NOT NULL DEFAULT 'local',
      remote_id TEXT,
      FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS api_history (
      id TEXT PRIMARY KEY,
      workspace_id TEXT NOT NULL,
      name TEXT,
      method TEXT NOT NULL,
      url TEXT NOT NULL,
      request_headers_json TEXT NOT NULL DEFAULT '[]',
      request_query_json TEXT NOT NULL DEFAULT '[]',
      request_body TEXT,
      status INTEGER,
      duration_ms INTEGER,
      response_headers_json TEXT NOT NULL DEFAULT '[]',
      response_body_preview TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      deleted_at TEXT,
      revision INTEGER NOT NULL DEFAULT 1,
      sync_status TEXT NOT NULL DEFAULT 'local',
      remote_id TEXT,
      FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS connections (
      id TEXT PRIMARY KEY,
      workspace_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      name TEXT NOT NULL,
      config_json TEXT NOT NULL DEFAULT '{}',
      credential_ref TEXT,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      deleted_at TEXT,
      revision INTEGER NOT NULL DEFAULT 1,
      sync_status TEXT NOT NULL DEFAULT 'local',
      remote_id TEXT,
      FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS activity_events (
      id TEXT PRIMARY KEY,
      workspace_id TEXT,
      action TEXT NOT NULL,
      target TEXT,
      details_json TEXT NOT NULL DEFAULT '{}',
      created_at TEXT NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_api_history_workspace_created ON api_history(workspace_id, created_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_connections_workspace_kind ON connections(workspace_id, kind)",
    r#"
    CREATE TABLE IF NOT EXISTS ssh_host_keys (
      host TEXT NOT NULL,
      port INTEGER NOT NULL,
      fingerprint TEXT NOT NULL,
      created_at TEXT NOT NULL,
      PRIMARY KEY (host, port)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS ssh_terminal_history (
      workspace_id TEXT NOT NULL,
      session_id TEXT NOT NULL,
      connection_id TEXT NOT NULL,
      status TEXT NOT NULL,
      reconnect_attempt INTEGER NOT NULL DEFAULT 0,
      auth_kind TEXT NOT NULL,
      host TEXT NOT NULL,
      username TEXT NOT NULL,
      cols INTEGER NOT NULL,
      rows INTEGER NOT NULL,
      content TEXT NOT NULL DEFAULT '',
      byte_len INTEGER NOT NULL DEFAULT 0,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY (workspace_id, session_id),
      FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_ssh_terminal_history_workspace_updated ON ssh_terminal_history(workspace_id, updated_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_ssh_terminal_history_connection ON ssh_terminal_history(workspace_id, connection_id)",
];

impl LocalDb {
    async fn ensure_host_key_columns(&self) -> AppResult<()> {
        let columns =
            sqlx::query_as::<_, (String,)>("SELECT name FROM pragma_table_info('ssh_host_keys')")
                .fetch_all(&self.pool)
                .await?;
        if !columns.iter().any(|(name,)| name == "key_type") {
            sqlx::query("ALTER TABLE ssh_host_keys ADD COLUMN key_type TEXT")
                .execute(&self.pool)
                .await?;
        }
        if !columns.iter().any(|(name,)| name == "public_key_data") {
            sqlx::query("ALTER TABLE ssh_host_keys ADD COLUMN public_key_data TEXT")
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
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
        assert!(names.contains(&"connections"));
        assert!(names.contains(&"activity_events"));
        assert!(names.contains(&"app_settings"));
        assert!(names.contains(&"workspace_settings"));
        assert!(names.contains(&"ssh_terminal_history"));
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
