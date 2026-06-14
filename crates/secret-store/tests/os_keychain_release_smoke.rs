use unfour_secret_store::SecretStore;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires access to the platform credential store"]
async fn os_keychain_save_load_delete_for_release_credentials() {
    let service = format!("unfour-release-smoke-{}", Uuid::new_v4());
    let workspace_id = format!("workspace-{}", Uuid::new_v4());
    let store = SecretStore::new(service);
    let kinds = [
        "ssh-password",
        "ssh-private-key-passphrase",
        "postgres-password",
        "mysql-password",
    ];

    for kind in kinds {
        let secret = format!("release-smoke-secret-{kind}-{}", Uuid::new_v4());
        let created = store
            .create_credential(
                workspace_id.clone(),
                kind.to_string(),
                format!("{kind} smoke credential"),
                secret.clone(),
            )
            .await
            .expect("create OS keychain credential");

        assert!(!created.credential_ref.contains(&secret));
        let loaded = store
            .read_secret(workspace_id.clone(), created.credential_ref.clone())
            .await;
        if loaded.is_err() {
            let _ = store
                .delete_credential(workspace_id.clone(), created.credential_ref.clone())
                .await;
        }
        assert_eq!(loaded.expect("load OS keychain credential"), secret);

        store
            .delete_credential(workspace_id.clone(), created.credential_ref.clone())
            .await
            .expect("delete OS keychain credential");
        assert!(
            store
                .read_secret(workspace_id.clone(), created.credential_ref)
                .await
                .is_err(),
            "deleted credential should no longer load"
        );
    }
}
