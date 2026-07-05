use super::super::super::*;
use super::super::support::{password_input, service_with_workspaces};

#[tokio::test]
async fn ssh_connection_crud_is_workspace_scoped_and_soft_deletes() {
    let (service, workspace_a, workspace_b) = service_with_workspaces().await;

    let created = service
        .save_connection(password_input(&workspace_a))
        .await
        .expect("save ssh connection");
    assert_eq!(created.host, "example.internal");
    assert_eq!(created.port, 22);
    assert_eq!(created.username, "deploy");
    assert_eq!(created.credential_ref.as_deref(), Some("ssh-password-1"));

    let workspace_a_items = service
        .list_connections(workspace_a.clone())
        .await
        .expect("list workspace a");
    let workspace_b_items = service
        .list_connections(workspace_b)
        .await
        .expect("list workspace b");
    assert_eq!(workspace_a_items.len(), 1);
    assert!(workspace_b_items.is_empty());

    let updated = service
        .save_connection(SshConnectionInput {
            id: Some(created.id.clone()),
            name: "Deploy bastion".to_string(),
            port: Some(2222),
            ..password_input(&workspace_a)
        })
        .await
        .expect("update ssh connection");
    assert_eq!(updated.name, "Deploy bastion");
    assert_eq!(updated.port, 2222);
    assert_eq!(updated.sync_status, "pending");

    let remaining = service
        .delete_connection(workspace_a.clone(), created.id)
        .await
        .expect("delete ssh connection");
    assert!(remaining.is_empty());
    assert!(service
        .list_connections(workspace_a)
        .await
        .expect("list after delete")
        .is_empty());
}

#[tokio::test]
async fn ssh_connection_validation_keeps_secrets_out_of_config() {
    let (service, workspace_id, _) = service_with_workspaces().await;

    let missing_credential = service
        .save_connection(SshConnectionInput {
            credential_ref: None,
            ..password_input(&workspace_id)
        })
        .await;
    assert!(matches!(missing_credential, Err(AppError::Validation(_))));

    let private_key = service
        .save_connection(SshConnectionInput {
            auth_kind: "private-key".to_string(),
            key_path: Some("C:/Users/zhang/.ssh/id_ed25519".to_string()),
            credential_ref: Some("ssh-key-passphrase-1".to_string()),
            ..password_input(&workspace_id)
        })
        .await
        .expect("save private key metadata");

    let stored_config: (String, i64, String, String, String, Option<String>) = sqlx::query_as(
        "SELECT c.host, c.port, sub.username, sub.auth_method, sub.config_json, c.credential_ref \
         FROM connections c \
         INNER JOIN ssh_connections sub ON sub.connection_id = c.id \
         WHERE c.id = ?1",
    )
    .bind(&private_key.id)
    .fetch_one(service.db.pool())
    .await
    .expect("load stored config");
    assert_eq!(stored_config.0, "example.internal");
    assert_eq!(stored_config.1, 22);
    assert_eq!(stored_config.2, "deploy");
    assert_eq!(stored_config.3, "private-key");
    assert!(stored_config.4.contains("id_ed25519"));
    assert!(!stored_config.4.contains("ssh-key-passphrase-1"));
    assert_eq!(stored_config.5.as_deref(), Some("ssh-key-passphrase-1"));

    let password_with_secret = service
        .save_connection(SshConnectionInput {
            credential_ref: None,
            secret: Some("plain-text-password".to_string()),
            ..password_input(&workspace_id)
        })
        .await
        .expect("save password credential through secret store");

    let password_config: (String, Option<String>) = sqlx::query_as(
        "SELECT sub.config_json, c.credential_ref \
         FROM connections c \
         INNER JOIN ssh_connections sub ON sub.connection_id = c.id \
         WHERE c.id = ?1",
    )
    .bind(password_with_secret.id)
    .fetch_one(service.db.pool())
    .await
    .expect("load stored password config");
    assert!(!password_config.0.contains("plain-text-password"));
    assert!(password_config.1.is_some());
}
