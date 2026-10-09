use super::*;

#[tokio::test]
async fn password_names_and_local_handles_are_redacted_even_without_secret_flag() {
    let (bus, db, workspace) = hooked_bus(vec![]).await;
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Test".into())
        .await
        .unwrap();
    for (index, (key, value)) in [
        ("DB_PASSWORD", "password-canary"),
        ("apiKey", "api-key-canary"),
        ("passphrase", "passphrase-canary"),
        ("ordinary", "@unfour-secret:local-handle-canary"),
    ]
    .into_iter()
    .enumerate()
    {
        for environment_scope in [false, true] {
            let key = format!("{key}_{index}");
            let id = if environment_scope {
                bus.workspace_environment_variable_create(
                    workspace.clone(),
                    environment.id.clone(),
                    variable(None, &key, value, false),
                )
                .await
                .unwrap()
                .id
            } else {
                bus.workspace_variable_create(workspace.clone(), variable(None, &key, value, false))
                    .await
                    .unwrap()
                    .id
            };
            let payload: String = sqlx::query_scalar(
                "SELECT canonical_payload_json FROM cloud_sync_outbox WHERE entity_id=?",
            )
            .bind(&id)
            .fetch_one(db.pool())
            .await
            .unwrap();
            let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
            assert_eq!(payload["isSecret"], true);
            assert!(payload.get("value").is_none());
            assert!(!payload.to_string().contains("canary"));
            let key = unfour_core::domain::DomainEntityKey::new(
                if environment_scope {
                    unfour_core::domain::DomainEntityType::WorkspaceEnvironmentVariable
                } else {
                    unfour_core::domain::DomainEntityType::WorkspaceVariable
                },
                &workspace,
                &id,
            );
            let snapshot =
                serde_json::to_value(bus.read_domain_snapshot(&key).await.unwrap()).unwrap();
            assert!(!snapshot.to_string().contains("canary"));
        }
    }
    let ordinary = bus
        .workspace_variable_create(workspace, variable(None, "region", "local", false))
        .await
        .unwrap();
    let payload: String = sqlx::query_scalar(
        "SELECT canonical_payload_json FROM cloud_sync_outbox WHERE entity_id=?",
    )
    .bind(ordinary.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&payload).unwrap()["value"],
        "local"
    );
}
