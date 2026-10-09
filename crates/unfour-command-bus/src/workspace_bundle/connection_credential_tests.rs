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
    bus.delete_ssh_connection(copy.id.clone(), connections[1].id.clone())
        .await
        .unwrap();
    assert!(bus
        .secret_store
        .read_secret(copy.id, reference)
        .await
        .is_err());
}

#[tokio::test]
async fn imported_database_credential_delete_retains_other_connections() {
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
    bus.delete_database_connection(copy.id.clone(), connections[1].id.clone())
        .await
        .unwrap();
    assert!(bus
        .secret_store
        .read_secret(copy.id, reference)
        .await
        .is_err());
}

fn database_input(connection: &DatabaseConnection) -> DatabaseConnectionInput {
    DatabaseConnectionInput {
        id: Some(connection.id.clone()),
        workspace_id: connection.workspace_id.clone(),
        name: connection.name.clone(),
        driver: connection.driver.clone(),
        host: connection.host.clone(),
        port: connection.port,
        database: connection.database.clone(),
        username: connection.username.clone(),
        ssl_mode: connection.ssl_mode.clone(),
        sqlite_path: connection.sqlite_path.clone(),
        credential_ref: connection.credential_ref.clone(),
        read_only: connection.read_only,
    }
}

#[tokio::test]
async fn saved_database_secret_view_preserve_replace_rollback_and_clear_are_independent() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus
        .workspace_bundle_import(tests::fixture().to_string(), "DB editor".into())
        .await
        .unwrap();
    let connection = bus
        .list_database_connections(workspace.id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.driver == "postgres")
        .unwrap();
    let saved = bus
        .save_database_connection_with_secret(
            database_input(&connection),
            Some("original-db-password".into()),
        )
        .await
        .unwrap();
    let reference = saved.credential_ref.clone().unwrap();
    let reveal = || {
        bus.reveal_connection_secret(
            workspace.id.clone(),
            saved.id.clone(),
            "database".into(),
            reference.clone(),
        )
    };
    assert_eq!(reveal().await.unwrap(), "original-db-password");
    assert_eq!(
        bus.save_database_connection(database_input(&saved))
            .await
            .unwrap()
            .credential_ref,
        Some(reference.clone())
    );
    for (scope, id, kind, expected) in [
        (
            "other-workspace",
            saved.id.as_str(),
            "database",
            reference.as_str(),
        ),
        (
            workspace.id.as_str(),
            "missing",
            "database",
            reference.as_str(),
        ),
        (
            workspace.id.as_str(),
            saved.id.as_str(),
            "ssh",
            reference.as_str(),
        ),
        (
            workspace.id.as_str(),
            saved.id.as_str(),
            "database",
            "arbitrary-ref",
        ),
    ] {
        let error = bus
            .reveal_connection_secret(scope.into(), id.into(), kind.into(), expected.into())
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("CONNECTION_CREDENTIAL_UNAVAILABLE"));
        assert!(!error.to_string().contains("original-db-password"));
    }
    sqlx::query("CREATE TRIGGER fail_credential_edit BEFORE UPDATE OF credential_ref ON connections BEGIN SELECT RAISE(ABORT, 'fixture write failure'); END")
        .execute(bus.db.pool()).await.unwrap();
    assert!(bus
        .save_database_connection_with_secret(
            database_input(&saved),
            Some("failed-replacement".into())
        )
        .await
        .is_err());
    assert_eq!(
        bus.secret_store
            .read_secret(workspace.id.clone(), reference.clone())
            .await
            .unwrap(),
        "original-db-password"
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_bundle_credential_journal WHERE state!='attached'",
    )
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(pending, 0);
    sqlx::query("DROP TRIGGER fail_credential_edit")
        .execute(bus.db.pool())
        .await
        .unwrap();
    let replaced = bus
        .save_database_connection_with_secret(
            database_input(&saved),
            Some("replacement-db-password".into()),
        )
        .await
        .unwrap();
    let replacement = replaced.credential_ref.clone().unwrap();
    assert_ne!(reference, replacement);
    assert!(bus
        .secret_store
        .read_secret(workspace.id.clone(), reference.clone())
        .await
        .is_err());
    assert!(bus
        .reveal_connection_secret(workspace.id.clone(), saved.id, "database".into(), reference)
        .await
        .is_err());
    assert_eq!(
        bus.reveal_connection_secret(
            workspace.id.clone(),
            replaced.id.clone(),
            "database".into(),
            replacement.clone()
        )
        .await
        .unwrap(),
        "replacement-db-password"
    );
    let mut cleared = database_input(&replaced);
    cleared.credential_ref = None;
    assert!(bus
        .save_database_connection(cleared)
        .await
        .unwrap()
        .credential_ref
        .is_none());
    assert!(bus
        .secret_store
        .read_secret(workspace.id.clone(), replacement)
        .await
        .is_err());
    let database_rows: Vec<String> =
        sqlx::query_scalar("SELECT d.config_json FROM database_connections d JOIN connections c ON c.id=d.connection_id WHERE c.workspace_id=?")
            .bind(&workspace.id)
            .fetch_all(bus.db.pool())
            .await
            .unwrap();
    assert!(!database_rows.join("").contains("password"));
    let activity: Vec<String> =
        sqlx::query_scalar("SELECT details_json FROM activity_events WHERE workspace_id=?")
            .bind(workspace.id)
            .fetch_all(bus.db.pool())
            .await
            .unwrap();
    assert!(!activity.join("").contains("db-password"));
}

#[tokio::test]
async fn saved_ssh_password_and_key_passphrase_can_be_viewed_and_cleared_without_replacement() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus
        .workspace_bundle_import(tests::fixture().to_string(), "SSH editor".into())
        .await
        .unwrap();
    let mut connection = bus
        .list_ssh_connections(workspace.id.clone())
        .await
        .unwrap()
        .remove(0);
    for kind in ["password", "private-key"] {
        let mut edit = input(&connection, "ssh-secret-fixture");
        edit.auth_kind = kind.into();
        edit.key_path = (kind == "private-key").then(|| "C:/fixture/id_test".into());
        edit.credential_ref = None;
        connection = bus.save_ssh_connection(edit).await.unwrap();
        let reference = connection.credential_ref.clone().unwrap();
        assert_eq!(
            bus.reveal_connection_secret(
                workspace.id.clone(),
                connection.id.clone(),
                "ssh".into(),
                reference.clone()
            )
            .await
            .unwrap(),
            "ssh-secret-fixture"
        );
        let mut edit = input(&connection, "");
        edit.secret = None;
        assert_eq!(
            bus.save_ssh_connection(edit.clone())
                .await
                .unwrap()
                .credential_ref,
            Some(reference.clone())
        );
        edit.credential_ref = None;
        connection = bus.save_ssh_connection(edit).await.unwrap();
        assert!(connection.credential_ref.is_none());
        assert!(bus
            .secret_store
            .read_secret(workspace.id.clone(), reference)
            .await
            .is_err());
    }
}
