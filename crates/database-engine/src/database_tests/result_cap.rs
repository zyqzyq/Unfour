//! Display-cap regression for SQLite and the PostgreSQL, openGauss, and MySQL
//! transports. The cap is not injected into SQL. Truncation is recorded only
//! when the driver yields another row of the retained result set.
use super::super::*;
use super::support::{
    mysql_input, postgres_input, service_with_workspace, sqlite_fixture, sqlite_input,
};
use unfour_core::models::{DatabaseQueryInput, DatabaseQueryResult, DatabaseScriptInput};

pub(super) struct ResultFixture {
    pub rows: usize,
    pub sets: usize,
    pub affected: u64,
}

pub(super) fn result_fixture(sql: &str) -> Option<ResultFixture> {
    let marker = "/*unfour-result ";
    let start = sql.find(marker)?;
    let body = sql[start + marker.len()..].split("*/").next()?;
    let mut rows = None;
    let mut sets = 1usize;
    let mut affected = None;
    for part in body.split_whitespace() {
        if let Some(value) = part.strip_prefix("rows=") {
            rows = value.parse().ok();
        } else if let Some(value) = part.strip_prefix("sets=") {
            sets = value.parse().unwrap_or(1);
        } else if let Some(value) = part.strip_prefix("affected=") {
            affected = value.parse().ok();
        }
    }
    let rows = rows?;
    Some(ResultFixture {
        rows,
        sets: sets.max(1),
        affected: affected.unwrap_or(rows as u64),
    })
}

fn assert_cap(
    result: &DatabaseQueryResult,
    rows: usize,
    truncated: bool,
    limit: u32,
    affected: u64,
) {
    assert_eq!(result.row_limit, limit, "{result:?}");
    assert_eq!(result.rows.len(), rows, "{result:?}");
    assert_eq!(result.truncated, truncated, "{result:?}");
    assert_eq!(result.affected_rows, affected, "{result:?}");
}

fn assert_sql_unchanged(queries: &[String], marker: &str, limit_clause: Option<&str>) {
    let matched: Vec<_> = queries
        .iter()
        .filter(|query| query.contains(marker))
        .collect();
    assert!(!matched.is_empty(), "missing {marker} in {queries:?}");
    for query in matched {
        let limits = query.matches("LIMIT").count();
        match limit_clause {
            Some(clause) => {
                assert!(query.contains(clause), "{query}");
                assert_eq!(limits, 1, "{query}");
            }
            None => assert_eq!(limits, 0, "{query}"),
        }
    }
}

fn query(
    workspace: &str,
    connection: &str,
    sql: &str,
    limit: u32,
    confirm: bool,
) -> DatabaseQueryInput {
    DatabaseQueryInput {
        workspace_id: workspace.into(),
        connection_id: connection.into(),
        sql: sql.into(),
        limit: Some(limit),
        confirm_mutation: Some(confirm),
        catalog: None,
        schema: None,
        timeout_ms: Some(5_000),

        enforce_read_only: false,
    }
}

fn script(
    workspace: &str,
    connection: &str,
    sql: &str,
    limit: u32,
    confirm: bool,
) -> DatabaseScriptInput {
    DatabaseScriptInput {
        query: query(workspace, connection, sql, limit, confirm),
        run_id: uuid::Uuid::new_v4().to_string(),
        cursor_offset: None,
        explain: false,
    }
}

async fn sqlite_service() -> (DatabaseService, String, String, std::path::PathBuf) {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    (service, workspace, connection.id, path)
}

