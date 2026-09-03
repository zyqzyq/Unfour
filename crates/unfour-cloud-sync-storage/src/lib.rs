use sqlx::migrate::Migrator;
use sqlx::SqlitePool;
use unfour_core::AppResult;

/// Runs the merged local schema chain in compatibility order:
/// core history, historical Pro Cloud Sync migrations, then public renames.
pub async fn migrate(pool: &SqlitePool) -> AppResult<()> {
    let core = unfour_local_storage::LocalDb::from_pool(pool.clone());
    core.migrate().await?;
    cloud_sync_migrator()
        .run(pool)
        .await
        .map_err(sqlx::Error::from)?;
    Ok(())
}

fn cloud_sync_migrator() -> Migrator {
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    const HISTORICAL_PRO_MIGRATION_CUTOFF: i64 = 20260821120000;
    const CLOUD_SYNC_RENAME_MIGRATION_VERSION: i64 = 20260827010000;
    const HISTORICAL_PRO_MIGRATION_VERSIONS: [i64; 8] = [
        20260727120000,
        20260728100000,
        20260729120000,
        20260813010000,
        20260817020000,
        20260818010000,
        20260821055320,
        20260821120000,
    ];
    const PRO_TABLES: [&str; 8] = [
        "pro_workspace_sync_bindings",
        "pro_sync_runtime_context",
        "pro_sync_outbox",
        "pro_sync_entity_state",
        "pro_sync_attempts",
        "pro_sync_snapshot_staging",
        "pro_sync_diagnostics",
        "pro_sync_account_settings",
    ];
    const CLOUD_SYNC_TABLES: [&str; 9] = [
        "cloud_sync_workspace_bindings",
        "cloud_sync_runtime_context",
        "cloud_sync_outbox",
        "cloud_sync_entity_state",
        "cloud_sync_attempts",
        "cloud_sync_snapshot_staging",
        "cloud_sync_diagnostics",
        "cloud_sync_account_settings",
        "cloud_sync_account_binding_pause_reasons",
    ];

    async fn test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);

        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory sqlite")
    }

    #[tokio::test]
    async fn empty_database_migrates_to_cloud_sync_schema() {
        let pool = test_pool().await;
        migrate(&pool).await.expect("run merged migrations");

        assert!(table_exists(&pool, "workspaces").await);
        for table in CLOUD_SYNC_TABLES {
            assert!(table_exists(&pool, table).await, "missing {table}");
        }
        for table in PRO_TABLES {
            assert!(!table_exists(&pool, table).await, "stale {table}");
        }

        sqlx::query(
            "INSERT INTO cloud_sync_account_settings (account_id, updated_at) VALUES ('new-account', '2026-07-29T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert account setting with defaults");
        let enabled: bool = sqlx::query_scalar(
            "SELECT sync_enabled FROM cloud_sync_account_settings WHERE account_id = 'new-account'",
        )
        .fetch_one(&pool)
        .await
        .expect("read global sync default");
        assert!(!enabled, "global sync must default to off");

        for table in ["cloud_sync_outbox", "cloud_sync_entity_state"] {
            let schema: String = sqlx::query_scalar(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("read protocol v4 sync schema");
            assert!(
                schema.contains("'connection'"),
                "{table} rejects connection"
            );
            assert!(schema.contains("'sshTask'"), "{table} rejects sshTask");
            assert!(
                schema.contains("'sshTaskStep'"),
                "{table} rejects sshTaskStep"
            );
        }
        let bootstrap_default: String = sqlx::query_scalar(
            r#"SELECT dflt_value FROM pragma_table_info('cloud_sync_workspace_bindings')
               WHERE name = 'ssh_task_v3_bootstrap_state'"#,
        )
        .fetch_one(&pool)
        .await
        .expect("read SSH Task v3 bootstrap default");
        assert_eq!(bootstrap_default, "'pending'");
        let connection_bootstrap_default: String = sqlx::query_scalar(
            r#"SELECT dflt_value FROM pragma_table_info('cloud_sync_workspace_bindings')
               WHERE name = 'connection_v4_bootstrap_state'"#,
        )
        .fetch_one(&pool)
        .await
        .expect("read Connection v4 bootstrap default");
        assert_eq!(connection_bootstrap_default, "'pending'");
        let api_bootstrap_default: String = sqlx::query_scalar(
            r#"SELECT dflt_value FROM pragma_table_info('cloud_sync_workspace_bindings')
               WHERE name = 'api_v2_bootstrap_state'"#,
        )
        .fetch_one(&pool)
        .await
        .expect("read API v2 bootstrap default");
        assert_eq!(api_bootstrap_default, "'pending'");
    }

    #[tokio::test]
    async fn old_community_database_adds_cloud_sync_schema() {
        let pool = test_pool().await;
        let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
        db.migrate().await.expect("create old Community database");
        assert!(table_exists(&pool, "workspaces").await);
        for table in PRO_TABLES.into_iter().chain(CLOUD_SYNC_TABLES) {
            assert!(!table_exists(&pool, table).await, "unexpected {table}");
        }

        migrate(&pool).await.expect("upgrade Community database");

        let versions = migration_versions(&pool).await;
        for version in HISTORICAL_PRO_MIGRATION_VERSIONS {
            assert!(versions.contains(&version), "missing migration {version}");
        }
        assert!(versions.contains(&CLOUD_SYNC_RENAME_MIGRATION_VERSION));
        for table in CLOUD_SYNC_TABLES {
            assert!(table_exists(&pool, table).await, "missing {table}");
        }
    }

    #[tokio::test]
    async fn old_pro_database_renames_all_local_tables() {
        let pool = test_pool().await;
        let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
        db.migrate().await.expect("run core migrations");
        apply_historical_pro_migrations(&pool).await;

        for table in PRO_TABLES {
            assert!(
                table_exists(&pool, table).await,
                "missing historical {table}"
            );
        }
        for table in CLOUD_SYNC_TABLES {
            assert!(!table_exists(&pool, table).await, "premature {table}");
        }

        migrate(&pool).await.expect("upgrade old Pro database");

        for table in PRO_TABLES {
            assert!(!table_exists(&pool, table).await, "stale {table}");
        }
        for table in CLOUD_SYNC_TABLES {
            assert!(table_exists(&pool, table).await, "missing renamed {table}");
        }
        assert!(sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("check renamed foreign keys")
            .is_empty());
    }

    #[tokio::test]
    async fn old_pro_mock_data_survives_table_rename_exactly() {
        let pool = test_pool().await;
        let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
        db.migrate().await.expect("run core migrations");
        apply_historical_pro_migrations(&pool).await;

        sqlx::query(
            r#"INSERT INTO workspaces (
                 id, name, is_default, last_opened_at, environment_type, mcp_policy,
                 created_at, updated_at, revision
               ) VALUES ('workspace-preserved', 'Preserved', 0, NULL, 'dev', 'auto',
                         '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z', 23)"#,
        )
        .execute(&pool)
        .await
        .expect("insert workspace");
        sqlx::query(
            r#"INSERT INTO pro_sync_account_settings (account_id, sync_enabled, updated_at)
               VALUES ('account-preserved', 1, '2026-08-26T00:00:01Z')"#,
        )
        .execute(&pool)
        .await
        .expect("insert account setting");
        sqlx::query(
            r#"INSERT INTO pro_workspace_sync_bindings (
                 account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, state, initial_cursor, initial_total, initial_confirmed,
                 initialization_checkpoint, ssh_task_v3_bootstrap_state,
                 connection_v4_bootstrap_state, generation, last_success_at, last_error,
                 consecutive_failure_count, created_at, updated_at
               ) VALUES (
                 'account-preserved', 'workspace-preserved', 'cloud-preserved', 987654321,
                 1, 'active', 987654000, 4, 4, 'checkpoint-preserved', 'completed',
                 'pending', 7, '2026-08-26T00:00:02Z', NULL, 0,
                 '2026-08-26T00:00:00Z', '2026-08-26T00:00:02Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert workspace sync binding");
        sqlx::query(
            r#"INSERT INTO pro_sync_outbox (
                 account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
                 operation_id, parent_entity_id, operation, base_version,
                 payload_schema_version, canonical_payload_json, deleted_at, content_revision,
                 status, attempt_count, next_attempt_at, lease_owner, lease_started_at,
                 lease_expires_at, last_error, created_at, updated_at
               ) VALUES (
                 'account-preserved', 'workspace-preserved', 'cloud-preserved',
                 'workspaceVariable', 'entity-preserved', 'operation-preserved',
                 'workspace-preserved', 'upsert', 17, 1, '{"key":"PRESERVED"}', NULL, 18,
                 'pending', 2, '2026-08-26T00:00:04Z', NULL, NULL, NULL, 'retryable_transport',
                 '2026-08-26T00:00:03Z', '2026-08-26T00:00:03Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert outbox operation");
        sqlx::query(
            r#"INSERT INTO pro_sync_entity_state (
                 account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                 last_operation_id, sync_status, conflict_remote_payload_json,
                 conflict_remote_operation, conflict_parent_entity_id, conflict_deleted_at,
                 conflict_operation_id, updated_at
               ) VALUES (
                 'account-preserved', 'cloud-preserved', 'workspaceVariable',
                 'entity-state-preserved', 19, 'operation-state-preserved', 'synced',
                 NULL, NULL, NULL, NULL, NULL, '2026-08-26T00:00:05Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert entity state");

        let before_counts = [
            table_row_count(&pool, "pro_workspace_sync_bindings").await,
            table_row_count(&pool, "pro_sync_outbox").await,
            table_row_count(&pool, "pro_sync_entity_state").await,
            table_row_count(&pool, "pro_sync_account_settings").await,
        ];

        migrate(&pool).await.expect("rename populated Pro tables");

        let after_counts = [
            table_row_count(&pool, "cloud_sync_workspace_bindings").await,
            table_row_count(&pool, "cloud_sync_outbox").await,
            table_row_count(&pool, "cloud_sync_entity_state").await,
            table_row_count(&pool, "cloud_sync_account_settings").await,
        ];
        assert_eq!(after_counts, before_counts);
        assert_eq!(after_counts, [1, 1, 1, 1]);

        let binding: (String, String, String, i64, String, i64) = sqlx::query_as(
            r#"SELECT account_id, local_workspace_id, cloud_workspace_id,
                      last_pulled_cursor, state, generation
               FROM cloud_sync_workspace_bindings"#,
        )
        .fetch_one(&pool)
        .await
        .expect("read renamed binding");
        assert_eq!(
            binding,
            (
                "account-preserved".into(),
                "workspace-preserved".into(),
                "cloud-preserved".into(),
                987654321,
                "active".into(),
                7,
            )
        );
        let outbox: (String, String, i64, i64, i64, String, i64) = sqlx::query_as(
            r#"SELECT operation_id, entity_id, base_version, payload_schema_version,
                      content_revision, status, attempt_count
               FROM cloud_sync_outbox"#,
        )
        .fetch_one(&pool)
        .await
        .expect("read renamed outbox operation");
        assert_eq!(
            outbox,
            (
                "operation-preserved".into(),
                "entity-preserved".into(),
                17,
                1,
                18,
                "pending".into(),
                2,
            )
        );
        let state: (String, i64, Option<String>, String) = sqlx::query_as(
            r#"SELECT entity_id, server_version, last_operation_id, sync_status
               FROM cloud_sync_entity_state"#,
        )
        .fetch_one(&pool)
        .await
        .expect("read renamed entity state");
        assert_eq!(
            state,
            (
                "entity-state-preserved".into(),
                19,
                Some("operation-state-preserved".into()),
                "synced".into(),
            )
        );
        let account: (String, bool) =
            sqlx::query_as("SELECT account_id, sync_enabled FROM cloud_sync_account_settings")
                .fetch_one(&pool)
                .await
                .expect("read renamed account setting");
        assert_eq!(account, ("account-preserved".into(), true));
        assert!(sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("check preserved foreign keys")
            .is_empty());
    }

    #[tokio::test]
    async fn repeated_merged_migrate_is_idempotent() {
        let pool = test_pool().await;
        migrate(&pool).await.expect("first migrate");
        let versions_before = migration_versions(&pool).await;

        migrate(&pool).await.expect("second migrate");
        migrate(&pool).await.expect("third migrate");

        assert_eq!(migration_versions(&pool).await, versions_before);
        for table in CLOUD_SYNC_TABLES {
            assert!(table_exists(&pool, table).await, "missing {table}");
        }
    }

    #[tokio::test]
    async fn legacy_binding_is_upgraded_to_paused_unclaimed_without_auto_sync() {
        let pool = test_pool().await;
        let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
        db.migrate().await.expect("run core migrations");
        sqlx::raw_sql(include_str!(
            "../migrations/20260727120000_pro_workspace_cloud_sync_v1.sql"
        ))
        .execute(&pool)
        .await
        .expect("apply legacy schema");
        let workspace_id = "legacy-workspace".to_string();
        sqlx::query(
            r#"INSERT INTO workspaces (
                 id, name, is_default, last_opened_at, environment_type, mcp_policy,
                 created_at, updated_at, revision
               ) VALUES (?1, 'Legacy', 0, NULL, 'dev', 'auto',
                         '2026-07-28T00:00:00Z', '2026-07-28T00:00:00Z', 1)"#,
        )
        .bind(&workspace_id)
        .execute(&pool)
        .await
        .expect("legacy workspace");
        sqlx::query(
            r#"INSERT INTO pro_workspace_sync_bindings (
                 local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, created_at, updated_at
               ) VALUES (?1, 'legacy-cloud', '42', 1, '2026-07-28T00:00:00Z', '2026-07-28T00:00:00Z')"#,
        )
        .bind(workspace_id)
        .execute(&pool)
        .await
        .expect("legacy binding");
        sqlx::raw_sql(include_str!(
            "../migrations/20260728100000_pro_cloud_sync_recovery_and_accounts.sql"
        ))
        .execute(&pool)
        .await
        .expect("upgrade schema");
        let upgraded: (String, bool, String, i64, String) = sqlx::query_as(
            "SELECT account_id, sync_enabled, state, last_pulled_cursor, last_error FROM pro_workspace_sync_bindings",
        )
        .fetch_one(&pool)
        .await
        .expect("upgraded binding");
        assert_eq!(
            upgraded,
            (
                "unclaimed".into(),
                false,
                "paused".into(),
                42,
                "legacy_binding_unclaimed".into()
            )
        );
    }

    #[tokio::test]
    async fn protocol_v3_upgrade_marks_existing_bindings_for_ssh_task_bootstrap() {
        let pool = test_pool().await;
        let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
        db.migrate().await.expect("run core migrations");
        for migration in [
            include_str!("../migrations/20260727120000_pro_workspace_cloud_sync_v1.sql"),
            include_str!("../migrations/20260728100000_pro_cloud_sync_recovery_and_accounts.sql"),
            include_str!("../migrations/20260729120000_pro_global_sync_settings.sql"),
            include_str!("../migrations/20260813010000_pro_api_client_cloud_sync_entities.sql"),
            include_str!("../migrations/20260817020000_pro_ssh_task_cloud_sync_entities.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&pool)
                .await
                .expect("apply pre-bootstrap pro migration");
        }
        sqlx::query(
            r#"INSERT INTO workspaces (
                 id, name, is_default, last_opened_at, environment_type, mcp_policy,
                 created_at, updated_at, revision
               ) VALUES ('v2-workspace', 'V2', 0, NULL, 'dev', 'auto',
                         '2026-08-18T00:00:00Z', '2026-08-18T00:00:00Z', 1)"#,
        )
        .execute(&pool)
        .await
        .expect("insert v2 workspace");
        sqlx::query(
            r#"INSERT INTO pro_workspace_sync_bindings (
                 account_id, local_workspace_id, cloud_workspace_id,
                 last_pulled_cursor, sync_enabled, state, initial_total,
                 initial_confirmed, generation, created_at, updated_at
               ) VALUES ('account-a', 'v2-workspace', 'v2-cloud', 7, 1,
                         'active', 1, 1, 3, '2026-08-18T00:00:00Z',
                         '2026-08-18T00:00:00Z')"#,
        )
        .execute(&pool)
        .await
        .expect("insert pre-bootstrap binding");

        sqlx::raw_sql(include_str!(
            "../migrations/20260818010000_pro_ssh_task_v3_bootstrap_state.sql"
        ))
        .execute(&pool)
        .await
        .expect("apply SSH Task bootstrap migration");

        let state: String = sqlx::query_scalar(
            "SELECT ssh_task_v3_bootstrap_state FROM pro_workspace_sync_bindings",
        )
        .fetch_one(&pool)
        .await
        .expect("read upgraded binding bootstrap state");
        assert_eq!(state, "pending");
    }

    #[tokio::test]
    async fn protocol_v3_to_v4_migration_preserves_real_sync_state() {
        let pool = test_pool().await;
        let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
        db.migrate().await.expect("run core migrations");

        let v3_cutoff = 20260818010000_i64;
        for migration in cloud_sync_migrator()
            .iter()
            .filter(|migration| migration.version <= v3_cutoff)
        {
            sqlx::raw_sql(migration.sql.as_ref())
                .execute(&pool)
                .await
                .expect("apply v3 migration");
            record_migration_with_checksum(
                &pool,
                migration.version,
                migration.description.as_ref(),
                migration.checksum.as_ref(),
            )
            .await;
        }

        sqlx::query(
            r#"INSERT INTO workspaces (
                 id, name, is_default, last_opened_at, environment_type, mcp_policy,
                 created_at, updated_at, revision
               ) VALUES ('v3-retained-workspace', 'V3 retained', 0, NULL, 'dev', 'auto',
                         '2026-08-20T00:00:00Z', '2026-08-20T00:00:00Z', 3)"#,
        )
        .execute(&pool)
        .await
        .expect("insert v3 workspace");
        sqlx::query(
            r#"INSERT INTO pro_sync_account_settings (account_id, sync_enabled, updated_at)
               VALUES ('account-v3', 1, '2026-08-20T00:00:00Z')"#,
        )
        .execute(&pool)
        .await
        .expect("insert v3 account setting");
        sqlx::query(
            r#"INSERT INTO pro_workspace_sync_bindings (
                 account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, state, initial_cursor, initial_total, initial_confirmed,
                 initialization_checkpoint, ssh_task_v3_bootstrap_state, generation,
                 last_success_at, last_error, consecutive_failure_count, created_at, updated_at
               ) VALUES (
                 'account-v3', 'v3-retained-workspace', 'cloud-v3-retained', 102,
                 1, 'active', 11, 23, 23, 'v3-checkpoint', 'completed', 7,
                 '2026-08-20T01:00:00Z', NULL, 0,
                 '2026-08-20T00:00:00Z', '2026-08-20T01:00:00Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert v3 binding");

        sqlx::query(
            r#"INSERT INTO pro_sync_outbox (
                 account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
                 operation_id, parent_entity_id, operation, base_version,
                 payload_schema_version, canonical_payload_json, deleted_at, content_revision,
                 status, attempt_count, next_attempt_at, lease_owner, lease_started_at,
                 lease_expires_at, last_error, created_at, updated_at
               ) VALUES (
                 'account-v3', 'v3-retained-workspace', 'cloud-v3-retained',
                 'workspaceVariable', 'pending-variable', 'op-pending',
                 'v3-retained-workspace', 'upsert', 3, 1, '{"key":"pending"}', NULL, 8,
                 'pending', 1, NULL, NULL, NULL, NULL, NULL,
                 '2026-08-20T02:00:00Z', '2026-08-20T02:00:00Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert pending v3 outbox row");
        sqlx::query(
            r#"INSERT INTO pro_sync_outbox (
                 account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
                 operation_id, parent_entity_id, operation, base_version,
                 payload_schema_version, canonical_payload_json, deleted_at, content_revision,
                 status, attempt_count, next_attempt_at, lease_owner, lease_started_at,
                 lease_expires_at, last_error, created_at, updated_at
               ) VALUES (
                 'account-v3', 'v3-retained-workspace', 'cloud-v3-retained',
                 'apiCollection', 'uncertain-collection', 'op-uncertain',
                 'v3-retained-workspace', 'upsert', 4, 1, '{"name":"uncertain"}', NULL, 9,
                 'uncertain', 2, '2026-08-20T02:30:00Z', 'worker-v3',
                 '2026-08-20T02:00:00Z', '2026-08-20T02:45:00Z', 'retryable_transport',
                 '2026-08-20T02:00:00Z', '2026-08-20T02:30:00Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert uncertain v3 outbox row");
        sqlx::query(
            r#"INSERT INTO pro_sync_outbox (
                 account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
                 operation_id, parent_entity_id, operation, base_version,
                 payload_schema_version, canonical_payload_json, deleted_at, content_revision,
                 status, attempt_count, next_attempt_at, lease_owner, lease_started_at,
                 lease_expires_at, last_error, created_at, updated_at
               ) VALUES (
                 'account-v3', 'v3-retained-workspace', 'cloud-v3-retained',
                 'apiRequest', 'dead-request', 'op-dead', 'folder-v3', 'delete', 5, 1, NULL,
                 '2026-08-20T03:00:00Z', 10, 'dead', 4, NULL, NULL, NULL, NULL,
                 'invalid_sync_entity', '2026-08-20T02:00:00Z', '2026-08-20T03:00:00Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert dead v3 outbox row");
        sqlx::query(
            r#"INSERT INTO pro_sync_outbox (
                 account_id, local_workspace_id, cloud_workspace_id, entity_type, entity_id,
                 operation_id, parent_entity_id, operation, base_version,
                 payload_schema_version, canonical_payload_json, deleted_at, content_revision,
                 status, attempt_count, next_attempt_at, lease_owner, lease_started_at,
                 lease_expires_at, last_error, created_at, updated_at
               ) VALUES (
                 'account-v3', 'v3-retained-workspace', 'cloud-v3-retained',
                 'sshTask', 'unsupported-task', 'op-unsupported', NULL, 'upsert', 6, 1,
                 '{"name":"unsupported"}', NULL, 11, 'dead', 5, NULL, NULL, NULL, NULL,
                 'protocol_version_unsupported', '2026-08-20T02:00:00Z',
                 '2026-08-20T03:00:00Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert protocol dead v3 outbox row");

        sqlx::query(
            r#"INSERT INTO pro_sync_entity_state (
                 account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                 last_operation_id, sync_status, conflict_remote_payload_json,
                 conflict_remote_operation, conflict_parent_entity_id, conflict_deleted_at,
                 conflict_operation_id, updated_at
               ) VALUES (
                 'account-v3', 'cloud-v3-retained', 'workspaceVariable', 'synced-variable',
                 12, 'remote-synced', 'synced', NULL, NULL, NULL, NULL, NULL,
                 '2026-08-20T04:00:00Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert synced v3 entity state");
        sqlx::query(
            r#"INSERT INTO pro_sync_entity_state (
                 account_id, cloud_workspace_id, entity_type, entity_id, server_version,
                 last_operation_id, sync_status, conflict_remote_payload_json,
                 conflict_remote_operation, conflict_parent_entity_id, conflict_deleted_at,
                 conflict_operation_id, updated_at
               ) VALUES (
                 'account-v3', 'cloud-v3-retained', 'apiRequest', 'conflict-request',
                 13, 'remote-conflict', 'conflict', '{"name":"remote"}', 'upsert',
                 'folder-v3', NULL, 'remote-conflict', '2026-08-20T04:00:00Z'
               )"#,
        )
        .execute(&pool)
        .await
        .expect("insert conflict v3 entity state");

        migrate(&pool).await.expect("run v4 migration");
        migrate(&pool).await.expect("rerun v4 migration");

        let outbox: Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            i64,
            i64,
            Option<String>,
            Option<String>,
            i64,
            String,
            i64,
            Option<String>,
        )> = sqlx::query_as(
            r#"SELECT account_id, local_workspace_id, cloud_workspace_id, entity_type,
                      entity_id, operation_id, parent_entity_id, operation, base_version,
                      payload_schema_version, canonical_payload_json, deleted_at,
                      content_revision, status, attempt_count, last_error
               FROM cloud_sync_outbox ORDER BY operation_id"#,
        )
        .fetch_all(&pool)
        .await
        .expect("read retained outbox rows");
        assert_eq!(outbox.len(), 4);
        for row in &outbox {
            assert_eq!(row.0, "account-v3");
            assert_eq!(row.1, "v3-retained-workspace");
            assert_eq!(row.2, "cloud-v3-retained");
            assert_eq!(row.9, 1);
        }
        let dead = outbox
            .iter()
            .find(|row| row.5 == "op-dead")
            .expect("retained dead row");
        assert_eq!(dead.3, "apiRequest");
        assert_eq!(dead.4, "dead-request");
        assert_eq!(dead.6.as_deref(), Some("folder-v3"));
        assert_eq!(dead.7, "delete");
        assert_eq!(dead.8, 5);
        assert_eq!(dead.10, None);
        assert_eq!(dead.11.as_deref(), Some("2026-08-20T03:00:00Z"));
        assert_eq!(dead.12, 10);
        assert_eq!(dead.13, "dead");
        assert_eq!(dead.14, 4);
        assert_eq!(dead.15.as_deref(), Some("invalid_sync_entity"));
        let pending = outbox
            .iter()
            .find(|row| row.5 == "op-pending")
            .expect("retained pending row");
        assert_eq!(pending.3, "workspaceVariable");
        assert_eq!(pending.4, "pending-variable");
        assert_eq!(pending.6.as_deref(), Some("v3-retained-workspace"));
        assert_eq!(pending.7, "upsert");
        assert_eq!(pending.8, 3);
        assert_eq!(pending.10.as_deref(), Some(r#"{"key":"pending"}"#));
        assert_eq!(pending.11, None);
        assert_eq!(pending.12, 8);
        assert_eq!(pending.13, "pending");
        assert_eq!(pending.14, 1);
        assert_eq!(pending.15, None);
        let uncertain = outbox
            .iter()
            .find(|row| row.5 == "op-uncertain")
            .expect("retained uncertain row");
        assert_eq!(uncertain.3, "apiCollection");
        assert_eq!(uncertain.4, "uncertain-collection");
        assert_eq!(uncertain.7, "upsert");
        assert_eq!(uncertain.8, 4);
        assert_eq!(uncertain.10.as_deref(), Some(r#"{"name":"uncertain"}"#));
        assert_eq!(uncertain.12, 9);
        assert_eq!(uncertain.13, "uncertain");
        assert_eq!(uncertain.14, 2);
        assert_eq!(uncertain.15.as_deref(), Some("retryable_transport"));
        let unsupported = outbox
            .iter()
            .find(|row| row.5 == "op-unsupported")
            .expect("retained protocol dead row");
        assert_eq!(unsupported.3, "sshTask");
        assert_eq!(unsupported.4, "unsupported-task");
        assert_eq!(unsupported.7, "upsert");
        assert_eq!(unsupported.8, 6);
        assert_eq!(unsupported.10.as_deref(), Some(r#"{"name":"unsupported"}"#));
        assert_eq!(unsupported.12, 11);
        assert_eq!(unsupported.13, "dead");
        assert_eq!(unsupported.14, 5);
        assert_eq!(
            unsupported.15.as_deref(),
            Some("protocol_version_unsupported")
        );

        let entity_state: Vec<(
            String,
            String,
            String,
            String,
            i64,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            r#"SELECT account_id, cloud_workspace_id, entity_type, entity_id,
                      server_version, last_operation_id, sync_status,
                      conflict_remote_payload_json, conflict_remote_operation,
                      conflict_parent_entity_id, conflict_deleted_at, conflict_operation_id
               FROM cloud_sync_entity_state ORDER BY entity_id"#,
        )
        .fetch_all(&pool)
        .await
        .expect("read retained entity state rows");
        assert_eq!(entity_state.len(), 2);
        let synced = entity_state
            .iter()
            .find(|row| row.3 == "synced-variable")
            .expect("retained synced state");
        assert_eq!(synced.0, "account-v3");
        assert_eq!(synced.1, "cloud-v3-retained");
        assert_eq!(synced.2, "workspaceVariable");
        assert_eq!(synced.4, 12);
        assert_eq!(synced.5.as_deref(), Some("remote-synced"));
        assert_eq!(synced.6, "synced");
        assert_eq!(synced.7, None);
        assert_eq!(synced.8, None);
        assert_eq!(synced.9, None);
        assert_eq!(synced.10, None);
        assert_eq!(synced.11, None);
        let conflict = entity_state
            .iter()
            .find(|row| row.3 == "conflict-request")
            .expect("retained conflict state");
        assert_eq!(conflict.4, 13);
        assert_eq!(conflict.5.as_deref(), Some("remote-conflict"));
        assert_eq!(conflict.6, "conflict");
        assert_eq!(conflict.7.as_deref(), Some(r#"{"name":"remote"}"#));
        assert_eq!(conflict.8.as_deref(), Some("upsert"));
        assert_eq!(conflict.9.as_deref(), Some("folder-v3"));
        assert_eq!(conflict.11.as_deref(), Some("remote-conflict"));

        let binding: (i64, String, String, i64, i64) = sqlx::query_as(
            r#"SELECT last_pulled_cursor, ssh_task_v3_bootstrap_state,
                      connection_v4_bootstrap_state, initial_total, initial_confirmed
               FROM cloud_sync_workspace_bindings
               WHERE local_workspace_id = 'v3-retained-workspace'"#,
        )
        .fetch_one(&pool)
        .await
        .expect("read retained binding");
        assert_eq!(binding, (102, "completed".into(), "pending".into(), 23, 23));

        let foreign_key_rows = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("check migrated foreign keys");
        assert!(foreign_key_rows.is_empty());
        for index in [
            "idx_pro_sync_outbox_due",
            "idx_pro_sync_entity_state_conflicts",
        ] {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1)",
            )
            .bind(index)
            .fetch_one(&pool)
            .await
            .expect("check migrated index");
            assert!(exists, "missing migrated index {index}");
        }
    }

    #[tokio::test]
    async fn protocol_v4_reconciliation_migration_reopens_completed_bootstrap() {
        let pool = test_pool().await;
        let db = unfour_local_storage::LocalDb::from_pool(pool.clone());
        db.migrate().await.expect("run core migrations");

        let v4_cutoff = 20260821055320_i64;
        for migration in cloud_sync_migrator()
            .iter()
            .filter(|migration| migration.version <= v4_cutoff)
        {
            sqlx::raw_sql(migration.sql.as_ref())
                .execute(&pool)
                .await
                .expect("apply pre-retry migrations");
            record_migration_with_checksum(
                &pool,
                migration.version,
                migration.description.as_ref(),
                migration.checksum.as_ref(),
            )
            .await;
        }

        sqlx::query(
            r#"INSERT INTO workspaces (
                 id, name, is_default, last_opened_at, environment_type, mcp_policy,
                 created_at, updated_at, revision
               ) VALUES ('completed-bootstrap-workspace', 'Completed bootstrap', 0, NULL,
                         'dev', 'auto', '2026-08-21T00:00:00Z',
                         '2026-08-21T00:00:00Z', 1)"#,
        )
        .execute(&pool)
        .await
        .expect("insert completed-bootstrap workspace");
        sqlx::query(
            r#"INSERT INTO pro_workspace_sync_bindings (
                 account_id, local_workspace_id, cloud_workspace_id, last_pulled_cursor,
                 sync_enabled, state, initial_cursor, initial_total, initial_confirmed,
                 ssh_task_v3_bootstrap_state, connection_v4_bootstrap_state, generation,
                 created_at, updated_at
               ) VALUES ('account-retry', 'completed-bootstrap-workspace',
                         'cloud-completed-bootstrap', 17, 1, 'active', 0, 1, 1,
                         'completed', 'completed', 0, '2026-08-21T00:00:00Z',
                         '2026-08-21T00:00:00Z')"#,
        )
        .execute(&pool)
        .await
        .expect("insert completed bootstrap binding");

        migrate(&pool)
            .await
            .expect("run reconciliation retry migration");
        let state: String = sqlx::query_scalar(
            "SELECT connection_v4_bootstrap_state FROM cloud_sync_workspace_bindings",
        )
        .fetch_one(&pool)
        .await
        .expect("read reopened bootstrap state");
        assert_eq!(state, "pending");

        migrate(&pool)
            .await
            .expect("rerun reconciliation retry migration");
        let state_after_retry: String = sqlx::query_scalar(
            "SELECT connection_v4_bootstrap_state FROM cloud_sync_workspace_bindings",
        )
        .fetch_one(&pool)
        .await
        .expect("read idempotent bootstrap state");
        assert_eq!(state_after_retry, "pending");
    }

    async fn apply_historical_pro_migrations(pool: &SqlitePool) {
        for migration in cloud_sync_migrator()
            .iter()
            .filter(|migration| migration.version <= HISTORICAL_PRO_MIGRATION_CUTOFF)
        {
            sqlx::raw_sql(migration.sql.as_ref())
                .execute(pool)
                .await
                .unwrap_or_else(|error| {
                    panic!("apply historical migration {}: {error}", migration.version)
                });
            record_migration_with_checksum(
                pool,
                migration.version,
                migration.description.as_ref(),
                migration.checksum.as_ref(),
            )
            .await;
        }
    }

    async fn record_migration_with_checksum(
        pool: &SqlitePool,
        version: i64,
        description: &str,
        checksum: &[u8],
    ) {
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
        .bind(checksum)
        .execute(pool)
        .await
        .expect("record migration");
    }

    async fn migration_versions(pool: &SqlitePool) -> Vec<i64> {
        sqlx::query_as::<_, (i64,)>("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .expect("list migrations")
            .into_iter()
            .map(|(version,)| version)
            .collect()
    }

    async fn table_exists(pool: &SqlitePool, table_name: &str) -> bool {
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
                .bind(table_name)
                .fetch_optional(pool)
                .await
                .expect("check table");
        exists.is_some()
    }

    async fn table_row_count(pool: &SqlitePool, table_name: &str) -> i64 {
        let sql = format!("SELECT COUNT(*) FROM {table_name}");
        sqlx::query_scalar(&sql)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|error| panic!("count {table_name}: {error}"))
    }
}
