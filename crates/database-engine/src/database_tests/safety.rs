use super::super::*;
use super::support::{service_with_workspace, sqlite_fixture, sqlite_input};
use std::fs;

#[test]
fn classify_query_flags_writes_hidden_behind_explain_and_with() {
    // Plain reads stay no-confirmation, including read-only EXPLAIN ANALYZE.
    assert!(!classify_query("SELECT * FROM users").requires_confirmation);
    assert!(!classify_query("EXPLAIN SELECT * FROM users").requires_confirmation);
    assert!(!classify_query("EXPLAIN ANALYZE SELECT * FROM users").requires_confirmation);
    assert!(!classify_query("WITH c AS (SELECT 1) SELECT * FROM c").requires_confirmation);

    // EXPLAIN ANALYZE <write> executes the write in PostgreSQL.
    let explain_write = classify_query("EXPLAIN ANALYZE DELETE FROM users");
    assert!(explain_write.requires_confirmation);
    assert_eq!(explain_write.classification, "mutation");

    // Data-modifying CTEs execute in PostgreSQL.
    let write_cte = classify_query("WITH d AS (DELETE FROM users RETURNING *) SELECT * FROM d");
    assert!(write_cte.requires_confirmation);
    assert_eq!(write_cte.classification, "mutation");

    // A schema change wrapped in EXPLAIN is flagged as schema-change.
    let explain_ddl = classify_query("EXPLAIN CREATE TABLE t (id INT)");
    assert!(explain_ddl.requires_confirmation);
    assert_eq!(explain_ddl.classification, "schema-change");

    // A column whose name merely contains a keyword is not a false positive.
    assert!(!classify_query("SELECT updated_at, created_at FROM users").requires_confirmation);
}

#[test]
fn classify_query_flags_select_into_as_write() {
    // PostgreSQL `SELECT INTO table` creates a new table (schema change).
    let pg = classify_query("SELECT * INTO new_table FROM users");
    assert!(pg.requires_confirmation);
    assert_eq!(pg.classification, "schema-change");

    // MySQL `SELECT ... INTO OUTFILE` writes to the server filesystem.
    let outfile = classify_query("SELECT * INTO OUTFILE '/tmp/out.csv' FROM users");
    assert!(outfile.requires_confirmation);
    assert_eq!(outfile.classification, "mutation");

    // MySQL `SELECT ... INTO DUMPFILE` writes a single row to the server filesystem.
    let dumpfile = classify_query("SELECT * INTO DUMPFILE '/tmp/out.txt' FROM users");
    assert!(dumpfile.requires_confirmation);
    assert_eq!(dumpfile.classification, "mutation");

    // MySQL `SELECT ... INTO @var` is a harmless session assignment, still a read.
    assert!(!classify_query("SELECT col INTO @var FROM users").requires_confirmation);

    // Wrapped variants are caught too (the leading keyword is not `select`).
    let wrapped_explain = classify_query("EXPLAIN ANALYZE SELECT * INTO new_table FROM users");
    assert!(wrapped_explain.requires_confirmation);
    assert_eq!(wrapped_explain.classification, "schema-change");

    let wrapped_cte =
        classify_query("WITH t AS (SELECT * INTO new_table FROM users) SELECT * FROM t");
    assert!(wrapped_cte.requires_confirmation);
    assert_eq!(wrapped_cte.classification, "schema-change");
}

#[tokio::test]
async fn explain_analyze_write_requires_confirmation_before_execution() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");

    // The classification gate runs before the connection is opened, so the
    // wrapped write is rejected without confirmation regardless of driver.
    let unconfirmed = service
        .execute_query(DatabaseQueryInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            sql: "EXPLAIN ANALYZE DELETE FROM deploys".to_string(),
            limit: Some(100),
            confirm_mutation: None,
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await;
    assert!(matches!(
        unconfirmed,
        Err(AppError::ConfirmationRequired { .. })
    ));
    let _ = fs::remove_file(path);
}

