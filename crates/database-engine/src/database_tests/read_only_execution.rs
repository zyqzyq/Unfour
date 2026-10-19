use super::super::*;
use super::support::{
    mysql_input, postgres_input, service_with_workspace, sqlite_fixture, sqlite_input,
};
use std::fs;
use unfour_core::models::DatabaseScriptInput;

fn query(
    workspace_id: &str,
    connection_id: &str,
    sql: &str,
    confirm_mutation: Option<bool>,
    enforce_read_only: bool,
) -> DatabaseQueryInput {
    DatabaseQueryInput {
        workspace_id: workspace_id.to_string(),
        connection_id: connection_id.to_string(),
        sql: sql.to_string(),
        limit: Some(10),
        confirm_mutation,
        catalog: None,
        schema: None,
        timeout_ms: Some(5_000),
        enforce_read_only,
    }
}

#[test]
fn select_nextval_stays_a_proven_read_without_a_function_blacklist() {
    for dialect in [
        DatabaseDialect::Postgres,
        DatabaseDialect::Sqlite,
        DatabaseDialect::Mysql,
        DatabaseDialect::Generic,
    ] {
        assert!(
            sql_is_proven_read("SELECT nextval('users_id_seq')", dialect),
            "{dialect:?}"
        );
        assert!(!sql_is_proven_read(
            "WITH d AS (DELETE FROM users RETURNING *) SELECT nextval('users_id_seq') FROM d",
            dialect
        ));
    }
}

#[tokio::test]
async fn sqlite_enforced_read_only_blocks_same_connection_writes_and_recovers() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save writable connection");

    let mode = service
        .execute_query(query(
            &workspace_id,
            &connection.id,
            "SELECT * FROM pragma_query_only",
            None,
            true,
        ))
        .await
        .expect("read under enforced constraint");
    assert_eq!(mode.rows[0][0].as_deref(), Some("1"));
    assert_eq!(mode.safety.classification, "read");

    let released = service
        .execute_query(query(
            &workspace_id,
            &connection.id,
            "SELECT * FROM pragma_query_only",
            None,
            false,
        ))
        .await
        .expect("later checkout is writable");
    assert_eq!(released.rows[0][0].as_deref(), Some("0"));

    let before = count_deploys(&service, &workspace_id, &connection.id).await;
    let input = query(
        &workspace_id,
        &connection.id,
        "INSERT INTO deploys (service, version) VALUES ('side', '1')",
        Some(true),
        true,
    );
    let mut conn = service
        .script_connection(&connection, &input)
        .await
        .expect("checkout");
    conn.establish_read_only()
        .await
        .expect("constraint on the checked-out connection");
    let safety = classify_query_for_dialect(&input.sql, DatabaseDialect::Sqlite);
    let write = conn.execute(&input.sql, 10, safety).await;
    assert!(
        write.is_err(),
        "query_only must reject a write on this connection: {write:?}"
    );
    conn.release_read_only().await.expect("clear query_only");
    drop(conn);
    assert_eq!(
        count_deploys(&service, &workspace_id, &connection.id).await,
        before,
        "rejected write must not persist"
    );

    let missing = service
        .execute_query(query(
            &workspace_id,
            &connection.id,
            "SELECT * FROM missing_read_only_table",
            None,
            true,
        ))
        .await;
    assert!(missing.is_err(), "failed read: {missing:?}");
    assert_eq!(
        count_deploys(&service, &workspace_id, &connection.id).await,
        before
    );

    service
        .execute_query(query(
            &workspace_id,
            &connection.id,
            "INSERT INTO deploys (service, version) VALUES ('recovered', '1')",
            Some(true),
            false,
        ))
        .await
        .expect("writable execution after a failed read-only attempt");
    assert_eq!(
        count_deploys(&service, &workspace_id, &connection.id).await,
        before + 1
    );

    let wrapped = service
        .execute_query(query(
            &workspace_id,
            &connection.id,
            "WITH d AS (DELETE FROM deploys RETURNING *) SELECT * FROM d",
            Some(true),
            true,
        ))
        .await;
    assert!(matches!(wrapped, Err(AppError::ReadOnly(_))));
    assert_eq!(
        count_deploys(&service, &workspace_id, &connection.id).await,
        before + 1
    );

    let script = DatabaseScriptInput {
        query: query(
            &workspace_id,
            &connection.id,
            "BEGIN; INSERT INTO deploys (service, version) VALUES ('tx', '1'); COMMIT;",
            Some(true),
            false,
        ),
        run_id: uuid::Uuid::new_v4().to_string(),
        cursor_offset: None,
        explain: false,
    };
    let script_result = service.execute_script(script).await.expect("script");
    assert!(script_result
        .statements
        .iter()
        .all(|s| s.status == "success"));
    assert_eq!(
        count_deploys(&service, &workspace_id, &connection.id).await,
        before + 2,
        "multi-statement transaction semantics stay intact"
    );

    let read_only = service
        .save_connection(DatabaseConnectionInput {
            read_only: true,
            ..sqlite_input(&workspace_id, &path)
        })
        .await
        .expect("save read-only connection");
    let flagged = service
        .execute_query(query(
            &workspace_id,
            &read_only.id,
            "SELECT * FROM pragma_query_only",
            None,
            false,
        ))
        .await
        .expect("saved read-only flag also enables the constraint");
    assert_eq!(flagged.rows[0][0].as_deref(), Some("1"));

    let _ = fs::remove_file(path);
}

