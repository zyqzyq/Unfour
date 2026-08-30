use super::super::*;
use super::support::{postgres_input, service_with_workspace, RejectingServer};

// -----------------------------------------------------------------------
// PostgreSQL-specific tests
// -----------------------------------------------------------------------

#[test]
fn postgres_config_maps_host_port_database_username() {
    let input = DatabaseConnectionInput {
        id: None,
        workspace_id: "ws".to_string(),
        name: "My PG".to_string(),
        driver: "postgres".to_string(),
        host: Some("pg.example.com".to_string()),
        port: Some(5433),
        database: Some("mydb".to_string()),
        username: Some("admin".to_string()),
        ssl_mode: None,
        sqlite_path: None,
        credential_ref: Some("unfour:ws:database-password:abc".to_string()),
        read_only: false,
    };

    let storage = input_to_storage(&input).expect("storage");
    assert_eq!(storage.driver, "postgres");
    assert_eq!(storage.host.as_deref(), Some("pg.example.com"));
    assert_eq!(storage.port, Some(5433));
    assert_eq!(storage.database_name.as_deref(), Some("mydb"));
    assert_eq!(storage.username.as_deref(), Some("admin"));
    assert!(storage.config.sqlite_path.is_none());
}

#[test]
fn postgres_table_browse_sql_is_schema_qualified_and_escaped() {
    assert_eq!(
        postgres_browse_sql("app\"data", "user\"events", "", "", 50, 100),
        "SELECT * FROM \"app\"\"data\".\"user\"\"events\" LIMIT 50 OFFSET 100"
    );
}

#[test]
fn postgres_schema_metadata_preserves_schema_name() {
    let table = postgres_table_from_metadata(
        Some("appdb".to_string()),
        "app_data".to_string(),
        "users".to_string(),
        "table".to_string(),
        vec![DatabaseTableColumn {
            name: "id".to_string(),
            data_type: "bigint".to_string(),
            nullable: false,
            primary_key: true,
            default_value: None,
            generated: false,
            auto_increment: false,
        }],
    );

    assert_eq!(table.catalog.as_deref(), Some("appdb"));
    assert_eq!(table.schema.as_deref(), Some("app_data"));
    assert_eq!(table.name, "users");
    assert!(table.columns[0].primary_key);
}

#[tokio::test]
async fn postgres_password_not_leaked_in_connection_error() {
    let (service, workspace_id) = service_with_workspace().await;
    let server = RejectingServer::start().await;
    let credential = service
        .secret_store
        .as_ref()
        .unwrap()
        .create_credential(
            workspace_id.clone(),
            "database-password".into(),
            "PG test".into(),
            "pg-secret-canary".into(),
        )
        .await
        .unwrap();

    // The controlled endpoint fails the handshake without contacting a real DB.
    let connection = service
        .save_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: workspace_id.clone(),
            name: "Bad PG".to_string(),
            driver: "postgres".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(server.port),
            database: Some("testdb".to_string()),
            username: Some("testuser".to_string()),
            ssl_mode: None,
            sqlite_path: None,
            credential_ref: Some(credential.credential_ref),
            read_only: false,
        })
        .await
        .expect("save pg connection");

    // test_connection should fail (no server), but should not panic
    let result = service.test_connection(workspace_id, connection.id).await;
    assert!(result.is_err());
    // The error message should not contain the username or host
    let err_msg = format!("{}", result.unwrap_err());
    assert!(!err_msg.contains("pg-secret-canary"));
    assert!(
        !err_msg.contains("testuser"),
        "error should not leak username: {}",
        err_msg
    );
}

