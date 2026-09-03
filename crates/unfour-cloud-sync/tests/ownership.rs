use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_cloud_sync::{SyncDependencies, SyncError, SyncOutboxHook, SyncRepository};
use unfour_command_bus::{CommandBus, CommandBusExtensions};
use unfour_core::models::WorkspaceVariableInput;
use unfour_core::AppError;
use unfour_local_storage::LocalDb;

fn variable(key: &str) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id: None,
        key: key.into(),
        value: "must-rollback".into(),
        is_secret: false,
        is_enabled: true,
        description: Some("metadata".into()),
        sort_order: 0,
    }
}

async fn database() -> LocalDb {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("sqlite");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("core migrations");
    unfour_cloud_sync_storage::migrate(db.pool())
        .await
        .expect("cloud sync migrations");
    db
}

#[tokio::test]
async fn ambiguous_historical_bindings_fail_mutation_without_fanout() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.expect("seed bus");
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let dependencies = SyncDependencies::default();
    let repository = SyncRepository::new(db.pool().clone());
    repository
        .create_binding_with_initial_outbox(
            "account-a",
            0,
            &workspace_id,
            "cloud-a",
            0,
            dependencies.ids.as_ref(),
            dependencies.clock.as_ref(),
        )
        .await
        .expect("owner binding");
    sqlx::query(
        r#"INSERT INTO cloud_sync_workspace_bindings (
             account_id, local_workspace_id, cloud_workspace_id,
             sync_enabled, state, initial_cursor, created_at, updated_at
           ) VALUES ('account-b', ?1, 'cloud-b', 1, 'active', 0,
                     '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')"#,
    )
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .expect("historical duplicate binding");
    sqlx::query("DELETE FROM cloud_sync_workspace_ownership WHERE local_workspace_id = ?1")
        .bind(&workspace_id)
        .execute(db.pool())
        .await
        .expect("remove owner metadata for historical fixture");
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .unwrap();

    let hook = Arc::new(SyncOutboxHook::new(
        dependencies.ids,
        dependencies.clock,
        None,
    ));
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .expect("ambiguous fixture bus");
    let error = bus
        .workspace_variable_create(workspace_id.clone(), variable("AMBIGUOUS"))
        .await
        .expect_err("ambiguous ownership must reject the mutation");
    assert!(matches!(
        error,
        AppError::Config(code) if code == SyncError::WorkspaceOwnershipAmbiguous.code()
    ));
    let business: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_variables WHERE workspace_id = ?1 AND key = 'AMBIGUOUS'",
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let outbox: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!((business, outbox), (0, 0));
}

#[tokio::test]
async fn single_binding_without_owner_fails_mutation_without_fallback() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.expect("seed bus");
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let dependencies = SyncDependencies::default();
    let repository = SyncRepository::new(db.pool().clone());
    repository
        .create_binding_with_initial_outbox(
            "account-a",
            0,
            &workspace_id,
            "cloud-a",
            0,
            dependencies.ids.as_ref(),
            dependencies.clock.as_ref(),
        )
        .await
        .expect("owner binding");
    sqlx::query("DELETE FROM cloud_sync_workspace_ownership WHERE local_workspace_id = ?1")
        .bind(&workspace_id)
        .execute(db.pool())
        .await
        .expect("remove owner metadata for invariant fixture");
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .expect("clear initial outbox");

    let hook = Arc::new(SyncOutboxHook::new(
        dependencies.ids,
        dependencies.clock,
        None,
    ));
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .expect("invariant fixture bus");
    let error = bus
        .workspace_variable_create(workspace_id.clone(), variable("MISSING_OWNER"))
        .await
        .expect_err("missing ownership metadata must reject the mutation");
    assert!(matches!(
        error,
        AppError::Config(code) if code == SyncError::WorkspaceOwnershipInvariant.code()
    ));
    let business: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_variables WHERE workspace_id = ?1 AND key = 'MISSING_OWNER'",
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let outbox: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!((business, outbox), (0, 0));
}
