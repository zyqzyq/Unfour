use super::super::*;
use super::profile_server::ProfileServer;
use super::support::{postgres_input, service_with_workspace};

#[test]
fn opengauss_detection_profile_and_metadata_dispatch() {
    for banner in [
        "openGauss 5.0.0",
        "PostgreSQL 9.2.4 (openGauss 5.0.0)",
        " OPENGAUSS 6.0.0 ",
    ] {
        let profile = RuntimeDatabaseProfile::postgres(banner.into());
        assert_eq!(profile.detected_server_type, DetectedServerType::OpenGauss);
        assert_eq!(profile.dialect, DatabaseDialect::Postgres);
        let caps = &profile.capabilities;
        assert!(
            caps.catalogs
                && caps.schemas
                && caps.row_mutation
                && caps.export
                && caps.generated_columns
        );
        assert!(!caps.indexes && !caps.foreign_keys && !caps.ddl);
        let sql = postgres_columns_sql(&profile);
        assert!(sql.contains("d.adgencol"));
        assert!(sql.contains("pg_catalog.pg_constraint"));
        for unavailable in [
            "attidentity",
            "attgenerated",
            "c.is_generated",
            "c.identity_generation",
            "pg_get_expr",
            "pg_get_indexdef",
            "pg_index",
        ] {
            assert!(!sql.contains(unavailable), "{unavailable}");
        }
        assert_ne!(
            sql,
            postgres_columns_sql(&RuntimeDatabaseProfile::postgres("PostgreSQL 16.4".into()))
        );
        assert_ne!(
            sql,
            postgres_columns_sql(&RuntimeDatabaseProfile::postgres("unknown".into()))
        );
        assert!(postgres_column_auto_increment(
            DetectedServerType::OpenGauss,
            Some("AUTO_INCREMENT"),
            None,
        ));
        assert!(!postgres_column_auto_increment(
            DetectedServerType::OpenGauss,
            Some("id + 1"),
            None,
        ));
    }
}

#[tokio::test]
async fn opengauss_catalog_schema_describe_and_data_export_use_postgres_transport() {
    let server = ProfileServer::start("PostgreSQL 9.2.4 (openGauss 5.0.0; metadata fixture)").await;
    let (service, workspace) = service_with_workspace().await;
    let mut input = postgres_input(&workspace);
    input.port = Some(server.port);
    let saved = service.save_connection(input).await.unwrap();
    assert_eq!(saved.driver, "postgres");
    let key = unfour_core::domain::connection_entity_key(&workspace, &saved.id);
    let snapshot =
        serde_json::to_value(service.read_connection_domain_snapshot(&key).await.unwrap()).unwrap();
    let connection_test = service
        .test_connection(workspace.clone(), saved.id.clone())
        .await
        .unwrap();
    assert_eq!(connection_test.protocol.as_deref(), Some("postgres"));
    assert_eq!(
        connection_test.detected_server.as_deref(),
        Some("openGauss")
    );
    let catalogs = service
        .list_catalogs(workspace.clone(), saved.id.clone())
        .await
        .unwrap();
    assert!(catalogs.contains(&"qingqi_config".into()));
    let schema = service
        .schema(
            workspace.clone(),
            saved.id.clone(),
            Some("qingqi_config".into()),
        )
        .await
        .unwrap();
    assert_eq!(schema.tables[0].catalog.as_deref(), Some("qingqi_config"));
    assert_eq!(schema.tables[0].schema.as_deref(), Some("public"));
    let structure = service
        .table_structure(DatabaseTableStructureInput {
            workspace_id: workspace.clone(),
            connection_id: saved.id.clone(),
            catalog: Some("qingqi_config".into()),
            schema: Some("public".into()),
            table_name: "config_info".into(),
        })
        .await
        .unwrap();
    assert_eq!(structure.catalog.as_deref(), Some("qingqi_config"));
    assert_eq!(structure.columns.len(), 3);
    assert!(structure.columns[0].primary_key);
    assert!(structure.columns[0].auto_increment);
    assert!(!structure.columns[0].generated);
    assert_eq!(
        structure.columns[0].default_value.as_deref(),
        Some("AUTO_INCREMENT")
    );
    assert!(!structure.columns[1].auto_increment && !structure.columns[1].generated);
    assert!(structure.columns[2].generated);
    assert!(!structure.columns[2].auto_increment);
    assert!(
        structure.indexes.is_empty()
            && structure.foreign_keys.is_empty()
            && structure.ddl.is_none()
    );
    export_selected(&service, &workspace, &saved.id).await;
    query_and_mutate(&service, &workspace, &saved.id).await;
    assert_eq!(
        snapshot,
        serde_json::to_value(service.read_connection_domain_snapshot(&key).await.unwrap()).unwrap()
    );
    let catalogs = server.catalogs.lock().unwrap();
    assert_eq!(catalogs[0], "testdb");
    assert!(catalogs.iter().any(|catalog| catalog == "qingqi_config"));
    assert!(server
        .query_catalogs
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, sql)| sql != "SELECT version()" && !sql.contains("FROM pg_database"))
        .all(|(catalog, _)| catalog == "qingqi_config"));
    let queries = server.queries.lock().unwrap();
    assert!(queries.iter().any(|sql| sql
        .starts_with("SELECT \"content\" FROM \"public\".\"config_info\" WHERE")
        && sql.contains("$1")
        && sql.ends_with("LIMIT 1")));
    assert!(!queries.iter().any(|sql| sql.contains("pg_index")
        || sql.contains("attidentity")
        || sql.contains("WITH ORDINALITY")));
}

