use super::super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_secret_store::SecretStore;

/// Build a service with one saved password SSH connection ready for a
/// session, returning (service, workspace_id, connection_id).
pub(super) async fn diagnostic_fixture() -> (SshService, String, String) {
    let (service, workspace_a, _workspace_b) = service_with_workspaces().await;
    let connection = service
        .save_connection(password_input(&workspace_a))
        .await
        .expect("save ssh connection");
    (service, workspace_a, connection.id)
}

pub(super) async fn service_with_workspaces() -> (SshService, String, String) {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory app db");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("run migrations");

    let secret_store = SecretStore::in_memory("unfour-test");

    let workspace_a = Uuid::new_v4().to_string();
    let workspace_b = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    for workspace_id in [&workspace_a, &workspace_b] {
        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, ?3, 0, ?2, ?2, ?2, 1, 'local')
            "#,
        )
        .bind(workspace_id)
        .bind(&now)
        .bind(format!("Test Workspace {workspace_id}"))
        .execute(db.pool())
        .await
        .expect("insert workspace");
    }

    (SshService::new(db, secret_store), workspace_a, workspace_b)
}

pub(super) fn password_input(workspace_id: &str) -> SshConnectionInput {
    SshConnectionInput {
        id: None,
        workspace_id: workspace_id.to_string(),
        name: "Deploy host".to_string(),
        host: " example.internal ".to_string(),
        port: None,
        username: " deploy ".to_string(),
        auth_kind: "password".to_string(),
        key_path: None,
        credential_ref: Some("ssh-password-1".to_string()),
        secret: None,
    }
}
