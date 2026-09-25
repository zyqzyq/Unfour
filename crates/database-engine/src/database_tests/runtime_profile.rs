use super::super::*;
use super::mysql_profile_server::MysqlProfileServer;
use super::profile_server::ProfileServer;
use super::support::{
    mysql_input, postgres_input, service_with_workspace, sqlite_fixture, sqlite_input,
    RejectingServer, SilentServer,
};
use unfour_core::domain::connection_entity_key;

#[test]
fn postgres_detection_requires_a_product_banner_and_rejects_known_fork_markers() {
    use DetectedServerType::*;
    for banner in [
        "PostgreSQL 16.4 on x86_64, compiled by gcc",
        " PostgreSQL 9.6.24 ",
        "PostgreSQL 17.2 (Debian)",
    ] {
        assert_eq!(
            runtime_profile::detect_postgres_server(banner),
            PostgreSql,
            "{banner}"
        );
    }
    for banner in [
        "",
        "PostgreSQL",
        "PostgreSQL compatible",
        "not PostgreSQL 16.4",
        "PostgreSQL 8.0.2 (Redshift 1.0)",
        "CockroachDB v24.1",
        "PostgreSQL 9.2.4 (GaussDB 5.0)",
        "Acme compatible with PostgreSQL 16.4",
    ] {
        assert_eq!(
            runtime_profile::detect_postgres_server(banner),
            UnknownPostgresCompatible,
            "{banner}"
        );
    }
}

#[test]
fn profiles_resolve_dialects_and_conservative_capabilities() {
    use DetectedServerType::*;
    for (server, dialect, catalogs, schemas) in [
        (Sqlite, DatabaseDialect::Sqlite, false, false),
        (PostgreSql, DatabaseDialect::Postgres, true, true),
        (Mysql, DatabaseDialect::Mysql, true, false),
        (
            UnknownPostgresCompatible,
            DatabaseDialect::Postgres,
            false,
            true,
        ),
    ] {
        let profile = RuntimeDatabaseProfile::resolve(server, Some("test-version".into()));
        assert_eq!(profile.detected_server_type, server);
        assert_eq!(profile.server_version.as_deref(), Some("test-version"));
        assert_eq!(profile.dialect, dialect);
        assert_eq!(profile.capabilities.catalogs, catalogs);
        assert_eq!(profile.capabilities.schemas, schemas);
        let known = server != UnknownPostgresCompatible;
        assert_eq!(profile.capabilities.indexes, known);
        assert_eq!(profile.capabilities.foreign_keys, known);
        assert_eq!(profile.capabilities.ddl, known);
        assert_eq!(profile.capabilities.generated_columns, known);
        assert_eq!(profile.capabilities.row_mutation, known);
        assert_eq!(profile.capabilities.export, known);
        assert!(profile.capabilities.explain);
        assert_eq!(
            require_capability(profile.capabilities.row_mutation, "row mutation").is_ok(),
            known
        );
    }
}

