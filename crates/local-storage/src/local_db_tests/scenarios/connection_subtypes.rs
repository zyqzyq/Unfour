use super::*;

#[tokio::test]
async fn redis_upgrade_preserves_live_foreign_keys_and_refreshes_database_connections() {
    let db = test_db().await;
    let mut old = core_migrator();
    old.migrations = std::borrow::Cow::Owned(
        old.iter()
            .filter(|m| m.version < 20261113030000)
            .cloned()
            .collect(),
    );
    old.run(db.pool()).await.unwrap();
    seed_workspace(&db, "w").await;
    seed_workspace(&db, "other").await;
    sqlx::raw_sql("
        INSERT INTO connections (id,workspace_id,connection_type,name,created_at,updated_at)
          VALUES ('s','w','ssh','shell','now','now'),('d','w','database','sql','now','now');
        INSERT INTO ssh_connections (connection_id,username,auth_method) VALUES ('s','alice','none');
        INSERT INTO database_connections (connection_id,driver) VALUES ('d','postgres');
        INSERT INTO saved_sql (id,workspace_id,connection_id,name,sql,created_at,updated_at)
          VALUES ('q','w','d','preserve','SELECT 1','now','now');")
        .execute(db.pool()).await.unwrap();
    db.migrate().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT connection_id FROM saved_sql WHERE id='q'")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "d"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT username FROM ssh_connections WHERE connection_id='s'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "alice"
    );
    sqlx::raw_sql("
        INSERT INTO connections (id,workspace_id,connection_type,name,created_at,updated_at)
          VALUES ('r','w','redis','cache','now','now');
        INSERT INTO redis_connections (connection_id,workspace_id,db_index,tls) VALUES ('r','w',3,0);")
        .execute(db.pool()).await.unwrap();
    assert!(sqlx::query(
        "UPDATE redis_connections SET workspace_id='other' WHERE connection_id='r'"
    )
    .execute(db.pool())
    .await
    .is_err());
    assert!(sqlx::query("INSERT INTO redis_connections (connection_id,workspace_id,db_index,tls) VALUES ('d','w',0,0)").execute(db.pool()).await.is_err());
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(db.pool())
        .await
        .unwrap()
        .is_empty());
    let domain: Option<String> = sqlx::query_scalar(
        "SELECT domain FROM local_data_versions WHERE domain='redis-connections'",
    )
    .fetch_optional(db.pool())
    .await
    .unwrap();
    assert!(domain.is_none());
}

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
