use super::super::*;
use super::support::{service_with_workspace, sqlite_fixture, sqlite_input};
use std::fs;
use unfour_core::models::DatabaseCellValueMode;
use unfour_core::models::{DatabaseExportContent, DatabaseExportFormat, DatabaseExportTableInput};

#[tokio::test]
async fn sqlite_large_table_export_writes_all_rows_to_disk() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .unwrap();
    let pool = sqlite_pool(&connection).await.unwrap();
    sqlx::query("WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM numbers WHERE n < 20000) INSERT INTO deploys (service, version) SELECT 'service-' || n, 'v1' FROM numbers")
        .execute(&pool).await.unwrap();
    let destination =
        std::env::temp_dir().join(format!("unfour-large-export-{}.csv", uuid::Uuid::new_v4()));
    let result = service
        .export_table(DatabaseExportTableInput {
            workspace_id,
            connection_id: connection.id,
            catalog: None,
            schema: None,
            table_name: "deploys".into(),
            content: DatabaseExportContent::Data,
            format: DatabaseExportFormat::Csv,
            destination_path: destination.to_string_lossy().into_owned(),
        })
        .await
        .unwrap();
    assert_eq!(result.row_count, 20_002);
    assert!(result.bytes_written > 100_000);
    assert_eq!(
        fs::read_to_string(&destination).unwrap().lines().count(),
        20_003
    );
    let _ = fs::remove_file(destination);
    pool.close().await;
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_export_streams_whole_table_and_empty_table() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .unwrap();
    let source_target = service
        .export_table(DatabaseExportTableInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".into(),
            content: DatabaseExportContent::Data,
            format: DatabaseExportFormat::Sql,
            destination_path: path.to_string_lossy().into_owned(),
        })
        .await;
    assert!(matches!(source_target, Err(AppError::Validation(_))));
    let pool = sqlite_pool(&connection).await.unwrap();
    sqlx::query("CREATE TABLE export_values (id INTEGER PRIMARY KEY, payload BLOB, metadata TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE INDEX export_values_metadata_idx ON export_values(metadata)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO export_values (id, payload, metadata) VALUES (1, ?1, ?2)")
        .bind(vec![0_u8, 255_u8, 10_u8])
        .bind("{\"quote\":\"O'Brien\"}")
        .execute(&pool)
        .await
        .unwrap();
    for i in 0..250 {
        sqlx::query("INSERT INTO deploys (service, version) VALUES (?1, ?2)")
            .bind(format!("service {i}"))
            .bind("a'b,\nline")
            .execute(&pool)
            .await
            .unwrap();
    }
    let mut destination = std::env::temp_dir();
    destination.push(format!("unfour-table-export-{}.sql", uuid::Uuid::new_v4()));
    let invalid = service
        .export_table(DatabaseExportTableInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".into(),
            content: DatabaseExportContent::Structure,
            format: DatabaseExportFormat::Csv,
            destination_path: destination.to_string_lossy().into_owned(),
        })
        .await;
    assert!(matches!(invalid, Err(AppError::Validation(_))));
    assert!(!destination.exists());
    let input = DatabaseExportTableInput {
        workspace_id: workspace_id.clone(),
        connection_id: connection.id.clone(),
        catalog: None,
        schema: None,
        table_name: "deploys".into(),
        content: DatabaseExportContent::StructureAndData,
        format: DatabaseExportFormat::Sql,
        destination_path: destination.to_string_lossy().into_owned(),
    };
    let result = service.export_table(input).await.unwrap();
    assert_eq!(result.row_count, 252);
    assert_eq!(
        result.bytes_written,
        fs::metadata(&destination).unwrap().len()
    );
    let sql = fs::read_to_string(&destination).unwrap();
    assert!(sql.starts_with("CREATE TABLE"));
    assert_eq!(sql.matches("INSERT INTO").count(), 3);
    assert!(sql.contains("a''b"));
    let restored = std::env::temp_dir().join(format!(
        "unfour-restored-export-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let restore_pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&restored)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::raw_sql(&sql).execute(&restore_pool).await.unwrap();
    let restored_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM deploys")
        .fetch_one(&restore_pool)
        .await
        .unwrap();
    assert_eq!(restored_count.0, 252);
    restore_pool.close().await;
    drop(restore_pool);
    let _ = fs::remove_file(restored);
    fs::remove_file(&destination).unwrap();

    destination.set_extension("sql");
    service
        .export_table(DatabaseExportTableInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "export_values".into(),
            content: DatabaseExportContent::StructureAndData,
            format: DatabaseExportFormat::Sql,
            destination_path: destination.to_string_lossy().into_owned(),
        })
        .await
        .unwrap();
    let script = fs::read_to_string(&destination).unwrap();
    assert!(script.contains("X'00ff0a'"));
    assert!(script.contains("O''Brien"));
    assert!(script.contains("CREATE INDEX export_values_metadata_idx"));
    fs::remove_file(&destination).unwrap();

    destination.set_extension("csv");
    let result = service
        .export_table(DatabaseExportTableInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "empty_deploys".into(),
            content: DatabaseExportContent::Data,
            format: DatabaseExportFormat::Csv,
            destination_path: destination.to_string_lossy().into_owned(),
        })
        .await
        .unwrap();
    assert_eq!(result.row_count, 0);
    assert!(fs::read_to_string(&destination)
        .unwrap()
        .starts_with("id,service"));
    fs::remove_file(&destination).unwrap();

    destination.set_extension("json");
    let result = service
        .export_table(DatabaseExportTableInput {
            workspace_id,
            connection_id: connection.id,
            catalog: None,
            schema: None,
            table_name: "deploys".into(),
            content: DatabaseExportContent::Data,
            format: DatabaseExportFormat::Json,
            destination_path: destination.to_string_lossy().into_owned(),
        })
        .await
        .unwrap();
    assert_eq!(result.row_count, 252);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&destination).unwrap())
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        252
    );
    fs::remove_file(&destination).unwrap();
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_schema_query_and_safe_browse_work() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");

    let test = service
        .test_connection(workspace_id.clone(), connection.id.clone())
        .await
        .expect("test connection");
    assert!(test.ok);
    assert!(test.server_version.is_some());

    let schema = service
        .schema(workspace_id.clone(), connection.id.clone(), None)
        .await
        .expect("schema");
    let deploys = schema
        .tables
        .iter()
        .find(|table| table.name == "deploys")
        .expect("deploys table");
    assert!(deploys
        .columns
        .iter()
        .any(|column| column.name == "service" && !column.primary_key));

    // SQLite is a single-file datasource and exposes no catalogs.
    let catalogs = service
        .list_catalogs(workspace_id.clone(), connection.id.clone())
        .await
        .expect("list catalogs");
    assert!(catalogs.is_empty());

    let query = service
        .execute_query(DatabaseQueryInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            sql: "select service, version from deploys order by id".to_string(),
            limit: Some(1),
            confirm_mutation: None,
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await
        .expect("query");
    assert_eq!(query.rows.len(), 1);
    assert_eq!(query.rows[0][0].as_deref(), Some("api"));
    assert_eq!(query.safety.classification, "read");
    assert!(!query.safety.requires_confirmation);

    let browse = service
        .browse_table(DatabaseBrowseInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            limit: Some(2),
            offset: Some(1),
            order_by: None,
            order_descending: false,
            filter: None,
            timeout_ms: None,
        })
        .await
        .expect("browse table");
    assert_eq!(browse.table_name, "deploys");
    assert_eq!(browse.sql, "SELECT * FROM \"deploys\" LIMIT 2 OFFSET 1");
    assert_eq!(browse.limit, 2);
    assert_eq!(browse.offset, 1);
    assert_eq!(browse.total_rows, 2);
    assert!(browse.read_only);
    assert_eq!(browse.result.rows.len(), 1);
    assert_eq!(browse.result.rows[0][1].as_deref(), Some("worker"));

    let first_page = service
        .browse_table(DatabaseBrowseInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            limit: Some(2),
            offset: None,
            order_by: None,
            order_descending: false,
            filter: None,
            timeout_ms: None,
        })
        .await
        .expect("browse first page");
    assert_eq!(first_page.result.rows.len(), 2);

    let empty = service
        .browse_table(DatabaseBrowseInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "empty_deploys".to_string(),
            limit: Some(10),
            offset: None,
            order_by: None,
            order_descending: false,
            filter: None,
            timeout_ms: None,
        })
        .await
        .expect("browse empty table");
    assert_eq!(empty.total_rows, 0);
    assert_eq!(empty.result.rows.len(), 0);
    assert_eq!(empty.result.columns.len(), 2);
    assert_eq!(empty.result.columns[1].name, "service");

    let missing = service
        .browse_table(DatabaseBrowseInput {
            workspace_id,
            connection_id: connection.id,
            catalog: None,
            schema: None,
            table_name: "missing".to_string(),
            limit: Some(10),
            offset: None,
            order_by: None,
            order_descending: false,
            filter: None,
            timeout_ms: None,
        })
        .await;
    assert!(matches!(missing, Err(AppError::NotFound(_))));
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_table_structure_exposes_columns_indexes_and_ddl() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");

    let structure = service
        .table_structure(DatabaseTableStructureInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
        })
        .await
        .expect("table structure");

    assert_eq!(structure.name, "deploys");
    assert!(structure
        .columns
        .iter()
        .any(|column| column.name == "version"));
    let ddl = structure.ddl.expect("ddl present");
    assert!(ddl.to_ascii_uppercase().contains("CREATE TABLE"));

    let missing = service
        .table_structure(DatabaseTableStructureInput {
            workspace_id,
            connection_id: connection.id,
            catalog: None,
            schema: None,
            table_name: "missing".to_string(),
        })
        .await;
    assert!(matches!(missing, Err(AppError::NotFound(_))));
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_row_mutation_inserts_updates_and_deletes_by_primary_key() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");

    let insert = service
        .mutate_table_row(DatabaseRowMutationInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            operation: "insert".to_string(),
            values: vec![
                DatabaseCellValue {
                    column: "id".to_string(),
                    mode: DatabaseCellValueMode::Value,
                    value: Some("99".to_string()),
                },
                DatabaseCellValue {
                    column: "service".to_string(),
                    mode: DatabaseCellValueMode::Value,
                    // Embedded quote must be escaped, not break the statement.
                    value: Some("ai'gent".to_string()),
                },
                DatabaseCellValue {
                    column: "version".to_string(),
                    mode: DatabaseCellValueMode::Value,
                    value: Some("9.9.9".to_string()),
                },
            ],
            primary_key: vec![],
            original_values: vec![],
            confirm_mutation: true,
        })
        .await
        .expect("insert row");
    assert_eq!(insert.affected_rows, 1);

    let update = service
        .mutate_table_row(DatabaseRowMutationInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            operation: "update".to_string(),
            values: vec![DatabaseCellValue {
                column: "version".to_string(),
                mode: DatabaseCellValueMode::Value,
                value: Some("10.0.0".to_string()),
            }],
            primary_key: vec![DatabaseCellValue {
                column: "id".to_string(),
                mode: DatabaseCellValueMode::Value,
                value: Some("99".to_string()),
            }],
            original_values: vec![],
            confirm_mutation: true,
        })
        .await
        .expect("update row");
    assert_eq!(update.affected_rows, 1);

    // Update/delete without a primary key must be rejected outright.
    let unsafe_update = service
        .mutate_table_row(DatabaseRowMutationInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            catalog: None,
            schema: None,
            table_name: "deploys".to_string(),
            operation: "update".to_string(),
            values: vec![DatabaseCellValue {
                column: "version".to_string(),
                mode: DatabaseCellValueMode::Value,
                value: Some("0".to_string()),
            }],
            primary_key: vec![],
            original_values: vec![],
            confirm_mutation: true,
        })
        .await;
    assert!(matches!(unsafe_update, Err(AppError::Validation(_))));

    let delete = service
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
                mode: DatabaseCellValueMode::Value,
                value: Some("99".to_string()),
            }],
            original_values: vec![],
            confirm_mutation: true,
        })
        .await
        .expect("delete row");
    assert_eq!(delete.affected_rows, 1);

    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_row_mutation_requires_confirmation_validates_metadata_and_detects_conflicts() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let pool = sqlx::SqlitePool::connect(path.to_string_lossy().as_ref())
        .await
        .expect("connect row mode fixture");
    sqlx::query(
        "CREATE TABLE row_modes (id INTEGER PRIMARY KEY, label TEXT DEFAULT 'generated', note TEXT)",
    )
    .execute(&pool)
    .await
    .expect("create row mode fixture");
    pool.close().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");

    let cell = |column: &str, mode: DatabaseCellValueMode, value: Option<&str>| DatabaseCellValue {
        column: column.to_string(),
        mode,
        value: value.map(str::to_string),
    };
    let base_input = || DatabaseRowMutationInput {
        workspace_id: workspace_id.clone(),
        connection_id: connection.id.clone(),
        catalog: None,
        schema: None,
        table_name: "row_modes".to_string(),
        operation: "insert".to_string(),
        values: vec![],
        primary_key: vec![],
        original_values: vec![],
        confirm_mutation: false,
    };

    let mut unconfirmed = base_input();
    unconfirmed.values = vec![cell("label", DatabaseCellValueMode::Value, Some("blocked"))];
    assert!(matches!(
        service.mutate_table_row(unconfirmed).await,
        Err(AppError::ConfirmationRequired { .. })
    ));

    let mut insert = base_input();
    insert.confirm_mutation = true;
    insert.values = vec![
        cell("id", DatabaseCellValueMode::Default, None),
        cell("label", DatabaseCellValueMode::Default, None),
        cell("note", DatabaseCellValueMode::Null, None),
    ];
    service
        .mutate_table_row(insert)
        .await
        .expect("default/null insert");

    let mut empty_insert = base_input();
    empty_insert.confirm_mutation = true;
    empty_insert.values = vec![cell("label", DatabaseCellValueMode::Value, Some(""))];
    let empty_result = service
        .mutate_table_row(empty_insert)
        .await
        .expect("empty string insert");
    assert!(empty_result.sql.contains('?'));

    let mut wrong_key = base_input();
    wrong_key.operation = "update".to_string();
    wrong_key.confirm_mutation = true;
    wrong_key.values = vec![cell("note", DatabaseCellValueMode::Value, Some("changed"))];
    wrong_key.primary_key = vec![cell(
        "label",
        DatabaseCellValueMode::Value,
        Some("generated"),
    )];
    assert!(matches!(
        service.mutate_table_row(wrong_key).await,
        Err(AppError::Validation(_))
    ));

    let mut stale = base_input();
    stale.operation = "update".to_string();
    stale.confirm_mutation = true;
    stale.values = vec![cell("note", DatabaseCellValueMode::Value, Some("changed"))];
    stale.primary_key = vec![cell("id", DatabaseCellValueMode::Value, Some("1"))];
    stale.original_values = vec![cell("label", DatabaseCellValueMode::Value, Some("stale"))];
    assert!(matches!(
        service.mutate_table_row(stale).await,
        Err(AppError::RowConflict(_))
    ));

    let payload = "safe'; DELETE FROM row_modes; --";
    let mut bound_update = base_input();
    bound_update.operation = "update".to_string();
    bound_update.confirm_mutation = true;
    bound_update.values = vec![cell("label", DatabaseCellValueMode::Value, Some(payload))];
    bound_update.primary_key = vec![cell("id", DatabaseCellValueMode::Value, Some("1"))];
    bound_update.original_values = vec![cell(
        "label",
        DatabaseCellValueMode::Value,
        Some("generated"),
    )];
    let result = service
        .mutate_table_row(bound_update)
        .await
        .expect("bound update");
    assert!(!result.sql.contains(payload));

    let pool = sqlx::SqlitePool::connect(path.to_string_lossy().as_ref())
        .await
        .expect("reconnect row mode fixture");
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM row_modes")
        .fetch_one(&pool)
        .await
        .expect("count rows");
    let first: (String, Option<String>) =
        sqlx::query_as("SELECT label, note FROM row_modes WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("load first row");
    let second: String = sqlx::query_scalar("SELECT label FROM row_modes WHERE id = 2")
        .fetch_one(&pool)
        .await
        .expect("load second row");
    assert_eq!(rows, 2);
    assert_eq!(first, (payload.to_string(), None));
    assert_eq!(second, "");
    pool.close().await;
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn mutating_sql_requires_confirmation_and_rejects_multiple_statements() {
    let (service, workspace_id) = service_with_workspace().await;
    let path = sqlite_fixture().await;
    let connection = service
        .save_connection(sqlite_input(&workspace_id, &path))
        .await
        .expect("save connection");

    let unconfirmed = service
        .execute_query(DatabaseQueryInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            sql: "update deploys set version = '1.0.2' where service = 'api'".to_string(),
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

    let confirmed = service
        .execute_query(DatabaseQueryInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            sql: "update deploys set version = '1.0.2' where service = 'api'".to_string(),
            limit: Some(100),
            confirm_mutation: Some(true),
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await
        .expect("confirmed update");
    assert_eq!(confirmed.affected_rows, 1);
    assert_eq!(confirmed.safety.classification, "mutation");
    assert!(confirmed.safety.confirmed);

    let multiple = service
        .execute_query(DatabaseQueryInput {
            workspace_id,
            connection_id: connection.id,
            sql: "select * from deploys; select * from deploys;".to_string(),
            limit: Some(100),
            confirm_mutation: Some(true),
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await;
    assert!(matches!(multiple, Err(AppError::Validation(_))));
    let _ = fs::remove_file(path);
}