async fn count_deploys(service: &DatabaseService, workspace_id: &str, connection_id: &str) -> i64 {
    let result = service
        .execute_query(query(
            workspace_id,
            connection_id,
            "SELECT COUNT(*) FROM deploys",
            None,
            false,
        ))
        .await
        .expect("count");
    result.rows[0][0]
        .as_deref()
        .unwrap()
        .parse()
        .expect("count parses")
}

#[tokio::test]
async fn postgres_and_opengauss_fail_closed_when_read_only_cannot_be_verified() {
    // Loopback protocol fixture. It records the statements we send. It is not
    // a PostgreSQL or openGauss server, so it does not verify `nextval`.
    for version in [
        "PostgreSQL 16.4 on fixture",
        "PostgreSQL 9.2.4 (openGauss 5.0.0 fixture)",
    ] {
        let server = super::profile_server::ProfileServer::start(version).await;
        let (service, workspace_id) = service_with_workspace().await;
        let mut input = postgres_input(&workspace_id);
        input.port = Some(server.port);
        let connection = service.save_connection(input).await.expect("save");

        *server.transaction_read_only.lock().unwrap() = "on".into();
        let read = service
            .execute_query(query(&workspace_id, &connection.id, "SELECT 1", None, true))
            .await
            .expect("verified fixture constraint");
        assert_eq!(read.rows.len(), 1);
        assert_read_only_sequence(
            &server.queries.lock().unwrap(),
            &[
                "BEGIN READ ONLY",
                "SHOW transaction_read_only",
                "SELECT 1",
                "ROLLBACK",
            ],
        );

        server.queries.lock().unwrap().clear();
        *server.transaction_read_only.lock().unwrap() = "off".into();
        let denied = service
            .execute_query(query(
                &workspace_id,
                &connection.id,
                "SELECT nextval('users_id_seq')",
                None,
                true,
            ))
            .await;
        assert!(
            matches!(denied, Err(AppError::ReadOnly(_))),
            "{version}: {denied:?}"
        );
        let queries = server.queries.lock().unwrap().clone();
        assert!(
            queries
                .iter()
                .all(|sql| !sql.to_lowercase().contains("nextval")),
            "{version} sent the caller statement without a verified constraint: {queries:?}"
        );
        assert_read_only_sequence(
            &queries,
            &["BEGIN READ ONLY", "SHOW transaction_read_only", "ROLLBACK"],
        );

        server.queries.lock().unwrap().clear();
        *server.transaction_read_only.lock().unwrap() = "reject".into();
        let rejected = service
            .execute_query(query(
                &workspace_id,
                &connection.id,
                "SELECT nextval('users_id_seq')",
                None,
                true,
            ))
            .await;
        assert!(rejected.is_err(), "{version}");
        let queries = server.queries.lock().unwrap().clone();
        assert!(queries
            .iter()
            .all(|sql| !sql.to_lowercase().contains("nextval")));
        assert!(queries
            .iter()
            .any(|sql| sql.trim().eq_ignore_ascii_case("BEGIN READ ONLY")));
    }
}

