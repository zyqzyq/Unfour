use super::*;

#[tokio::test]
async fn initial_schema_uses_connection_subtypes() {
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    // Parent must no longer carry kind / config_json.
    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('connections')")
            .fetch_all(db.pool())
            .await
            .expect("list columns");
    let names: Vec<&str> = columns.iter().map(|(name,)| name.as_str()).collect();
    assert!(!names.contains(&"kind"), "kind dropped from connections");
    assert!(
        !names.contains(&"config_json"),
        "config_json dropped from connections"
    );
    assert!(
        names.contains(&"connection_type"),
        "connection_type added to connections"
    );
    assert!(names.contains(&"host"), "host added to connections");
    assert!(names.contains(&"port"), "port added to connections");
    assert!(
        names.contains(&"last_connected_at"),
        "last_connected_at added to connections"
    );
    assert!(names.contains(&"credential_ref"), "credential_ref retained");

    // Subtype tables exist with the expected shape.
    let ssh_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('ssh_connections')")
            .fetch_all(db.pool())
            .await
            .expect("list ssh_connections columns");
    let ssh_names: Vec<&str> = ssh_cols.iter().map(|(n,)| n.as_str()).collect();
    assert!(ssh_names.contains(&"connection_id"));
    assert!(ssh_names.contains(&"username"));
    assert!(ssh_names.contains(&"auth_method"));
    assert!(ssh_names.contains(&"config_json"));

    let db_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('database_connections')")
            .fetch_all(db.pool())
            .await
            .expect("list database_connections columns");
    let db_names: Vec<&str> = db_cols.iter().map(|(n,)| n.as_str()).collect();
    assert!(db_names.contains(&"connection_id"));
    assert!(db_names.contains(&"driver"));
    assert!(db_names.contains(&"database_name"));
    assert!(db_names.contains(&"username"));
    assert!(db_names.contains(&"ssl_mode"));
    assert!(db_names.contains(&"read_only"));
    assert!(db_names.contains(&"config_json"));
}

#[tokio::test]
async fn initial_schema_reads_connection_subtypes() {
    // Exercises the post-split shape end-to-end: insert a parent row plus
    // a subtype row, then read both back.
    let db = test_db().await;
    db.migrate().await.expect("run migrations");

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO workspaces (id, name, is_default, created_at, updated_at, revision, sync_status)
        VALUES ('ws-conn', 'Conn', 0, ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert workspace");

    sqlx::query(
        r#"
        INSERT INTO connections (
          id, workspace_id, connection_type, name, host, port, credential_ref,
          created_at, updated_at, revision, sync_status
        )
        VALUES ('c-ssh-2', 'ws-conn', 'ssh', 'ssh-2', 'h2', 22, NULL, ?1, ?1, 1, 'local')
        "#,
    )
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("insert new-style parent row");

    sqlx::query(
        r#"
        INSERT INTO ssh_connections (connection_id, username, auth_method, config_json)
        VALUES ('c-ssh-2', 'deploy', 'password', '{}')
        "#,
    )
    .execute(db.pool())
    .await
    .expect("insert ssh subtype row");

    let row: (String, String, String) = sqlx::query_as(
        "SELECT c.host, sub.username, sub.auth_method \
         FROM connections c \
         INNER JOIN ssh_connections sub ON sub.connection_id = c.id \
         WHERE c.id = 'c-ssh-2'",
    )
    .fetch_one(db.pool())
    .await
    .expect("read joined subtype row");
    assert_eq!(row.0, "h2");
    assert_eq!(row.1, "deploy");
    assert_eq!(row.2, "password");
}