#[tokio::test]
async fn postgres_connection_without_credential_ref_uses_no_password() {
    let connection = DatabaseConnection {
        id: "pg1".to_string(),
        workspace_id: "ws1".to_string(),
        name: "No PW".to_string(),
        driver: "postgres".to_string(),
        host: Some("localhost".to_string()),
        port: Some(5432),
        database: Some("testdb".to_string()),
        username: Some("dev".to_string()),
        ssl_mode: None,
        sqlite_path: None,
        credential_ref: None,
        read_only: false,
        created_at: String::new(),
        updated_at: String::new(),
        deleted_at: None,
        revision: 1,
        sync_status: "local".to_string(),
        remote_id: None,
    };

    // No credential_ref → password resolves to None (passwordless / peer auth)
    let password = resolve_database_password(&connection, None)
        .await
        .expect("resolve password without credential_ref");
    assert!(
        password.is_none(),
        "expected no password when credential_ref is None"
    );

    // Empty-string credential_ref should also resolve to None
    let mut empty_ref = connection.clone();
    empty_ref.credential_ref = Some("".to_string());
    let password = resolve_database_password(&empty_ref, None)
        .await
        .expect("resolve password with empty credential_ref");
    assert!(
        password.is_none(),
        "expected no password when credential_ref is empty"
    );

    // Whitespace-only credential_ref should also resolve to None
    let mut ws_ref = connection.clone();
    ws_ref.credential_ref = Some("   ".to_string());
    let password = resolve_database_password(&ws_ref, None)
        .await
        .expect("resolve password with whitespace credential_ref");
    assert!(
        password.is_none(),
        "expected no password when credential_ref is whitespace"
    );
}

#[tokio::test]
async fn postgres_connection_with_credential_ref_loads_password() {
    let secret_store = SecretStore::in_memory("unfour-test");
    let credential_ref = secret_store
        .create_credential(
            "ws1".to_string(),
            "database-password".to_string(),
            "PG password".to_string(),
            "s3cret!".to_string(),
        )
        .await
        .expect("create credential");

    let connection = DatabaseConnection {
        id: "pg2".to_string(),
        workspace_id: "ws1".to_string(),
        name: "With PW".to_string(),
        driver: "postgres".to_string(),
        host: Some("localhost".to_string()),
        port: Some(5432),
        database: Some("testdb".to_string()),
        username: Some("dev".to_string()),
        ssl_mode: None,
        sqlite_path: None,
        credential_ref: Some(credential_ref.credential_ref),
        read_only: false,
        created_at: String::new(),
        updated_at: String::new(),
        deleted_at: None,
        revision: 1,
        sync_status: "local".to_string(),
        remote_id: None,
    };

    let password = resolve_database_password(&connection, Some(&secret_store))
        .await
        .expect("resolve password with valid credential_ref");
    assert_eq!(password, Some("s3cret!".to_string()));
}

#[tokio::test]
async fn postgres_mutating_query_requires_confirmation() {
    let (service, workspace_id) = service_with_workspace().await;

    // Save a PostgreSQL connection (no live server needed for this test —
    // the confirmation check happens before the connection is opened)
    let connection = service
        .save_connection(postgres_input(&workspace_id))
        .await
        .expect("save pg connection");

    let unconfirmed = service
        .execute_query(DatabaseQueryInput {
            workspace_id: workspace_id.clone(),
            connection_id: connection.id.clone(),
            sql: "UPDATE users SET active = false".to_string(),
            limit: Some(100),
            confirm_mutation: None,
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await;
    assert!(
        matches!(unconfirmed, Err(AppError::ConfirmationRequired { .. })),
        "PostgreSQL mutations should require confirmation"
    );
}

#[tokio::test]
async fn postgres_metadata_can_be_saved_and_listed() {
    let (service, workspace_id) = service_with_workspace().await;

    let saved = service
        .save_connection(postgres_input(&workspace_id))
        .await
        .expect("save pg connection");
    assert_eq!(saved.driver, "postgres");
    assert_eq!(saved.name, "PG test");
    assert_eq!(saved.host.as_deref(), Some("127.0.0.1"));
    assert_eq!(saved.port, Some(5432));

    let listed = service
        .list_connections(workspace_id)
        .await
        .expect("list connections");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].driver, "postgres");
}

#[tokio::test]
async fn postgres_schema_fails_without_live_server() {
    let (service, workspace_id) = service_with_workspace().await;
    let server = RejectingServer::start().await;
    let mut input = postgres_input(&workspace_id);
    input.port = Some(server.port);
    let connection = service
        .save_connection(input)
        .await
        .expect("save pg connection");

    let result = service.schema(workspace_id, connection.id, None).await;
    // Should fail because there's no live PostgreSQL server
    assert!(result.is_err());
}
