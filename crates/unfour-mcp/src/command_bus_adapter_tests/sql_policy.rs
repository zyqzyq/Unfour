use super::*;
use crate::response::error_json;
use crate::tools::ToolRegistry;
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Row;
use std::path::{Path, PathBuf};
use unfour_core::models::DatabaseConnectionInput;

pub(super) fn workspace(adapter: &LocalCommandBusAdapter, env: &str, policy: &str) -> String {
    adapter
        .run(adapter.bus.create_workspace_with_options(
            format!("sql-policy-{env}-{policy}-{}", unfour_core::id::new_id()),
            Some(env.into()),
            Some(policy.into()),
        ))
        .unwrap()
        .id
}

pub(super) struct SqliteFile {
    pub(super) path: PathBuf,
    journal_mode: String,
}

impl SqliteFile {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "unfour-mcp-sql-policy-{}.sqlite",
            unfour_core::id::new_id()
        ));
        let journal_mode = block_on(async {
            let pool = open_pool(&path).await;
            sqlx::query("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO items (name) VALUES ('kept')")
                .execute(&pool)
                .await
                .unwrap();
            let mode = sqlx::query("PRAGMA journal_mode")
                .fetch_one(&pool)
                .await
                .unwrap()
                .get::<String, _>(0);
            pool.close().await;
            mode
        });
        Self { path, journal_mode }
    }

    pub(super) fn snapshot(&self) -> (i64, Vec<String>, String) {
        let path = self.path.clone();
        block_on(async move {
            let pool = open_pool(&path).await;
            let count = sqlx::query("SELECT COUNT(*) FROM items")
                .fetch_one(&pool)
                .await
                .unwrap()
                .get::<i64, _>(0);
            let tables =
                sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                    .fetch_all(&pool)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|row| row.get::<String, _>(0))
                    .collect();
            let mode = sqlx::query("PRAGMA journal_mode")
                .fetch_one(&pool)
                .await
                .unwrap()
                .get::<String, _>(0);
            pool.close().await;
            (count, tables, mode)
        })
    }

    pub(super) fn names(&self) -> Vec<String> {
        block_on(async {
            let pool = open_pool(&self.path).await;
            let names = sqlx::query_scalar("SELECT name FROM items ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
            pool.close().await;
            names
        })
    }
}

impl Drop for SqliteFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-shm"));
    }
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

async fn open_pool(path: &Path) -> sqlx::SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .unwrap()
}

pub(super) fn connect(adapter: &LocalCommandBusAdapter, workspace_id: &str, path: &Path) -> String {
    adapter
        .save_db_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: workspace_id.into(),
            name: "sqlite".into(),
            driver: "sqlite".into(),
            host: None,
            port: None,
            database: None,
            username: None,
            ssl_mode: None,
            sqlite_path: Some(path.to_string_lossy().into_owned()),
            credential_ref: None,
            read_only: false,
        })
        .unwrap()
        .id
}

fn call(registry: &ToolRegistry, tool: &str, args: Value) -> Value {
    registry.call(tool, args).unwrap()
}

const WRAPPED_WRITES: &[&str] = &[
    "EXPLAIN ANALYZE DELETE FROM items",
    "WITH d AS (DELETE FROM items RETURNING id, name) SELECT * FROM d",
    "SELECT * INTO copied FROM items",
    "SELECT name INTO OUTFILE 'items.csv' FROM items",
    "PRAGMA writable_schema = ON",
    "PRAGMA journal_mode = WAL",
    "/* hide */ EXPLAIN ANALYZE INSERT INTO items (name) VALUES ('x')",
    "-- hide\nWITH c AS (SELECT 1) DELETE FROM items",
    "SELECT 1; DELETE FROM items",
    "EXPLAIN PRAGMA writable_schema = ON",
    "EXPLAIN CREATE TABLE extra (id INT)",
];

