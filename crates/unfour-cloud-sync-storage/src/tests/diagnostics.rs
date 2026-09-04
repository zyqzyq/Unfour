use super::*;

#[tokio::test]
async fn diagnostic_context_migration_adds_safe_columns() {
    let pool = test_pool().await;
    migrate(&pool).await.expect("run merged migrations");

    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('cloud_sync_diagnostics')")
            .fetch_all(&pool)
            .await
            .expect("read diagnostic columns");
    for column in [
        "source",
        "request_id",
        "http_status",
        "phase",
        "operation_id",
        "operation_index",
    ] {
        assert!(
            columns.iter().any(|value| value == column),
            "missing diagnostic column {column}"
        );
    }
}