#[test]
fn resolve_timeout_applies_default_and_clamps() {
    assert_eq!(
        resolve_timeout(None),
        Duration::from_millis(DEFAULT_QUERY_TIMEOUT_MS)
    );
    assert_eq!(
        resolve_timeout(Some(0)),
        Duration::from_millis(MIN_QUERY_TIMEOUT_MS)
    );
    assert_eq!(resolve_timeout(Some(5_000)), Duration::from_millis(5_000));
    assert_eq!(
        resolve_timeout(Some(10_000_000)),
        Duration::from_millis(MAX_QUERY_TIMEOUT_MS)
    );
}

#[tokio::test]
async fn browse_table_pushes_sort_and_filter_into_the_query() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");

    // Sort descending by service: 'worker' comes before 'api', and the SQL
    // carries the ORDER BY so it orders the whole table, not just the page.
    let sorted = service
        .browse_table(DatabaseBrowseInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            limit: Some(100),
            offset: None,
            order_by: Some("service".to_string()),
            order_descending: true,
            filter: None,
            timeout_ms: None,
        })
        .await
        .expect("sorted browse");
    assert!(sorted.sql.contains("ORDER BY \"service\" DESC"));
    assert_eq!(sorted.result.rows[0][1].as_deref(), Some("worker"));
    assert_eq!(sorted.result.rows[1][1].as_deref(), Some("api"));

    // Filter narrows both the rows and the total count.
    let filtered = service
        .browse_table(DatabaseBrowseInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            limit: Some(100),
            offset: None,
            order_by: None,
            order_descending: false,
            filter: Some("worker".to_string()),
            timeout_ms: None,
        })
        .await
        .expect("filtered browse");
    assert!(filtered.sql.contains("WHERE"));
    assert_eq!(filtered.total_rows, 1);
    assert_eq!(filtered.result.rows.len(), 1);
    assert_eq!(filtered.result.rows[0][1].as_deref(), Some("worker"));

    // An unknown sort column is rejected rather than silently ignored.
    let bad_sort = service
        .browse_table(DatabaseBrowseInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            limit: Some(100),
            offset: None,
            order_by: Some("not_a_column".to_string()),
            order_descending: false,
            filter: None,
            timeout_ms: None,
        })
        .await;
    assert!(matches!(bad_sort, Err(AppError::Validation(_))));

    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn read_only_connection_blocks_writes_and_row_edits() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(DatabaseConnectionInput {
            read_only: true,
            ..sqlite_input(&workspace_id, &path)
        })
        .await
        .expect("save read-only connection");
    assert!(connection.read_only);

    // Reads still work on a read-only connection.
    let read = service
        .execute_query(DatabaseQueryInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            sql: "SELECT * FROM deploys".to_string(),
            limit: Some(10),
            confirm_mutation: None,
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await
        .expect("read query allowed");
    assert!(!read.rows.is_empty());

    // A confirmed mutation is still blocked: read-only overrides confirmation.
    let write = service
        .execute_query(DatabaseQueryInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            sql: "DELETE FROM deploys".to_string(),
            limit: Some(10),
            confirm_mutation: Some(true),
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await;
    assert!(matches!(write, Err(AppError::ReadOnly(_))));

    // Inline row edits are blocked too.
    let row = service
        .mutate_table_row(DatabaseRowMutationInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            operation: "delete".to_string(),
            values: vec![],
            primary_key: vec![DatabaseCellValue {
                column: "id".to_string(),
                value: Some("1".to_string()),
            }],
        })
        .await;
    assert!(matches!(row, Err(AppError::ReadOnly(_))));

    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn read_only_connection_blocks_select_into() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(DatabaseConnectionInput {
            read_only: true,
            ..sqlite_input(&workspace_id, &path)
        })
        .await
        .expect("save read-only connection");
    assert!(connection.read_only);

    // `SELECT ... INTO` is classified as a schema change / mutation, so a
    // read-only connection must reject it before the driver is ever asked to
    // run it. Confirmation cannot override the read-only gate.
    let select_into = service
        .execute_query(DatabaseQueryInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            sql: "SELECT * INTO exported FROM deploys".to_string(),
            limit: Some(10),
            confirm_mutation: Some(true),
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await;
    assert!(matches!(select_into, Err(AppError::ReadOnly(_))));

    let _ = fs::remove_file(path);
}
