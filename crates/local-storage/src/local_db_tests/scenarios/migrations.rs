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
    assert!(names.contains(&"workspace_variables"));
    assert!(names.contains(&"workspace_environments"));
    assert!(names.contains(&"workspace_environment_variables"));
    assert!(names.contains(&"workspace_local_state"));
}

#[tokio::test]
async fn workspace_variable_migration_preserves_legacy_api_environments() {
    let db = test_db().await;
    sqlx::raw_sql(include_str!(
        "../../../migrations/20260708221117_core_initial_schema.sql"
    ))
    .execute(db.pool())
    .await
    .expect("apply legacy base schema");
    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, environment_type, mcp_policy,
          created_at, updated_at, revision, sync_status
        ) VALUES ('workspace-a', 'Workspace A', 1, 'dev', 'auto', 'now', 'now', 1, 'local')
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert workspace");
    sqlx::query(
        r#"
        INSERT INTO workspace_settings (
          workspace_id, layout_json, created_at, updated_at, revision, sync_status
        ) VALUES ('workspace-a', '{}', 'now', 'now', 1, 'local')
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert workspace settings");
    sqlx::query(
        r#"
        INSERT INTO api_environments (
          id, workspace_id, name, variables_json, is_active,
          created_at, updated_at, revision, sync_status
        ) VALUES (
          'environment-a', 'workspace-a', 'Development',
          '[{"key":"BASE_URL","value":"disabled duplicate","enabled":false},{"key":"BASE_URL","value":"https://legacy.example","enabled":true},{"key":"","value":"unfinished legacy value","enabled":true}]',
          1, 'created', 'updated', 3, 'pending'
        )
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert legacy environment");

    sqlx::raw_sql(include_str!(
        "../../../migrations/20260722120000_core_workspace_variables.sql"
    ))
    .execute(db.pool())
    .await
    .expect("apply workspace variable migration");
    sqlx::raw_sql(include_str!(
        "../../../migrations/20260723091537_core_workspace_domain_cleanup.sql"
    ))
    .execute(db.pool())
    .await
    .expect("apply workspace domain cleanup migration");

    let environment: (String, String, String, i64) = sqlx::query_as(
        "SELECT id, workspace_id, name, revision FROM workspace_environments WHERE id = 'environment-a'",
    )
    .fetch_one(db.pool())
    .await
    .expect("migrated environment");
    assert_eq!(environment.0, "environment-a");
    assert_eq!(environment.1, "workspace-a");
    assert_eq!(environment.2, "Development");
    assert_eq!(environment.3, 3);

    let variable: (String, String, bool) = sqlx::query_as(
        "SELECT key, value, is_enabled FROM workspace_environment_variables WHERE environment_id = 'environment-a' AND key = 'BASE_URL'",
    )
    .fetch_one(db.pool())
    .await
    .expect("migrated variable");
    assert_eq!(variable.0, "BASE_URL");
    assert_eq!(variable.1, "https://legacy.example");
    assert!(variable.2);

    let migrated_values: Vec<(String, bool)> = sqlx::query_as(
        "SELECT value, is_enabled FROM workspace_environment_variables WHERE environment_id = 'environment-a' ORDER BY sort_order",
    )
    .fetch_all(db.pool())
    .await
    .expect("all legacy variable rows");
    assert_eq!(migrated_values.len(), 3);
    assert_eq!(
        migrated_values[0],
        ("disabled duplicate".to_string(), false)
    );
    assert_eq!(
        migrated_values[2],
        ("unfinished legacy value".to_string(), false)
    );

    let active: (Option<String>,) = sqlx::query_as(
        "SELECT active_environment_id FROM workspace_local_state WHERE workspace_id = 'workspace-a'",
    )
    .fetch_one(db.pool())
    .await
    .expect("migrated active environment");
    assert_eq!(active.0.as_deref(), Some("environment-a"));

    for table in [
        "workspace_variables",
        "workspace_environments",
        "workspace_environment_variables",
    ] {
        let columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&format!("PRAGMA table_info({table})"))
                .fetch_all(db.pool())
                .await
                .expect("read cleaned table columns");
        let names: Vec<&str> = columns.iter().map(|column| column.1.as_str()).collect();
        assert!(!names.contains(&"sync_status"));
        assert!(!names.contains(&"remote_id"));
    }
}

#[tokio::test]
async fn mainline_database_upgrades_ssh_task_domain_sync_columns() {
    let db = test_db().await;
    for sql in [
        include_str!("../../../migrations/20260708221117_core_initial_schema.sql"),
        include_str!("../../../migrations/20260721090000_core_ssh_tasks.sql"),
        include_str!("../../../migrations/20260722120000_core_workspace_variables.sql"),
        include_str!("../../../migrations/20260723091537_core_workspace_domain_cleanup.sql"),
        include_str!("../../../migrations/20260728120000_core_api_request_scripts.sql"),
        include_str!("../../../migrations/20260809000000_core_ssh_task_sort_order.sql"),
        include_str!("../../../migrations/20260813000000_core_ssh_command_history.sql"),
    ] {
        sqlx::raw_sql(sql)
            .execute(db.pool())
            .await
            .expect("apply current main schema");
    }

    sqlx::query(
        r#"
        INSERT INTO workspaces (
          id, name, is_default, environment_type, mcp_policy,
          created_at, updated_at, revision
        ) VALUES (
          'workspace-main', 'Main Workspace', 1, 'dev', 'auto',
          '2026-08-19T00:00:00Z', '2026-08-19T00:00:00Z', 1
        )
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert main-era workspace");
    sqlx::query(
        r#"
        INSERT INTO ssh_task (
          id, workspace_id, name, description, created_at, updated_at, sort_order
        ) VALUES (
          'task-main', 'workspace-main', 'Legacy task', '',
          '2026-08-19T00:00:00Z', '2026-08-19T00:00:00Z', 0
        )
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert main-era SSH task without revision");
    sqlx::query(
        r#"
        INSERT INTO ssh_task_step (
          id, workspace_id, task_id, name, step_type, position, enabled,
          config_version, config_json, created_at, updated_at
        ) VALUES (
          'step-main', 'workspace-main', 'task-main', 'Command 1', 'command',
          0, 1, 1, '{"command":"echo main"}',
          '2026-08-19T00:00:00Z', '2026-08-19T00:00:00Z'
        )
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert main-era SSH task step without revision");

    sqlx::raw_sql(include_str!(
        "../../../migrations/20260817000000_core_ssh_task_domain_sync.sql"
    ))
    .execute(db.pool())
    .await
    .expect("upgrade SSH task revision columns from current main");
    sqlx::raw_sql(include_str!(
        "../../../migrations/20260817000001_core_ssh_task_step_position_paging.sql"
    ))
    .execute(db.pool())
    .await
    .expect("drop SSH task step unique position index");

    let task_revision: i64 =
        sqlx::query_scalar("SELECT revision FROM ssh_task WHERE id = 'task-main'")
            .fetch_one(db.pool())
            .await
            .expect("read upgraded task revision");
    let step_revision: i64 =
        sqlx::query_scalar("SELECT revision FROM ssh_task_step WHERE id = 'step-main'")
            .fetch_one(db.pool())
            .await
            .expect("read upgraded step revision");
    assert_eq!(task_revision, 1);
    assert_eq!(step_revision, 1);
    let active_position_index_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'uq_ssh_task_step_active_position')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!active_position_index_exists);
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
