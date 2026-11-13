use unfour_local_storage::LocalDb;

async fn database() -> LocalDb {
    let db = LocalDb::connect_ephemeral().await.unwrap();
    db.migrate().await.unwrap();
    sqlx::query("INSERT INTO workspaces(id,name,created_at,updated_at) VALUES('ws','Workspace','now','now')")
        .execute(db.pool()).await.unwrap();
    db
}

async fn revision(db: &LocalDb, domain: &str) -> i64 {
    sqlx::query_scalar("SELECT COALESCE((SELECT version FROM local_data_versions WHERE workspace_id='ws' AND domain=?),0)")
        .bind(domain).fetch_one(db.pool()).await.unwrap()
}

#[tokio::test]
async fn child_changes_advance_only_the_affected_scope_and_rollback_is_invisible() {
    let db = database().await;
    sqlx::query("INSERT INTO workspace_environments(id,workspace_id,name,created_at,updated_at) VALUES('env','ws','Local','unchanged','unchanged')")
        .execute(db.pool()).await.unwrap();
    let before = revision(&db, "environments").await;
    sqlx::query("INSERT INTO workspace_environment_variables(id,workspace_id,environment_id,key,value,created_at,updated_at) VALUES('v','ws','env','URL','one','now','now')")
        .execute(db.pool()).await.unwrap();
    assert!(revision(&db, "environments").await > before);
    let before = revision(&db, "environments").await;
    sqlx::query("UPDATE workspace_environment_variables SET value='two' WHERE id='v'")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(revision(&db, "environments").await > before);
    assert_eq!(revision(&db, "api-requests").await, 0);
    let parent: String =
        sqlx::query_scalar("SELECT updated_at FROM workspace_environments WHERE id='env'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(parent, "unchanged");
    let before = revision(&db, "environments").await;
    let mut transaction = db.pool().begin().await.unwrap();
    sqlx::query("DELETE FROM workspace_environment_variables WHERE id='v'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(revision(&db, "environments").await, before);
    sqlx::query("DELETE FROM workspace_environment_variables WHERE id='v'")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(revision(&db, "environments").await > before);
}

#[tokio::test]
async fn api_route_changes_publish_workspace_clocks_at_commit() {
    let db = database().await;
    sqlx::query("INSERT INTO workspace_environments(id,workspace_id,name,created_at,updated_at) VALUES('env','ws','Test','now','now')")
        .execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO api_collections(id,workspace_id,name,created_at,updated_at) VALUES('c','ws','Collection','now','now')")
        .execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO api_requests(id,workspace_id,collection_id,name,method,url,created_at,updated_at) VALUES('r','ws','c','Health','GET','https://example.test','now','now')")
        .execute(db.pool()).await.unwrap();
    for (table, id, domain, other) in [
        (
            "api_request_local_routes",
            "r",
            "api-requests",
            "environments",
        ),
        (
            "api_environment_local_routes",
            "env",
            "environments",
            "api-requests",
        ),
    ] {
        let unaffected = revision(&db, other).await;
        let before = revision(&db, domain).await;
        sqlx::query(&format!("INSERT INTO {table} VALUES ('ws',?,'first')"))
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
        assert!(revision(&db, domain).await > before);
        let inserted = revision(&db, domain).await;
        let update =
            format!("UPDATE {table} SET ssh_connection_id='second' WHERE workspace_id='ws'");
        let mut tx = db.pool().begin().await.unwrap();
        sqlx::query(&update).execute(&mut *tx).await.unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(revision(&db, domain).await, inserted);
        sqlx::query(&update).execute(db.pool()).await.unwrap();
        let updated = revision(&db, domain).await;
        assert!(updated > inserted);
        sqlx::query(&format!("DELETE FROM {table} WHERE workspace_id='ws'"))
            .execute(db.pool())
            .await
            .unwrap();
        assert!(revision(&db, domain).await > updated);
        assert_eq!(revision(&db, other).await, unaffected);
    }
}

#[tokio::test]
async fn runtime_history_does_not_advance_definition_versions() {
    let db = database().await;
    let before: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT workspace_id, domain, version FROM local_data_versions ORDER BY 1,2",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO api_history(id,workspace_id,method,url,created_at,updated_at) VALUES('h','ws','GET','https://example.test','now','now')").execute(db.pool()).await.unwrap();
    let after: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT workspace_id, domain, version FROM local_data_versions ORDER BY 1,2",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn connections_subtypes_saved_sql_and_folders_have_scoped_commit_clocks() {
    let db = database().await;
    sqlx::query(
        "INSERT INTO workspaces(id,name,created_at,updated_at) VALUES('other','Other','now','now')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    for (id, kind) in [("db", "database"), ("ssh", "ssh")] {
        sqlx::query("INSERT INTO connections(id,workspace_id,connection_type,name,created_at,updated_at) VALUES(?,'ws',?,?,'now','now')")
            .bind(id).bind(kind).bind(id).execute(db.pool()).await.unwrap();
    }
    sqlx::query("INSERT INTO database_connections(connection_id,driver) VALUES('db','postgres')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO ssh_connections(connection_id,username,auth_method) VALUES('ssh','user','none')").execute(db.pool()).await.unwrap();
    for (domain, update) in [
        (
            "database-connections",
            "UPDATE database_connections SET username='changed' WHERE connection_id='db'",
        ),
        (
            "ssh-connections",
            "UPDATE ssh_connections SET config_json='{\"keepAlive\":10}' WHERE connection_id='ssh'",
        ),
    ] {
        let before = revision(&db, domain).await;
        sqlx::query(update).execute(db.pool()).await.unwrap();
        assert!(revision(&db, domain).await > before);
    }
    sqlx::query("INSERT INTO api_collections(id,workspace_id,name,created_at,updated_at) VALUES('c','ws','Collection','now','now')").execute(db.pool()).await.unwrap();
    for (domain, insert, update, delete) in [
        ("api-folders", "INSERT INTO api_collection_folders(id,workspace_id,collection_id,name,created_at,updated_at) VALUES('f','ws','c','Folder','now','now')", "UPDATE api_collection_folders SET name='Changed' WHERE id='f'", "DELETE FROM api_collection_folders WHERE id='f'"),
        ("saved-sql", "INSERT INTO saved_sql(id,workspace_id,name,sql,created_at,updated_at) VALUES('q','ws','Query','select 1','now','now')", "UPDATE saved_sql SET sql='select 2' WHERE id='q'", "DELETE FROM saved_sql WHERE id='q'"),
    ] {
        for statement in [insert, update, delete] {
            let before = revision(&db, domain).await;
            sqlx::query(statement).execute(db.pool()).await.unwrap();
            assert!(revision(&db, domain).await > before);
        }
    }
    let other: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM local_data_versions WHERE workspace_id='other' AND domain<>'workspaces'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(other, 0);
    let before = revision(&db, "workspaces").await;
    sqlx::query("DELETE FROM workspaces WHERE id='ws'")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        revision(&db, "workspaces").await > before,
        "deletion clocks must survive workspace deletion"
    );
}
