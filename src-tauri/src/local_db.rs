use crate::app_error::AppResult;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

#[derive(Clone)]
pub struct LocalDb {
    pool: SqlitePool,
}

impl LocalDb {
    pub async fn connect(app: &AppHandle) -> AppResult<Self> {
        let app_data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&app_data_dir)?;

        let db_path = app_data_dir.join("unfour-workspace.sqlite");
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

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    #[cfg(test)]
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> AppResult<()> {
        for statement in MIGRATIONS {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        self.ensure_api_request_folder_path().await?;

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
];
