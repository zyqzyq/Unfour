use super::super::*;
use super::support::{mysql_input, service_with_workspace};
use unfour_core::domain::{
    CommandContext, ConnectionSnapshotConfig, ExternalConnectionApply, ExternalConnectionUpsert,
    MutationOrigin,
};

#[tokio::test]
async fn local_and_external_writes_keep_distinct_revision_and_credential_policies() {
    let (service, workspace) = service_with_workspace().await;
    let mut input = mysql_input(&workspace, None);
    let saved = service.save_connection(input.clone()).await.unwrap();
    input.id = Some(saved.id.clone());
    input.credential_ref = Some(format!("test:{workspace}:database:rotated"));
    let local = CommandContext::local("database.connection.save");
    let mut tx = service.db.pool().begin().await.unwrap();
    let outcome = service
        .save_connection_on(&mut tx, &local, input.clone())
        .await
        .unwrap();
    assert!(outcome.mutations.is_empty());
    assert_eq!(outcome.value.revision, saved.revision);
    assert_eq!(outcome.value.updated_at, saved.updated_at);
    assert_eq!(outcome.value.sync_status, saved.sync_status);
    assert_eq!(outcome.value.credential_ref, input.credential_ref);
    tx.commit().await.unwrap();

    input.name = "Changed locally".into();
    input.database = Some("another".into());
    input.ssl_mode = Some(" REQUIRE ".into());
    let mut tx = service.db.pool().begin().await.unwrap();
    let outcome = service
        .save_connection_on(&mut tx, &local, input.clone())
        .await
        .unwrap();
    assert_eq!(outcome.mutations.len(), 1);
    assert_eq!(outcome.mutations[0].origin, MutationOrigin::Local);
    assert_eq!(outcome.value.revision, saved.revision + 1);
    assert_eq!(outcome.value.sync_status, "pending");
    assert_eq!(outcome.value.database, input.database);
    assert_eq!(outcome.value.ssl_mode.as_deref(), Some("require"));
    tx.commit().await.unwrap();

    // External metadata must preserve every compatible device-local setting.
    let local_config = database_config_to_json(&DatabaseConnectionConfig {
        sqlite_path: None,
        connect_timeout_ms: Some(1234),
        statement_timeout_ms: Some(5678),
        default_schema: Some("local-schema".into()),
    })
    .unwrap();
    sqlx::query("UPDATE database_connections SET config_json = ?1 WHERE connection_id = ?2")
        .bind(&local_config)
        .bind(&saved.id)
        .execute(service.db.pool())
        .await
        .unwrap();
    let external = CommandContext::external("workspace.external.apply_page");
    let change = ExternalConnectionApply::Upsert(ExternalConnectionUpsert {
        id: saved.id.clone(),
        workspace_id: workspace.clone(),
        connection_type: "database".into(),
        name: "Remote".into(),
        host: Some("remote-host".into()),
        port: Some(3306),
        config: ConnectionSnapshotConfig::Database {
            driver: "mysql".into(),
            database_name: Some("remote-db".into()),
            username: Some("remote-user".into()),
            ssl_mode: Some("prefer".into()),
            read_only: true,
        },
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-02T00:00:00Z".into(),
    });
    let mut tx = service.db.pool().begin().await.unwrap();
    let outcome = service
        .apply_external_connection_on(&mut tx, &external, change.clone())
        .await
        .unwrap();
    assert!(outcome.value.is_none());
    assert_eq!(outcome.mutations.len(), 1);
    assert_eq!(outcome.mutations[0].origin, MutationOrigin::External);
    assert_eq!(outcome.mutations[0].revision, saved.revision + 2);
    let repeated = service
        .apply_external_connection_on(&mut tx, &external, change)
        .await
        .unwrap();
    assert!(repeated.mutations.is_empty());
    assert!(repeated.value.is_none());
    tx.commit().await.unwrap();
    let remote = service.get_connection(&workspace, &saved.id).await.unwrap();
    assert_eq!(remote.credential_ref, input.credential_ref);
    assert_eq!(remote.sync_status, "local");
    assert_eq!(remote.created_at, "2026-01-01T00:00:00Z");
    assert_eq!(remote.updated_at, "2026-01-02T00:00:00Z");
    assert_eq!(remote.database.as_deref(), Some("remote-db"));
    assert_eq!(remote.username.as_deref(), Some("remote-user"));
    assert_eq!(remote.ssl_mode.as_deref(), Some("prefer"));
    assert!(remote.read_only);
    let preserved_config: String =
        sqlx::query_scalar("SELECT config_json FROM database_connections WHERE connection_id = ?1")
            .bind(&saved.id)
            .fetch_one(service.db.pool())
            .await
            .unwrap();
    assert_eq!(preserved_config, local_config);
}