#[test]
fn production_read_only_workspace_does_not_apply_wrapped_sql() {
    let file = SqliteFile::new();
    let before = file.snapshot();
    assert_eq!(before.0, 1);
    assert_eq!(file.journal_mode, before.2);

    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let workspace_id = workspace(&adapter, "prod", "auto");
    let connection_id = connect(&adapter, &workspace_id, &file.path);
    let registry = ToolRegistry::with_command_bus(adapter.clone());

    let read = call(
        &registry,
        "unfour.db.query_readonly",
        json!({
            "workspaceId": workspace_id,
            "connectionId": connection_id,
            "sql": "SELECT name FROM items"
        }),
    );
    assert_eq!(read["isError"], false);
    assert_eq!(read["structuredContent"]["rows"][0][0], "kept");

    for sql in [
        "WITH c AS (SELECT name FROM items) SELECT * FROM c",
        "EXPLAIN SELECT name FROM items",
        "/* comment */ SELECT COUNT(*) FROM items",
        "PRAGMA table_info(items)",
    ] {
        let result = call(
            &registry,
            "unfour.db.query_readonly",
            json!({"workspaceId": workspace_id, "connectionId": connection_id, "sql": sql}),
        );
        assert_eq!(result["isError"], false, "{sql}");
    }

    for sql in WRAPPED_WRITES {
        let execute = call(
            &registry,
            "unfour.db.execute",
            json!({
                "workspaceId": workspace_id,
                "connectionId": connection_id,
                "sql": sql,
                "confirm": true,
                "confirmation_text": "UNKNOWN_SQL:not-a-real-token"
            }),
        );
        assert_eq!(execute["isError"], true, "{sql}");
        assert_eq!(
            error_json(&execute)["error"]["code"],
            "WORKSPACE_POLICY_BLOCKED",
            "{sql}"
        );
        for tool in ["unfour.db.query_readonly", "unfour.db.explain"] {
            let result = call(
                &registry,
                tool,
                json!({"workspaceId": workspace_id, "connectionId": connection_id, "sql": sql}),
            );
            assert_eq!(result["isError"], true, "{tool} {sql}");
            assert_eq!(
                error_json(&result)["error"]["code"],
                "READONLY_SQL_REJECTED",
                "{tool} {sql}"
            );
        }
    }

    adapter.shutdown();
    let after = file.snapshot();
    assert_eq!(after, before);
}

#[test]
fn guarded_workspace_confirms_wrapped_writes_and_keeps_ordinary_updates() {
    let file = SqliteFile::new();
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let workspace_id = workspace(&adapter, "test", "auto");
    let connection_id = connect(&adapter, &workspace_id, &file.path);
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    for sql in [
        "WITH d AS (DELETE FROM items RETURNING id, name) SELECT * FROM d",
        "PRAGMA journal_mode = WAL",
        "EXPLAIN ANALYZE DELETE FROM items",
    ] {
        let required = call(
            &registry,
            "unfour.db.execute",
            json!({"workspaceId": workspace_id, "connectionId": connection_id, "sql": sql}),
        );
        assert_eq!(
            error_json(&required)["error"]["code"],
            "CONFIRMATION_REQUIRED",
            "{sql}"
        );
    }
    adapter.shutdown();
    let unchanged = file.snapshot();
    assert_eq!(unchanged.0, 1);
    assert_eq!(unchanged.2, file.journal_mode);

    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let workspace_id = workspace(&adapter, "test", "auto");
    let connection_id = connect(&adapter, &workspace_id, &file.path);
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    let updated = call(
        &registry,
        "unfour.db.execute",
        json!({
            "workspaceId": workspace_id,
            "connectionId": connection_id,
            "sql": "UPDATE items SET name = 'changed' WHERE id = 1"
        }),
    );
    assert_eq!(updated["isError"], false, "{updated}");
    let read = call(
        &registry,
        "unfour.db.query_readonly",
        json!({
            "workspaceId": workspace_id,
            "connectionId": connection_id,
            "sql": "SELECT name FROM items"
        }),
    );
    assert_eq!(read["structuredContent"]["rows"][0][0], "changed");

    let pragma = "PRAGMA journal_mode = WAL";
    let required = call(
        &registry,
        "unfour.db.execute",
        json!({"workspaceId": workspace_id, "connectionId": connection_id, "sql": pragma}),
    );
    let token = error_json(&required)["confirmation_text"].clone();
    let confirmed = call(
        &registry,
        "unfour.db.execute",
        json!({
            "workspaceId": workspace_id,
            "connectionId": connection_id,
            "sql": pragma,
            "confirm": true,
            "confirmation_text": token
        }),
    );
    assert_eq!(confirmed["isError"], false, "{confirmed}");
    adapter.shutdown();
    let after = file.snapshot();
    assert_eq!(after.0, 1);
    assert_eq!(after.2, "wal");
    assert!(!after.1.iter().any(|name| name == "copied"));
}

