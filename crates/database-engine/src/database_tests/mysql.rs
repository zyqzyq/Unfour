use super::super::*;
use super::support::{mysql_input, service_with_workspace};

// -----------------------------------------------------------------------
// MySQL-specific tests
// -----------------------------------------------------------------------

#[test]
fn mysql_config_maps_host_port_database_username() {
    let input = mysql_input("ws", Some("unfour:ws:database-password:abc".to_string()));
    let storage = input_to_storage(&input).expect("storage");

    assert_eq!(storage.driver, "mysql");
    assert_eq!(storage.host.as_deref(), Some("127.0.0.1"));
    assert_eq!(storage.port, Some(9));
    assert_eq!(storage.database_name.as_deref(), Some("app"));
    assert_eq!(storage.username.as_deref(), Some("testuser"));
    assert!(storage.config.sqlite_path.is_none());
}

#[tokio::test]
async fn mysql_password_loads_from_secret_store_and_is_not_persisted() {
    let (service, workspace_id) = service_with_workspace().await;
    let secret = "mysql-super-secret";
    let credential = service
        .secret_store
        .as_ref()
        .expect("secret store")
        .create_credential(
            workspace_id.clone(),
            "database-password".to_string(),
            "MySQL password".to_string(),
            secret.to_string(),
        )
        .await
        .expect("create credential");
    let connection = service
        .save_connection(mysql_input(
            &workspace_id,
            Some(credential.credential_ref.clone()),
        ))
        .await
        .expect("save mysql connection");

    let password = resolve_database_password(&connection, service.secret_store.as_ref())
        .await
        .expect("resolve mysql password");
    assert_eq!(password.as_deref(), Some(secret));

    let stored: (String, Option<String>) = sqlx::query_as(
        "SELECT sub.config_json, c.credential_ref FROM connections c \
         INNER JOIN database_connections sub ON sub.connection_id = c.id \
         WHERE c.id = ?1",
    )
    .bind(&connection.id)
    .fetch_one(service.db.pool())
    .await
    .expect("load persisted connection");
    assert!(!stored.0.contains(secret));
    assert_eq!(
        stored.1.as_deref(),
        Some(credential.credential_ref.as_str())
    );
}

#[test]
fn mysql_schema_metadata_maps_database_table_and_columns() {
    let table = mysql_table_from_metadata(
        "analytics".to_string(),
        "events".to_string(),
        "BASE TABLE".to_string(),
        vec![DatabaseTableColumn {
            name: "id".to_string(),
            data_type: "bigint unsigned".to_string(),
            nullable: false,
            primary_key: true,
            default_value: None,
        }],
    );

    assert_eq!(table.catalog.as_deref(), Some("analytics"));
    assert!(table.schema.is_none());
    assert_eq!(table.name, "events");
    assert_eq!(table.kind, "table");
    assert!(table.columns[0].primary_key);
}

#[test]
fn clean_identifier_trims_rejects_control_chars_and_passes_empty_as_none() {
    assert_eq!(clean_identifier(None).expect("none"), None);
    assert_eq!(clean_identifier(Some("   ")).expect("blank"), None);
    assert_eq!(
        clean_identifier(Some("  app_data  ")).expect("trim"),
        Some("app_data")
    );
    // A quote is allowed here because callers quote/escape the identifier;
    // control characters are rejected as defense in depth.
    assert!(clean_identifier(Some("bad\nname")).is_err());
    assert!(clean_identifier(Some("bad\0name")).is_err());
}

#[test]
fn mysql_table_browse_sql_is_qualified_paginated_and_escaped() {
    assert_eq!(
        mysql_browse_sql("app`data", "user`events", "", "", 50, 100),
        "SELECT * FROM `app``data`.`user``events` LIMIT 50 OFFSET 100"
    );
}

#[test]
fn mysql_connection_string_passwords_are_redacted() {
    // A connection string with an embedded password leaked into the error keeps
    // non-sensitive fields (user) but scrubs the password value, instead of
    // wiping the whole message.
    let error = sanitize_mysql_error(sqlx::Error::Protocol(
        "mysql://testuser:mysql-super-secret@db.internal:3306/app".to_string(),
    ));
    let message = error.to_string();

    assert!(message.contains("testuser"));
    assert!(!message.contains("mysql-super-secret"));
    assert!(message.contains("<redacted>"));
    assert!(!message.contains("details redacted"));
}

#[test]
fn mysql_access_denied_message_is_preserved() {
    // Benign, useful diagnostics are no longer overwritten with a generic
    // redacted placeholder.
    let error = sanitize_mysql_error(sqlx::Error::Protocol(
        "Access denied for user 'testuser'@'localhost' (using password: YES)".to_string(),
    ));
    let message = error.to_string();

    assert!(message.contains(
        "Access denied for user 'testuser'@'localhost' (using password: YES)"
    ));
    assert!(!message.contains("<redacted>"));
    assert!(!message.contains("details redacted"));
}

#[tokio::test]
async fn mysql_test_connection_and_read_query_use_live_path_with_sanitized_errors() {
    let (service, workspace_id) = service_with_workspace().await;
    let credential = service
        .secret_store
        .as_ref()
        .expect("secret store")
        .create_credential(
            workspace_id.clone(),
            "database-password".to_string(),
            "MySQL password".to_string(),
            "mysql-super-secret".to_string(),
        )
        .await
        .expect("create credential");
    let connection = service
        .save_connection(mysql_input(&workspace_id, Some(credential.credential_ref)))
        .await
        .expect("save mysql connection");

    let test_error = service
        .test_connection(workspace_id.clone(), connection.id.clone())
        .await
        .expect_err("closed local port should reject MySQL connection")
        .to_string();
    assert!(!test_error.contains("mysql-super-secret"));

    let query_error = service
        .execute_query(DatabaseQueryInput {
            workspace_id,
            connection_id: connection.id,
            sql: "SELECT id FROM users".to_string(),
            limit: Some(25),
            confirm_mutation: None,
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await
        .expect_err("closed local port should reject MySQL query");
    assert!(!matches!(
        query_error,
        AppError::ConfirmationRequired { .. }
    ));
    assert!(!query_error.to_string().contains("mysql-super-secret"));
}

#[tokio::test]
async fn mysql_mutating_query_requires_confirmation_before_connecting() {
    let (service, workspace_id) = service_with_workspace().await;
    let connection = service
        .save_connection(mysql_input(&workspace_id, None))
        .await
        .expect("save mysql connection");

    let result = service
        .execute_query(DatabaseQueryInput {
            workspace_id,
            connection_id: connection.id,
            sql: "UPDATE users SET active = false".to_string(),
            limit: Some(100),
            confirm_mutation: None,
            catalog: None,
            schema: None,
            timeout_ms: None,
        })
        .await;
    assert!(matches!(result, Err(AppError::ConfirmationRequired { .. })));
}