async fn query_and_mutate(service: &DatabaseService, workspace: &str, connection: &str) {
    for sql in [
        "SELECT \"content\" FROM \"public\".\"config_info\"",
        "UPDATE public.config_info SET content = 'changed' WHERE id = 2",
    ] {
        let result = service
            .execute_query(DatabaseQueryInput {
                workspace_id: workspace.into(),
                connection_id: connection.into(),
                sql: sql.into(),
                catalog: Some("qingqi_config".into()),
                schema: Some("public".into()),
                limit: Some(1),
                confirm_mutation: Some(true),
                timeout_ms: Some(2000),
            })
            .await
            .unwrap();
        if sql.starts_with("SELECT") {
            assert_eq!(result.rows.len(), 1);
        } else {
            assert_eq!(result.affected_rows, 1);
        }
    }
    let cell = |column: &str, value: &str| DatabaseCellValue {
        column: column.into(),
        value: Some(value.into()),
        mode: Default::default(),
    };
    for operation in ["insert", "update", "delete"] {
        let result = service
            .mutate_table_row(DatabaseRowMutationInput {
                workspace_id: workspace.into(),
                connection_id: connection.into(),
                catalog: Some("qingqi_config".into()),
                schema: Some("public".into()),
                table_name: "config_info".into(),
                operation: operation.into(),
                values: if operation == "delete" {
                    vec![]
                } else {
                    vec![cell("content", "changed")]
                },
                primary_key: if operation == "insert" {
                    vec![]
                } else {
                    vec![cell("id", "2")]
                },
                original_values: vec![],
                confirm_mutation: true,
            })
            .await
            .unwrap();
        assert_eq!(result.affected_rows, 1);
        assert!(result.sql.contains("\"public\".\"config_info\""));
        assert!(!result.sql.contains("changed"));
    }
}

#[tokio::test]
async fn postgres_data_only_export_also_skips_optional_metadata() {
    let server = ProfileServer::start("PostgreSQL 16.4 (metadata fixture)").await;
    let (service, workspace) = service_with_workspace().await;
    let mut input = postgres_input(&workspace);
    input.port = Some(server.port);
    let saved = service.save_connection(input).await.unwrap();
    export_selected(&service, &workspace, &saved.id).await;
    assert!(!server
        .queries
        .lock()
        .unwrap()
        .iter()
        .any(|sql| sql.contains("pg_index") || sql.contains("attidentity")));
}

async fn export_selected(service: &DatabaseService, workspace: &str, connection: &str) {
    let path = std::env::temp_dir().join(format!("unfour-opengauss-{}.csv", uuid::Uuid::new_v4()));
    let result = service
        .export_table(DatabaseExportTableInput {
            workspace_id: workspace.into(),
            connection_id: connection.into(),
            catalog: Some("qingqi_config".into()),
            schema: Some("public".into()),
            table_name: "config_info".into(),
            content: DatabaseExportContent::Data,
            format: DatabaseExportFormat::Csv,
            destination_path: path.to_string_lossy().into_owned(),
            columns: Some(vec!["content".into()]),
            filters: vec![DatabaseExportFilter {
                column: "id".into(),
                op: DatabaseExportFilterOp::Eq,
                values: vec![Some("2".into())],
            }],
            limit: Some(1),
        })
        .await
        .unwrap();
    assert_eq!(result.row_count, 1);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "content\r\nselected content\r\n"
    );
    std::fs::remove_file(path).unwrap();
}