#[tokio::test]
async fn postgres_connect_detects_again_without_persisting_or_syncing_profile() {
    let server = ProfileServer::start("PostgreSQL 16.4 on fixture").await;
    let (service, workspace) = service_with_workspace().await;
    let mut input = postgres_input(&workspace);
    input.port = Some(server.port);
    let saved = service.save_connection(input).await.unwrap();
    let key = connection_entity_key(&workspace, &saved.id);
    let before =
        serde_json::to_value(service.read_connection_domain_snapshot(&key).await.unwrap()).unwrap();
    let profile = tokio::time::timeout(
        Duration::from_secs(5),
        service.runtime_profile(workspace.clone(), saved.id.clone(), None),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(profile.detected_server_type, DetectedServerType::PostgreSql);
    assert_eq!(
        profile.server_version.as_deref(),
        Some("PostgreSQL 16.4 on fixture")
    );
    *server.version.lock().unwrap() = "Acme PostgreSQL-compatible 1.0".into();
    let next = service
        .runtime_profile(
            workspace.clone(),
            saved.id.clone(),
            Some("another_catalog".into()),
        )
        .await
        .unwrap();
    assert_eq!(
        next.detected_server_type,
        DetectedServerType::UnknownPostgresCompatible
    );
    assert_eq!(
        next.server_version.as_deref(),
        Some("Acme PostgreSQL-compatible 1.0")
    );
    assert!(!next.capabilities.ddl);
    assert_eq!(
        service
            .list_catalogs(workspace.clone(), saved.id.clone())
            .await
            .unwrap(),
        vec!["testdb"]
    );
    assert!(server
        .queries
        .lock()
        .unwrap()
        .iter()
        .all(|sql| sql == "SELECT version()"));
    assert_eq!(server.queries.lock().unwrap().len(), 3);
    let after =
        serde_json::to_value(service.read_connection_domain_snapshot(&key).await.unwrap()).unwrap();
    assert_eq!(
        before, after,
        "runtime reads must not change sync snapshots"
    );
    let reloaded = service.get_connection(&workspace, &saved.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(&saved).unwrap(),
        serde_json::to_value(reloaded).unwrap()
    );
    let config: String =
        sqlx::query_scalar("SELECT config_json FROM database_connections WHERE connection_id = ?")
            .bind(&saved.id)
            .fetch_one(service.db.pool())
            .await
            .unwrap();
    assert_eq!(config, "{}");
    *server.version.lock().unwrap() = "probe-error".into();
    let degraded = service
        .runtime_profile(workspace.clone(), saved.id.clone(), None)
        .await
        .expect("version probe failure must not fail the pool");
    assert_eq!(
        degraded.detected_server_type,
        DetectedServerType::UnknownPostgresCompatible
    );
    assert_eq!(degraded.server_version, None);
    assert_eq!(degraded.dialect, DatabaseDialect::Postgres);
    assert!(!degraded.capabilities.catalogs);
    assert!(degraded.capabilities.schemas);
    assert!(!degraded.capabilities.ddl);
    let tested = service
        .test_connection(workspace, saved.id)
        .await
        .expect("version probe failure must not fail test_connection");
    assert!(tested.ok);
    assert_eq!(tested.server_version, None);
}

#[tokio::test]
async fn mysql_version_probe_failure_keeps_pool_and_baseline_profile() {
    let server = MysqlProfileServer::start().await;
    let (service, workspace) = service_with_workspace().await;
    let mut input = mysql_input(&workspace, None);
    input.port = Some(server.port);
    let saved = service.save_connection(input).await.unwrap();
    let profile = tokio::time::timeout(
        Duration::from_secs(5),
        service.runtime_profile(workspace.clone(), saved.id.clone(), None),
    )
    .await
    .expect("mysql probe fallback timed out")
    .expect("version probe failure must not fail the mysql pool");
    assert_eq!(profile.detected_server_type, DetectedServerType::Mysql);
    assert_eq!(profile.server_version, None);
    assert_eq!(profile.dialect, DatabaseDialect::Mysql);
    assert!(profile.capabilities.catalogs);
    assert!(!profile.capabilities.schemas);
    assert!(profile.capabilities.ddl);
    assert!(profile.capabilities.row_mutation);
    let tested = service
        .test_connection(workspace, saved.id)
        .await
        .expect("version probe failure must not fail test_connection");
    assert!(tested.ok);
    assert_eq!(tested.server_version, None);
    assert!(server
        .queries
        .lock()
        .unwrap()
        .iter()
        .any(|sql| sql.trim().eq_ignore_ascii_case("SELECT VERSION()")));
}

#[tokio::test]
async fn postgres_version_probe_timeout_keeps_conservative_fallback() {
    let server = ProfileServer::start("probe-hang").await;
    let (service, workspace) = service_with_workspace().await;
    let mut input = postgres_input(&workspace);
    input.port = Some(server.port);
    let saved = service.save_connection(input).await.unwrap();
    let profile = service
        .runtime_profile(workspace.clone(), saved.id.clone(), None)
        .await
        .expect("version probe timeout must not fail the pool");
    assert_eq!(
        profile.detected_server_type,
        DetectedServerType::UnknownPostgresCompatible
    );
    assert_eq!(profile.server_version, None);
    assert_eq!(profile.dialect, DatabaseDialect::Postgres);
    assert!(!profile.capabilities.catalogs);
    assert!(profile.capabilities.schemas);
    assert!(!profile.capabilities.indexes);
    assert!(!profile.capabilities.foreign_keys);
    assert!(!profile.capabilities.ddl);
    assert!(!profile.capabilities.generated_columns);
    assert!(!profile.capabilities.row_mutation);
    assert!(!profile.capabilities.export);
    assert!(profile.capabilities.explain);
    let tested = service
        .test_connection(workspace, saved.id)
        .await
        .expect("version probe timeout must not fail test_connection");
    assert!(tested.ok);
    assert_eq!(tested.server_version, None);
    assert!(server
        .queries
        .lock()
        .unwrap()
        .iter()
        .any(|sql| sql == "SELECT version()"));
}

#[tokio::test]
async fn mysql_version_probe_timeout_keeps_baseline_profile() {
    let server = MysqlProfileServer::start_hanging_probe().await;
    let (service, workspace) = service_with_workspace().await;
    let mut input = mysql_input(&workspace, None);
    input.port = Some(server.port);
    let saved = service.save_connection(input).await.unwrap();
    let profile = service
        .runtime_profile(workspace.clone(), saved.id.clone(), None)
        .await
        .expect("version probe timeout must not fail the mysql pool");
    assert_eq!(profile.detected_server_type, DetectedServerType::Mysql);
    assert_eq!(profile.server_version, None);
    assert_eq!(profile.dialect, DatabaseDialect::Mysql);
    assert!(profile.capabilities.catalogs);
    assert!(!profile.capabilities.schemas);
    assert!(profile.capabilities.ddl);
    assert!(profile.capabilities.row_mutation);
    let tested = service
        .test_connection(workspace, saved.id)
        .await
        .expect("version probe timeout must not fail test_connection");
    assert!(tested.ok);
    assert_eq!(tested.server_version, None);
    assert!(server
        .queries
        .lock()
        .unwrap()
        .iter()
        .any(|sql| sql.trim().eq_ignore_ascii_case("SELECT VERSION()")));
}

#[tokio::test]
async fn transport_failure_is_still_a_connection_error() {
    let server = RejectingServer::start().await;
    let (service, workspace) = service_with_workspace().await;
    let mut postgres = postgres_input(&workspace);
    postgres.port = Some(server.port);
    let saved = service.save_connection(postgres).await.unwrap();
    assert!(service
        .runtime_profile(workspace.clone(), saved.id.clone(), None)
        .await
        .is_err());
    assert!(service
        .test_connection(workspace.clone(), saved.id)
        .await
        .is_err());

    let mut mysql = mysql_input(&workspace, None);
    mysql.port = Some(server.port);
    let saved = service.save_connection(mysql).await.unwrap();
    assert!(service
        .runtime_profile(workspace.clone(), saved.id.clone(), None)
        .await
        .is_err());
    assert!(service.test_connection(workspace, saved.id).await.is_err());
}

#[tokio::test]
async fn connect_timeout_is_still_a_connection_error() {
    let server = SilentServer::start().await;
    let (service, workspace) = service_with_workspace().await;
    let mut postgres = postgres_input(&workspace);
    postgres.port = Some(server.port);
    let saved = service.save_connection(postgres).await.unwrap();
    assert!(service
        .runtime_profile(workspace.clone(), saved.id.clone(), None)
        .await
        .is_err());
    assert!(service
        .test_connection(workspace.clone(), saved.id)
        .await
        .is_err());

    let mut mysql = mysql_input(&workspace, None);
    mysql.port = Some(server.port);
    let saved = service.save_connection(mysql).await.unwrap();
    assert!(service
        .runtime_profile(workspace.clone(), saved.id.clone(), None)
        .await
        .is_err());
    assert!(service.test_connection(workspace, saved.id).await.is_err());
}

#[tokio::test]
async fn existing_sqlite_connection_needs_no_new_fields_or_migration() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let saved = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let profile = service
        .runtime_profile(workspace.clone(), saved.id.clone(), None)
        .await
        .unwrap();
    let tested = service
        .test_connection(workspace.clone(), saved.id.clone())
        .await
        .unwrap();
    assert_eq!(profile.detected_server_type, DetectedServerType::Sqlite);
    assert_eq!(profile.server_version, tested.server_version);
    assert!(profile.capabilities.row_mutation);
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('database_connections')")
            .fetch_all(service.db.pool())
            .await
            .unwrap();
    assert_eq!(
        columns,
        [
            "connection_id",
            "driver",
            "database_name",
            "username",
            "ssl_mode",
            "read_only",
            "config_json"
        ]
    );
    let schema = service.schema(workspace, saved.id, None).await.unwrap();
    assert!(schema.tables.iter().any(|t| t.name == "deploys"));
    // SQLx closes dropped pools asynchronously; Windows retains file locks
    // until those background closes finish.
    for attempt in 0..20 {
        match std::fs::remove_file(&path) {
            Ok(()) => break,
            Err(error) if attempt == 19 => panic!("fixture cleanup: {error}"),
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
}

#[tokio::test]
async fn unknown_metadata_fallback_reads_standard_columns_and_primary_keys() {
    // Exercise the actual selected fallback SQL against a minimal standard
    // catalog fixture. No PostgreSQL catalog or generated/identity fields exist.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    pool.execute(
        r#"
        ATTACH DATABASE ':memory:' AS information_schema;
        CREATE TABLE information_schema.columns (
            column_name TEXT, data_type TEXT, is_nullable TEXT, column_default TEXT,
            table_schema TEXT, table_name TEXT, ordinal_position INTEGER
        );
        CREATE TABLE information_schema.table_constraints (
            constraint_catalog TEXT, constraint_schema TEXT, constraint_name TEXT,
            table_schema TEXT, table_name TEXT, constraint_type TEXT
        );
        CREATE TABLE information_schema.key_column_usage (
            constraint_catalog TEXT, constraint_schema TEXT, constraint_name TEXT,
            table_schema TEXT, table_name TEXT, column_name TEXT
        );
        INSERT INTO information_schema.columns VALUES
            ('id', 'integer', 'NO', NULL, 'public', 'items', 1),
            ('name', 'text', 'YES', NULL, 'public', 'items', 2);
        INSERT INTO information_schema.table_constraints VALUES
            ('db', 'public', 'items_pk', 'public', 'items', 'PRIMARY KEY');
        INSERT INTO information_schema.key_column_usage VALUES
            ('db', 'public', 'items_pk', 'public', 'items', 'id');
    "#,
    )
    .await
    .unwrap();
    let unknown = RuntimeDatabaseProfile::postgres("unknown compatible server".into());
    let sql = postgres_columns_sql(&unknown);
    let rows = sqlx::query(sql)
        .bind("public")
        .bind("items")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("column_name"), "id");
    assert!(rows[0].get::<bool, _>("primary_key"));
    assert!(!rows[1].get::<bool, _>("primary_key"));
    assert_eq!(rows[0].get::<String, _>("is_generated"), "NEVER");
    assert_eq!(
        rows[0].get::<Option<String>, _>("identity_generation"),
        None
    );
    let postgres = RuntimeDatabaseProfile::postgres("PostgreSQL 16.4".into());
    assert_ne!(sql, postgres_columns_sql(&postgres));
    pool.close().await;
}
