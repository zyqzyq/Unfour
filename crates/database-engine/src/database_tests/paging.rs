use super::super::*;
use super::support::{service_with_workspace, sqlite_fixture, sqlite_input};

fn input(workspace: &str, connection: &str, table: &str, offset: u32) -> DatabaseBrowseInput {
    DatabaseBrowseInput {
        workspace_id: workspace.into(),
        connection_id: connection.into(),
        table_name: table.into(),
        catalog: None,
        schema: None,
        limit: Some(1),
        offset: Some(offset),
        order_by: None,
        order_descending: false,
        filter: None,
        timeout_ms: Some(5000),
        include_total: Some(false),
    }
}

#[tokio::test]
async fn lookahead_paging_handles_last_empty_and_filtered_pages() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let first = service
        .browse_table(input(&workspace, &connection.id, "deploys", 0))
        .await
        .unwrap();
    assert_eq!(first.result.rows.len(), 1);
    assert!(first.has_more);
    assert!(!first.total_rows_exact);
    assert!(first.sql.contains("LIMIT 2"));
    let last = service
        .browse_table(input(&workspace, &connection.id, "deploys", 1))
        .await
        .unwrap();
    assert_eq!(last.result.rows.len(), 1);
    assert!(!last.has_more);
    assert!(last.total_rows_exact);
    assert_eq!(last.total_rows, 2);
    let mut empty = input(&workspace, &connection.id, "deploys", 0);
    empty.filter = Some("definitely-no-such-service".into());
    let empty = service.browse_table(empty).await.unwrap();
    assert!(empty.result.rows.is_empty());
    assert!(empty.total_rows_exact);
    assert_eq!(empty.total_rows, 0);
    let beyond = service
        .browse_table(input(&workspace, &connection.id, "deploys", 99))
        .await
        .unwrap();
    assert!(beyond.result.rows.is_empty());
    assert!(!beyond.has_more);
    assert!(!beyond.total_rows_exact);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn fast_paging_does_not_evaluate_the_full_table_count() {
    let (service, workspace) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace, &path))
        .await
        .unwrap();
    let pool = sqlite_pool(&connection).await.unwrap();
    // Reading the first two rows is valid; scanning the rest raises an error.
    sqlx::raw_sql("CREATE TABLE count_probe(id INTEGER PRIMARY KEY); INSERT INTO count_probe VALUES (1),(2),(3),(4); CREATE VIEW count_trap AS SELECT id FROM count_probe WHERE CASE WHEN id <= 2 THEN 1 ELSE json('not-json') END IS NOT NULL;")
        .execute(&*pool).await.unwrap();
    pool.close().await;
    let fast = service
        .browse_table(input(&workspace, &connection.id, "count_trap", 0))
        .await
        .unwrap();
    assert_eq!(fast.result.rows.len(), 1);
    assert!(fast.has_more);
    let mut exact = input(&workspace, &connection.id, "count_trap", 0);
    exact.include_total = None; // Existing clients retain exact-count behavior.
    assert!(service.browse_table(exact).await.is_err());
    let _ = std::fs::remove_file(path);
}
