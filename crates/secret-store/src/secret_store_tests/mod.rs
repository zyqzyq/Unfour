use super::*;

#[tokio::test]
async fn named_secrets_are_stored_loaded_overwritten_and_deleted() {
    let store = SecretStore::in_memory("unfour-test");

    store
        .put_named_secret("pro-auth", "login-key", "first-value")
        .await
        .expect("put named secret");
    assert_eq!(
        store
            .get_named_secret("pro-auth", "login-key")
            .await
            .expect("get named secret"),
        "first-value"
    );

    store
        .put_named_secret("pro-auth", "login-key", "rotated-value")
        .await
        .expect("overwrite named secret");
    assert_eq!(
        store
            .get_named_secret("pro-auth", "login-key")
            .await
            .expect("get overwritten named secret"),
        "rotated-value"
    );

    store
        .delete_named_secret("pro-auth", "login-key")
        .await
        .expect("delete named secret");
    assert!(matches!(
        store.get_named_secret("pro-auth", "login-key").await,
        Err(AppError::NotFound(resource)) if resource == "named secret"
    ));
}

#[tokio::test]
async fn named_secret_scopes_and_keys_are_isolated() {
    let store = SecretStore::in_memory("unfour-test");

    store
        .put_named_secret("scope-a", "shared-key", "scope-a-value")
        .await
        .expect("put scope-a secret");
    store
        .put_named_secret("scope-b", "shared-key", "scope-b-value")
        .await
        .expect("put scope-b secret");
    store
        .put_named_secret("scope-a", "other-key", "other-key-value")
        .await
        .expect("put other-key secret");

    assert_eq!(
        store
            .get_named_secret("scope-a", "shared-key")
            .await
            .expect("get scope-a secret"),
        "scope-a-value"
    );
    assert_eq!(
        store
            .get_named_secret("scope-b", "shared-key")
            .await
            .expect("get scope-b secret"),
        "scope-b-value"
    );
    assert_eq!(
        store
            .get_named_secret("scope-a", "other-key")
            .await
            .expect("get other-key secret"),
        "other-key-value"
    );
}

#[tokio::test]
async fn named_secret_inputs_are_validated() {
    let store = SecretStore::in_memory("unfour-test");

    assert!(matches!(
        store.put_named_secret(" ", "key", "value").await,
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        store.put_named_secret("scope", "bad:key", "value").await,
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        store.put_named_secret("scope", "key", "").await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
async fn credentials_are_created_read_and_deleted_by_reference() {
    let store = SecretStore::in_memory("unfour-test");

    let created = store
        .create_credential(
            "workspace-a".to_string(),
            "ssh-password".to_string(),
            "Deploy host password".to_string(),
            "secret-value".to_string(),
        )
        .await
        .expect("create credential");

    assert_eq!(created.workspace_id, "workspace-a");
    assert_eq!(created.kind, "ssh-password");
    assert_eq!(created.label, "Deploy host password");
    assert!(created
        .credential_ref
        .starts_with("unfour-test:workspace-a:ssh-password:"));
    assert!(!created.credential_ref.contains("secret-value"));

    let loaded = store
        .read_secret("workspace-a".to_string(), created.credential_ref.clone())
        .await
        .expect("read credential");
    assert_eq!(loaded, "secret-value");

    store
        .delete_credential("workspace-a".to_string(), created.credential_ref.clone())
        .await
        .expect("delete credential");

    let missing = store
        .read_secret("workspace-a".to_string(), created.credential_ref)
        .await;
    assert!(missing.is_err());
}

#[tokio::test]
async fn credential_refs_cannot_cross_workspace_boundaries() {
    let store = SecretStore::in_memory("unfour-test");

    let created = store
        .create_credential(
            "workspace-a".to_string(),
            "database-password".to_string(),
            "Database password".to_string(),
            "secret-value".to_string(),
        )
        .await
        .expect("create credential");

    let cross_workspace = store
        .read_secret("workspace-b".to_string(), created.credential_ref)
        .await;
    assert!(cross_workspace.is_err());
}

#[tokio::test]
async fn credentials_can_be_rotated_without_changing_reference() {
    let store = SecretStore::in_memory("unfour-test");
    let created = store
        .create_credential(
            "workspace-a".to_string(),
            "ssh-password".to_string(),
            "Deploy password".to_string(),
            "old-secret".to_string(),
        )
        .await
        .expect("create credential");

    let rotated = store
        .rotate_credential(
            "workspace-a".to_string(),
            created.credential_ref.clone(),
            "new-secret".to_string(),
        )
        .await
        .expect("rotate credential");

    assert_eq!(rotated.credential_ref, created.credential_ref);
    assert_eq!(rotated.workspace_id, "workspace-a");
    assert_eq!(rotated.kind, "ssh-password");
    assert_eq!(rotated.label, "Rotated credential");
    let loaded = store
        .read_secret("workspace-a".to_string(), rotated.credential_ref)
        .await
        .expect("read rotated credential");
    assert_eq!(loaded, "new-secret");
}

#[tokio::test]
async fn credential_reference_metadata_is_derived_without_loading_secret() {
    let store = SecretStore::in_memory("unfour-test");
    let created = store
        .create_credential(
            "workspace-a".to_string(),
            "database-password".to_string(),
            "Database password".to_string(),
            "secret-value".to_string(),
        )
        .await
        .expect("create credential");

    let metadata = store
        .inspect_credential("workspace-a".to_string(), created.credential_ref.clone())
        .await
        .expect("inspect credential");
    let wrong_workspace = store
        .inspect_credential("workspace-b".to_string(), created.credential_ref)
        .await;

    assert_eq!(metadata.workspace_id, "workspace-a");
    assert_eq!(metadata.kind, "database-password");
    assert_eq!(metadata.label, "Credential reference");
    assert!(!metadata.credential_ref.contains("secret-value"));
    assert!(wrong_workspace.is_err());
}
