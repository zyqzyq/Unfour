use super::super::*;
use super::support::{mysql_input, service_with_workspace};

#[tokio::test]
async fn saved_and_history_context_round_trip_and_legacy_default() {
    let (service, workspace) = service_with_workspace().await;
    for driver in ["mysql", "postgres"] {
        let mut input = mysql_input(&workspace, None);
        input.driver = driver.into();
        let connection = service.save_connection(input).await.unwrap();
        for catalog in [Some("analytics".to_string()), None] {
            let expected = catalog.as_deref().unwrap_or("app");
            let saved = service
                .save_sql(SavedSqlInput {
                    id: None,
                    workspace_id: workspace.clone(),
                    connection_id: Some(connection.id.clone()),
                    catalog: catalog.clone(),
                    schema: Some("audit".into()),
                    name: "query".into(),
                    sql: "select * from other_db.users".into(),
                })
                .await
                .unwrap();
            assert_eq!(saved.catalog.as_deref(), Some(expected));
            assert_eq!(saved.schema.as_deref(), Some("audit"));
            let history: DbQueryHistoryRecordInput = serde_json::from_value(serde_json::json!({
                "id": saved.id, "workspaceId": workspace, "connectionId": connection.id,
                "connectionName": "server", "catalog": catalog, "schema": "audit",
                "sql": saved.sql, "status": "success", "executedAt": "now"
            }))
            .unwrap();
            service.record_query_history(history).await.unwrap();
            let history = service
                .list_query_history(workspace.clone(), None)
                .await
                .unwrap();
            let entry = history.iter().find(|row| row.id == saved.id).unwrap();
            assert_eq!(entry.catalog, saved.catalog);
            assert_eq!(entry.schema, saved.schema);
            let updated = service
                .save_sql(SavedSqlInput {
                    id: Some(saved.id),
                    workspace_id: workspace.clone(),
                    connection_id: Some(connection.id.clone()),
                    catalog: Some("second".into()),
                    schema: None,
                    name: "updated".into(),
                    sql: "select 2".into(),
                })
                .await
                .unwrap();
            assert_eq!(updated.catalog.as_deref(), Some("second"));
            assert!(updated.schema.is_none());
        }
    }
}

#[tokio::test]
async fn unresolved_legacy_context_stays_nullable_and_old_json_is_accepted() {
    let (service, workspace) = service_with_workspace().await;
    let saved: SavedSqlInput = serde_json::from_value(serde_json::json!({
        "workspaceId": workspace, "name": "legacy", "sql": "select 1"
    }))
    .unwrap();
    let saved = service.save_sql(saved).await.unwrap();
    assert!(saved.catalog.is_none());
    assert!(saved.schema.is_none());
    assert!(service
        .resolve_saved_catalog(&workspace, Some("   "), None)
        .await
        .unwrap()
        .is_none());
    let mut input = mysql_input(&workspace, None);
    input.database = None;
    let connection = service.save_connection(input).await.unwrap();
    assert!(service
        .resolve_saved_catalog(&workspace, Some(&connection.id), None)
        .await
        .unwrap()
        .is_none());
}

#[test]
fn catalog_overrides_default_without_rewriting_sql_or_connection() {
    // openGauss is represented by the postgres driver and runtime profile.
    for driver in ["postgres", "mysql"] {
        let mut connection: DatabaseConnection = serde_json::from_value(serde_json::json!({
            "id": "conn", "workspaceId": "ws", "name": "server", "driver": driver,
            "database": "app", "readOnly": false, "createdAt": "now", "updatedAt": "now",
            "revision": 1, "syncStatus": "local"
        }))
        .unwrap();
        let effective = DatabaseService::effective_connection(&connection, Some("analytics"));
        assert_eq!(effective.database.as_deref(), Some("analytics"));
        assert_eq!(connection.database.as_deref(), Some("app"));
        connection.database = None;
        assert!(DatabaseService::effective_connection(&connection, None)
            .database
            .is_none());
    }
}

#[test]
fn mysql_use_changes_only_the_default_and_preserves_quoted_names() {
    use super::super::script_parser::mysql_use_catalog;
    assert_eq!(
        mysql_use_catalog("/* comment */ USE `other-db`;"),
        Some("other-db".into())
    );
    assert_eq!(mysql_use_catalog("use analytics"), Some("analytics".into()));
    assert_eq!(mysql_use_catalog("SELECT * FROM other_db.users"), None);
    assert_eq!(mysql_use_catalog("SELECT 'USE wrong_db'"), None);
    assert_eq!(mysql_use_catalog("-- USE wrong_db\nSELECT 1"), None);
}

#[tokio::test]
async fn mysql_script_reports_each_statement_catalog_after_successful_use() {
    // Protocol fixture accepts statements; this verifies execution bookkeeping,
    // not live MySQL SQL semantics.
    let server = super::mysql_profile_server::MysqlProfileServer::start().await;
    let (service, workspace) = service_with_workspace().await;
    let mut connection = mysql_input(&workspace, None);
    connection.port = Some(server.port);
    let connection = service.save_connection(connection).await.unwrap();
    let output = service
        .execute_script(unfour_core::models::DatabaseScriptInput {
            query: DatabaseQueryInput {
                workspace_id: workspace.clone(),
                connection_id: connection.id.clone(),
                catalog: Some("initial".into()),
                schema: None,
                sql: "USE `other-db`; SELECT 1; USE app; SELECT * FROM other_db.users;".into(),
                limit: None,
                timeout_ms: Some(2000),
                confirm_mutation: Some(true),
            },
            run_id: uuid::Uuid::new_v4().to_string(),
            cursor_offset: None,
            explain: false,
        })
        .await
        .unwrap();
    assert_eq!(output.statements.len(), 4);
    assert!(output
        .statements
        .iter()
        .all(|entry| entry.status == "success"));
    let catalogs: Vec<_> = output
        .statements
        .iter()
        .map(|entry| entry.catalog.as_deref())
        .collect();
    assert_eq!(
        catalogs,
        vec![
            Some("initial"),
            Some("other-db"),
            Some("other-db"),
            Some("app")
        ]
    );
    assert_eq!(
        service
            .get_connection(&workspace, &connection.id)
            .await
            .unwrap()
            .database
            .as_deref(),
        Some("app")
    );
}
