use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_command_bus::CommandBus;
use unfour_core::models::{DatabaseConnectionInput, DatabaseQueryInput, DatabaseScriptInput};
use unfour_local_storage::LocalDb;

#[tokio::test]
async fn script_activity_failure_does_not_hide_executed_mutations() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();
    let db = LocalDb::from_pool(pool);
    db.migrate().await.unwrap();
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let path = std::env::temp_dir().join(format!(
        "unfour-script-{}.sqlite",
        unfour_core::id::new_id()
    ));
    let fixture = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    fixture.close().await;
    let connection = bus
        .save_database_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: workspace_id.clone(),
            name: "Script test".into(),
            driver: "sqlite".into(),
            sqlite_path: Some(path.to_string_lossy().into_owned()),
            host: None,
            port: None,
            database: None,
            username: None,
            ssl_mode: None,
            credential_ref: None,
            read_only: false,
        })
        .await
        .unwrap();
    // Only the local activity sink fails; SQL runs against a separate test DB.
    sqlx::query("DROP TABLE activity_events")
        .execute(db.pool())
        .await
        .unwrap();
    let request = DatabaseScriptInput {
        query: DatabaseQueryInput {
            workspace_id,
            connection_id: connection.id,
            sql: "CREATE TABLE repeated(n INT); SELECT 1;".into(),
            limit: Some(10),
            confirm_mutation: Some(true),
            catalog: None,
            schema: None,
            timeout_ms: None,
        },
        run_id: unfour_core::id::new_id(),
        cursor_offset: None,
        explain: false,
    };
    let result = bus.execute_database_script(request.clone()).await.unwrap();
    assert!(result.statements.iter().all(|s| s.status == "success"));
    assert_eq!(result.warnings, ["database.batch.activityFailed"]);
    let repeated = bus.execute_database_script(request).await.unwrap();
    assert_eq!(repeated.statements[0].status, "failed");
    assert_eq!(repeated.statements[1].status, "skipped");
    let _ = std::fs::remove_file(path);
}