#[test]
fn dev_full_access_still_runs_ordinary_inserts_without_confirmation() {
    let file = SqliteFile::new();
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let workspace_id = workspace(&adapter, "dev", "auto");
    let connection_id = connect(&adapter, &workspace_id, &file.path);
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    let inserted = call(
        &registry,
        "unfour.db.execute",
        json!({
            "workspaceId": workspace_id,
            "connectionId": connection_id,
            "sql": "INSERT INTO items (name) VALUES ('added')"
        }),
    );
    assert_eq!(inserted["isError"], false, "{inserted}");
    adapter.shutdown();
    assert_eq!(file.snapshot().0, 2);
}

#[test]
fn guarded_forged_where_never_writes_sqlite_before_confirmation() {
    for sql in [
        "DELETE FROM items /* WHERE id = 1 */",
        "DELETE FROM items -- WHERE id = 1",
        "UPDATE items SET name = 'where'",
        "UPDATE items SET name = (SELECT name || '-changed' FROM items WHERE id = 1)",
        "UPDATE items SET name = 'changed' RETURNING 'where'",
        "WITH c AS (SELECT id FROM items WHERE id = 1) DELETE FROM items",
        "WITH c AS (SELECT id FROM items WHERE id = 1) UPDATE items SET name = 'changed'",
    ] {
        let file = SqliteFile::new();
        let before = file.snapshot();
        let names = file.names();
        let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
        let workspace_id = workspace(&adapter, "test", "guarded");
        let connection_id = connect(&adapter, &workspace_id, &file.path);
        let registry = ToolRegistry::with_command_bus(adapter.clone());
        let mut args =
            json!({"workspaceId": workspace_id, "connectionId": connection_id, "sql": sql});
        let required = call(&registry, "unfour.db.execute", args.clone());
        // Check the real data even if the old implementation executed the SQL.
        assert_eq!(
            file.snapshot(),
            before,
            "unconfirmed SQL changed SQLite: {sql}"
        );
        assert_eq!(file.names(), names, "unconfirmed SQL changed values: {sql}");
        let error = error_json(&required);
        assert_eq!(
            error["error"]["code"], "CONFIRMATION_REQUIRED",
            "{sql}: {required}"
        );
        args["confirm"] = json!(true);
        args["confirmation_text"] = error["confirmation_text"].clone();
        let executed = call(&registry, "unfour.db.execute", args);
        assert_eq!(executed["isError"], false, "{sql}: {executed}");
        assert!(
            file.snapshot() != before || file.names() != names,
            "confirmed SQL must execute: {sql}"
        );
        adapter.shutdown();
    }
}

#[test]
fn forged_where_keeps_full_access_guarded_and_read_only_semantics() {
    for policy in ["full_access", "guarded", "read_only"] {
        let file = SqliteFile::new();
        let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
        let workspace_id = workspace(&adapter, "dev", policy);
        let connection_id = connect(&adapter, &workspace_id, &file.path);
        let registry = ToolRegistry::with_command_bus(adapter.clone());
        let result = call(
            &registry,
            "unfour.db.execute",
            json!({
                "workspaceId":workspace_id, "connectionId":connection_id,
                "sql":"UPDATE items SET name = 'where'"
            }),
        );
        match policy {
            "full_access" => {
                assert_eq!(result["isError"], false, "{result}");
                assert_eq!(file.names(), vec!["where"]);
            }
            "guarded" | "read_only" => {
                assert_eq!(
                    error_json(&result)["error"]["code"],
                    if policy == "guarded" {
                        "CONFIRMATION_REQUIRED"
                    } else {
                        "WORKSPACE_POLICY_BLOCKED"
                    }
                );
                assert_eq!(file.names(), vec!["kept"]);
            }
            _ => unreachable!(),
        }
        adapter.shutdown();
    }
}