#[tokio::test]
async fn subtype_failure_rolls_back_parent_and_update_does_not_repair_missing_subtype() {
    let (service, workspace) = service_with_workspace().await;
    sqlx::query("CREATE TRIGGER reject_subtype BEFORE INSERT ON database_connections BEGIN SELECT RAISE(ABORT, 'reject subtype'); END")
        .execute(service.db.pool()).await.unwrap();
    assert!(service
        .save_connection(mysql_input(&workspace, None))
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM connections")
        .fetch_one(service.db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER reject_subtype")
        .execute(service.db.pool())
        .await
        .unwrap();
    let saved = service
        .save_connection(mysql_input(&workspace, None))
        .await
        .unwrap();
    sqlx::query("DELETE FROM database_connections WHERE connection_id = ?1")
        .bind(&saved.id)
        .execute(service.db.pool())
        .await
        .unwrap();
    let mut input = mysql_input(&workspace, None);
    input.id = Some(saved.id.clone());
    input.name = "Must not repair".into();
    assert!(matches!(
        service.save_connection(input).await,
        Err(AppError::NotFound(_))
    ));
    let parent: (String, i64) =
        sqlx::query_as("SELECT name, revision FROM connections WHERE id = ?1")
            .bind(saved.id)
            .fetch_one(service.db.pool())
            .await
            .unwrap();
    assert_eq!(parent, (saved.name, saved.revision));
}

#[tokio::test]
async fn saved_sql_detach_is_scoped_live_only_and_transactional() {
    let (service, workspace) = service_with_workspace().await;
    let saved = service
        .save_connection(mysql_input(&workspace, None))
        .await
        .unwrap();
    let mut ids = Vec::new();
    for name in ["live", "deleted"] {
        let sql = service
            .save_sql(SavedSqlInput {
                catalog: None,
                schema: None,
                id: None,
                workspace_id: workspace.clone(),
                connection_id: Some(saved.id.clone()),
                name: name.into(),
                sql: "select 1".into(),
            })
            .await
            .unwrap();
        ids.push(sql.id);
    }
    service
        .delete_saved_sql(workspace.clone(), ids[1].clone())
        .await
        .unwrap();
    let mut tx = service.db.pool().begin().await.unwrap();
    super::super::saved_sql::clear_saved_sql_connection_on(
        &mut tx,
        "other-workspace",
        &saved.id,
        "unchanged",
    )
    .await
    .unwrap();
    let linked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM saved_sql WHERE connection_id = ?1")
        .bind(&saved.id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(linked, 2);
    service
        .delete_connection_on(
            &mut tx,
            &CommandContext::local("database.connection.delete"),
            workspace.clone(),
            saved.id.clone(),
        )
        .await
        .unwrap();
    let rows: Vec<(Option<String>, i64, String)> =
        sqlx::query_as("SELECT connection_id, revision, sync_status FROM saved_sql ORDER BY name")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(rows[0], (Some(saved.id.clone()), 2, "deleted".into()));
    assert_eq!(rows[1], (None, 2, "pending".into()));
    tx.rollback().await.unwrap();
    let linked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM saved_sql WHERE connection_id = ?1")
        .bind(&saved.id)
        .fetch_one(service.db.pool())
        .await
        .unwrap();
    assert_eq!(linked, 2);
    assert!(service.get_connection(&workspace, &saved.id).await.is_ok());
}