#[tokio::test]
async fn mysql_fail_closed_when_read_only_cannot_be_verified() {
    // Loopback protocol fixture. Not a MySQL server.
    let server = super::mysql_profile_server::MysqlProfileServer::start().await;
    let (service, workspace_id) = service_with_workspace().await;
    let mut input = mysql_input(&workspace_id, None);
    input.port = Some(server.port);
    let connection = service.save_connection(input).await.expect("save");

    *server.transaction_read_only.lock().unwrap() = "on".into();
    service
        .execute_query(query(&workspace_id, &connection.id, "SELECT 1", None, true))
        .await
        .expect("verified fixture constraint");
    assert_read_only_sequence(
        &server.queries.lock().unwrap(),
        &[
            "START TRANSACTION READ ONLY",
            "SELECT @@transaction_read_only",
            "SELECT 1",
            "ROLLBACK",
        ],
    );

    server.queries.lock().unwrap().clear();
    *server.transaction_read_only.lock().unwrap() = "off".into();
    let denied = service
        .execute_query(query(
            &workspace_id,
            &connection.id,
            "SELECT nextval('users_id_seq')",
            None,
            true,
        ))
        .await;
    assert!(matches!(denied, Err(AppError::ReadOnly(_))), "{denied:?}");
    let queries = server.queries.lock().unwrap().clone();
    assert!(queries
        .iter()
        .all(|sql| !sql.to_lowercase().contains("nextval")));

    server.queries.lock().unwrap().clear();
    *server.transaction_read_only.lock().unwrap() = "reject".into();
    let rejected = service
        .execute_query(query(
            &workspace_id,
            &connection.id,
            "SELECT nextval('users_id_seq')",
            None,
            true,
        ))
        .await;
    assert!(rejected.is_err());
    let queries = server.queries.lock().unwrap().clone();
    assert!(queries
        .iter()
        .all(|sql| !sql.to_lowercase().contains("nextval")));
    assert!(queries.iter().any(|sql| sql
        .trim()
        .eq_ignore_ascii_case("START TRANSACTION READ ONLY")));
}

#[tokio::test]
async fn unconstrained_postgres_query_does_not_open_a_read_only_transaction() {
    let server = super::profile_server::ProfileServer::start("PostgreSQL 16.4 on fixture").await;
    let (service, workspace_id) = service_with_workspace().await;
    let mut input = postgres_input(&workspace_id);
    input.port = Some(server.port);
    let connection = service.save_connection(input).await.expect("save");
    service
        .execute_query(query(
            &workspace_id,
            &connection.id,
            "SELECT 1",
            None,
            false,
        ))
        .await
        .expect("ordinary read");
    let queries = server.queries.lock().unwrap().clone();
    assert!(queries
        .iter()
        .all(|sql| !sql.to_ascii_uppercase().contains("READ ONLY")));
    assert!(queries
        .iter()
        .all(|sql| !sql.trim().eq_ignore_ascii_case("BEGIN READ ONLY")));
}

fn assert_read_only_sequence(queries: &[String], expected: &[&str]) {
    let relevant: Vec<String> = queries
        .iter()
        .map(|sql| sql.trim().to_string())
        .filter(|sql| {
            !sql.eq_ignore_ascii_case("SELECT version()")
                && !sql.eq_ignore_ascii_case("SELECT VERSION()")
                && !sql.to_ascii_uppercase().starts_with("SET ")
        })
        .collect();
    assert_eq!(relevant, expected);
}
