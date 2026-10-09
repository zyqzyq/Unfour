use super::*;
use std::sync::Arc;

fn options() -> WorkspaceBundleOptions {
    WorkspaceBundleOptions {
        password: Some("connection-test-password".into()),
        include_secrets: true,
        ..Default::default()
    }
}
fn input(connection: &SshConnection, value: &str) -> SshConnectionInput {
    SshConnectionInput {
        id: Some(connection.id.clone()),
        workspace_id: connection.workspace_id.clone(),
        name: format!("{} edited", connection.name),
        host: connection.host.clone(),
        port: Some(connection.port),
        username: connection.username.clone(),
        auth_kind: connection.auth_kind.clone(),
        key_path: connection.key_path.clone(),
        credential_ref: connection.credential_ref.clone(),
        secret: Some(value.into()),
    }
}

#[tokio::test]
async fn shared_imported_connection_credentials_rotate_without_mutating_siblings_and_survive_failed_edits(
) {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let mut fixture = tests::fixture();
    let mut sibling = fixture["connections"][0].clone();
    sibling["id"] = json!("ssh-sibling");
    sibling["name"] = json!("Sibling");
    fixture["connections"].as_array_mut().unwrap().push(sibling);
    let source = bus
        .workspace_bundle_import(fixture.to_string(), "Source".into())
        .await
        .unwrap();
    let credential = bus
        .secret_store
        .create_credential(
            source.id.clone(),
            "ssh-password".into(),
            "Test".into(),
            "connection-original".into(),
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE connections SET credential_ref=? WHERE workspace_id=? AND connection_type='ssh'",
    )
    .bind(credential.credential_ref)
    .bind(&source.id)
    .execute(bus.db.pool())
    .await
    .unwrap();
    let artifact = bus
        .workspace_bundle_export_with_options(source.id, options())
        .await
        .unwrap();
    let copy = bus
        .workspace_bundle_import_with_options(artifact.content, "Copy".into(), options())
        .await
        .unwrap();
    let connections = bus.list_ssh_connections(copy.id.clone()).await.unwrap();
    let reference = connections[0].credential_ref.clone().unwrap();
    assert_eq!(
        connections[1].credential_ref.as_deref(),
        Some(reference.as_str())
    );
    bus.extensions = crate::CommandBusExtensions::new(vec![Arc::new(tests::FailCommit)]);
    assert!(bus
        .save_ssh_connection(input(&connections[0], "failed-connection-secret"))
        .await
        .is_err());
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), reference.clone())
            .await
            .unwrap(),
        "connection-original"
    );
    let staged: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_bundle_credential_journal WHERE state!='attached'",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(staged, 0);
    bus.extensions = crate::CommandBusExtensions::default();
    let changed = bus
        .save_ssh_connection(input(&connections[0], "connection-rotated"))
        .await
        .unwrap();
    let new_reference = changed.credential_ref.unwrap();
    assert_ne!(reference, new_reference);
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), reference.clone())
            .await
            .unwrap(),
        "connection-original"
    );
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), new_reference.clone())
            .await
            .unwrap(),
        "connection-rotated"
    );
    bus.delete_ssh_connection(copy.id.clone(), connections[0].id.clone())
        .await
        .unwrap();
    assert!(bus
        .secret_store
        .read_secret(copy.id.clone(), new_reference)
        .await
        .is_err());
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), reference.clone())
            .await
            .unwrap(),
        "connection-original"
    );
    let request: String = sqlx::query_scalar("SELECT id FROM api_requests WHERE workspace_id=?")
        .bind(&copy.id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE api_requests SET auth_json=? WHERE id=?")
        .bind(
            json!({"type":"bearer","token":format!("{{{{@unfour-secret:{reference}}}}}")})
                .to_string(),
        )
        .bind(&request)
        .execute(bus.db.pool())
        .await
        .unwrap();
    bus.delete_ssh_connection(copy.id.clone(), connections[1].id.clone())
        .await
        .unwrap();
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), reference.clone())
            .await
            .unwrap(),
        "connection-original"
    );
    bus.delete_api_request(copy.id.clone(), request)
        .await
        .unwrap();
    assert!(bus
        .secret_store
        .read_secret(copy.id, reference)
        .await
        .is_err());
}

#[tokio::test]
async fn imported_database_credential_delete_retains_other_connections_and_api_references() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let mut fixture = tests::fixture();
    let mut sibling = fixture["connections"][1].clone();
    sibling["id"] = json!("db-sibling");
    sibling["name"] = json!("DB sibling");
    fixture["connections"].as_array_mut().unwrap().push(sibling);
    let source = bus
        .workspace_bundle_import(fixture.to_string(), "Source".into())
        .await
        .unwrap();
    let credential = bus
        .secret_store
        .create_credential(
            source.id.clone(),
            "database-password".into(),
            "Test".into(),
            "database-original".into(),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE connections SET credential_ref=? WHERE workspace_id=? AND id IN (SELECT connection_id FROM database_connections WHERE driver='postgres')")
        .bind(credential.credential_ref).bind(&source.id).execute(bus.db.pool()).await.unwrap();
    let artifact = bus
        .workspace_bundle_export_with_options(source.id, options())
        .await
        .unwrap();
    let copy = bus
        .workspace_bundle_import_with_options(artifact.content, "Copy".into(), options())
        .await
        .unwrap();
    let connections: Vec<_> = bus
        .list_database_connections(copy.id.clone())
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.driver == "postgres")
        .collect();
    let reference = connections[0].credential_ref.clone().unwrap();
    assert_eq!(
        connections[1].credential_ref.as_deref(),
        Some(reference.as_str())
    );
    bus.extensions = crate::CommandBusExtensions::new(vec![Arc::new(tests::FailCommit)]);
    assert!(bus
        .delete_database_connection(copy.id.clone(), connections[0].id.clone())
        .await
        .is_err());
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), reference.clone())
            .await
            .unwrap(),
        "database-original"
    );
    bus.extensions = crate::CommandBusExtensions::default();
    bus.delete_database_connection(copy.id.clone(), connections[0].id.clone())
        .await
        .unwrap();
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), reference.clone())
            .await
            .unwrap(),
        "database-original"
    );
    let request: String = sqlx::query_scalar("SELECT id FROM api_requests WHERE workspace_id=?")
        .bind(&copy.id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE api_requests SET auth_json=? WHERE id=?")
        .bind(
            json!({"type":"bearer","token":format!("{{{{@unfour-secret:{reference}}}}}")})
                .to_string(),
        )
        .bind(&request)
        .execute(bus.db.pool())
        .await
        .unwrap();
    bus.delete_database_connection(copy.id.clone(), connections[1].id.clone())
        .await
        .unwrap();
    assert_eq!(
        bus.secret_store
            .read_secret(copy.id.clone(), reference.clone())
            .await
            .unwrap(),
        "database-original"
    );
    bus.delete_api_request(copy.id.clone(), request)
        .await
        .unwrap();
    assert!(bus
        .secret_store
        .read_secret(copy.id, reference)
        .await
        .is_err());
}