#[tokio::test]
async fn sqlite_exact_limit_is_not_truncated_and_one_extra_row_is() {
    let (service, workspace, connection, path) = sqlite_service().await;
    let sql = "WITH RECURSIVE s(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM s WHERE n < 3) SELECT n FROM s";
    let exact = service
        .execute_query(query(&workspace, &connection, sql, 3, false))
        .await
        .unwrap();
    assert_cap(&exact, 3, false, 3, 0);
    assert_eq!(
        exact
            .rows
            .iter()
            .map(|row| row[0].as_deref())
            .collect::<Vec<_>>(),
        [Some("1"), Some("2"), Some("3")]
    );
    let over = service
        .execute_query(query(&workspace, &connection, sql, 2, false))
        .await
        .unwrap();
    assert_cap(&over, 2, true, 2, 0);
    assert_eq!(over.rows[0][0].as_deref(), Some("1"));
    assert_eq!(over.rows[1][0].as_deref(), Some("2"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_sql_limit_stays_in_the_statement_and_is_not_the_display_cap() {
    let (service, workspace, connection, path) = sqlite_service().await;
    let below = "WITH RECURSIVE s(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM s WHERE n < 5) SELECT n FROM s LIMIT 2";
    let complete = service
        .execute_query(query(&workspace, &connection, below, 100, false))
        .await
        .unwrap();
    assert_cap(&complete, 2, false, 100, 0);
    assert_eq!(complete.rows[1][0].as_deref(), Some("2"));

    let above = "WITH RECURSIVE s(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM s WHERE n < 5) SELECT n FROM s LIMIT 5";
    let capped = service
        .execute_query(query(&workspace, &connection, above, 2, false))
        .await
        .unwrap();
    assert_cap(&capped, 2, true, 2, 0);
    assert_eq!(capped.rows[0][0].as_deref(), Some("1"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_returning_writes_every_row_when_the_display_cap_keeps_one() {
    let (service, workspace, connection, path) = sqlite_service().await;
    let sql = "CREATE TABLE cap_probe(n INTEGER); INSERT INTO cap_probe(n) VALUES (1),(2),(3) RETURNING n; SELECT count(*) FROM cap_probe; SELECT n FROM cap_probe ORDER BY n;";
    let output = service
        .execute_script(script(&workspace, &connection, sql, 1, true))
        .await
        .unwrap();
    assert!(
        output
            .statements
            .iter()
            .all(|entry| entry.status == "success"),
        "{output:?}"
    );
    assert!(output.warnings.is_empty(), "{output:?}");
    let inserted = output.statements[1].result.as_ref().unwrap();
    assert_cap(inserted, 1, true, 1, 3);
    assert_eq!(inserted.rows[0][0].as_deref(), Some("1"));
    assert_eq!(
        output.statements[2].result.as_ref().unwrap().rows[0][0].as_deref(),
        Some("3")
    );
    let listed = output.statements[3].result.as_ref().unwrap();
    assert_cap(listed, 1, true, 1, 0);
    assert_eq!(listed.rows[0][0].as_deref(), Some("1"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_script_keeps_statement_results_separate() {
    let (service, workspace, connection, path) = sqlite_service().await;
    let sql = "SELECT 1 AS n; SELECT 2 AS n;";
    let output = service
        .execute_script(script(&workspace, &connection, sql, 10, false))
        .await
        .unwrap();
    assert!(output.warnings.is_empty(), "{output:?}");
    assert_eq!(output.statements.len(), 2);
    assert_cap(
        output.statements[0].result.as_ref().unwrap(),
        1,
        false,
        10,
        0,
    );
    assert_eq!(
        output.statements[0].result.as_ref().unwrap().rows[0][0].as_deref(),
        Some("1")
    );
    assert_cap(
        output.statements[1].result.as_ref().unwrap(),
        1,
        false,
        10,
        0,
    );
    assert_eq!(
        output.statements[1].result.as_ref().unwrap().rows[0][0].as_deref(),
        Some("2")
    );
    let _ = std::fs::remove_file(path);
}

async fn assert_transport_cap(
    service: &DatabaseService,
    workspace: &str,
    connection: &str,
    returning_affected: u64,
) {
    let exact_sql = "SELECT /*unfour-result rows=3*/ 1";
    let exact = service
        .execute_query(query(workspace, connection, exact_sql, 3, false))
        .await
        .unwrap();
    assert_cap(&exact, 3, false, 3, 0);
    assert_eq!(exact.rows[2][0].as_deref(), Some("3"));

    let over_sql = "SELECT /*unfour-result rows=4*/ 1";
    let over = service
        .execute_query(query(workspace, connection, over_sql, 3, false))
        .await
        .unwrap();
    assert_cap(&over, 3, true, 3, 0);

    let below_sql = "SELECT /*unfour-result rows=2*/ 1 LIMIT 2";
    let below = service
        .execute_query(query(workspace, connection, below_sql, 100, false))
        .await
        .unwrap();
    assert_cap(&below, 2, false, 100, 0);

    let above_sql = "SELECT /*unfour-result rows=5*/ 1 LIMIT 5";
    let above = service
        .execute_query(query(workspace, connection, above_sql, 2, false))
        .await
        .unwrap();
    assert_cap(&above, 2, true, 2, 0);
    assert_eq!(above.rows[0][0].as_deref(), Some("1"));

    let drained_sql = "SELECT /*unfour-result rows=5*/ 1; SELECT /*unfour-result rows=1*/ 9";
    let drained = service
        .execute_script(script(workspace, connection, drained_sql, 2, false))
        .await
        .unwrap();
    assert!(
        drained
            .statements
            .iter()
            .all(|entry| entry.status == "success"),
        "{drained:?}"
    );
    assert_cap(
        drained.statements[0].result.as_ref().unwrap(),
        2,
        true,
        2,
        0,
    );
    assert_cap(
        drained.statements[1].result.as_ref().unwrap(),
        1,
        false,
        2,
        0,
    );

    let returning_sql = "INSERT INTO demo(n) VALUES (1),(2),(3) RETURNING n /*unfour-result rows=3 affected=3*/; SELECT /*unfour-result rows=1*/ 1";
    let returning = service
        .execute_script(script(workspace, connection, returning_sql, 1, true))
        .await
        .unwrap();
    assert!(
        returning
            .statements
            .iter()
            .all(|entry| entry.status == "success"),
        "{returning:?}"
    );
    let inserted = returning.statements[0].result.as_ref().unwrap();
    // PostgreSQL reports the command tag. sqlx reports 0 for a MySQL text
    // result set because its row terminator does not carry affected rows.
    assert_cap(inserted, 1, true, 1, returning_affected);
    assert_eq!(inserted.rows[0][0].as_deref(), Some("1"));
    assert_cap(
        returning.statements[1].result.as_ref().unwrap(),
        1,
        false,
        1,
        0,
    );

    let multi_sql = "SELECT /*unfour-result rows=4 sets=2*/ 1; SELECT /*unfour-result rows=1*/ 1";
    let multi = service
        .execute_script(script(workspace, connection, multi_sql, 2, false))
        .await
        .unwrap();
    assert!(
        multi
            .statements
            .iter()
            .all(|entry| entry.status == "success"),
        "{multi:?}"
    );
    assert!(
        multi
            .warnings
            .iter()
            .any(|warning| warning == "database.batch.additionalResults"),
        "{multi:?}"
    );
    let first = multi.statements[0].result.as_ref().unwrap();
    assert_cap(first, 2, true, 2, 0);
    assert_eq!(first.rows.len(), 2);
    assert_cap(multi.statements[1].result.as_ref().unwrap(), 1, false, 2, 0);
}

#[tokio::test]
async fn postgres_and_opengauss_result_cap_uses_the_shared_stream() {
    for version in ["PostgreSQL 16.4", "PostgreSQL 9.2.4 (openGauss 5.0.0)"] {
        let server = super::profile_server::ProfileServer::start(version).await;
        let (service, workspace) = service_with_workspace().await;
        let mut input = postgres_input(&workspace);
        input.port = Some(server.port);
        let saved = service.save_connection(input).await.unwrap();
        assert_transport_cap(&service, &workspace, &saved.id, 3).await;
        let recorded = server.queries.lock().unwrap().clone();
        assert_sql_unchanged(&recorded, "rows=3*/", None);
        assert_sql_unchanged(&recorded, "LIMIT 2", Some("LIMIT 2"));
        assert_sql_unchanged(&recorded, "LIMIT 5", Some("LIMIT 5"));
        assert_sql_unchanged(&recorded, "affected=3", None);
        assert_sql_unchanged(&recorded, "sets=2", None);
    }
}

#[tokio::test]
async fn mysql_result_cap_uses_the_shared_stream() {
    let server = super::mysql_profile_server::MysqlProfileServer::start().await;
    let (service, workspace) = service_with_workspace().await;
    let mut input = mysql_input(&workspace, None);
    input.port = Some(server.port);
    let saved = service.save_connection(input).await.unwrap();
    assert_transport_cap(&service, &workspace, &saved.id, 0).await;
    let recorded = server.queries.lock().unwrap().clone();
    assert_sql_unchanged(&recorded, "rows=3*/", None);
    assert_sql_unchanged(&recorded, "LIMIT 2", Some("LIMIT 2"));
    assert_sql_unchanged(&recorded, "LIMIT 5", Some("LIMIT 5"));
    assert_sql_unchanged(&recorded, "affected=3", None);
    assert_sql_unchanged(&recorded, "sets=2", None);
}
