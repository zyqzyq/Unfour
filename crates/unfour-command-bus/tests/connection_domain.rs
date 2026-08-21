use std::sync::{Arc, Mutex};

use unfour_command_bus::{CommandBus, CommandBusExtensions};
use unfour_core::domain::{
    ConnectionSnapshotConfig, DomainEntityKey, DomainEntityType, DomainSnapshot,
    ExternalConnectionApply, ExternalDelete, ExternalWorkspaceApply, MutationOperation,
};
use unfour_secret_store::SecretStore;

#[path = "connection_domain/support.rs"]
mod support;
use support::*;

#[test]
fn connection_entity_contract_serializes_to_stable_protocol_names() {
    assert_eq!(
        serde_json::to_value(DomainEntityType::Connection).unwrap(),
        serde_json::json!("connection")
    );
    let config = ConnectionSnapshotConfig::Ssh {
        username: "alice".to_string(),
        auth_method: "private-key".to_string(),
    };
    assert_eq!(
        serde_json::to_value(config).unwrap(),
        serde_json::json!({
            "kind": "ssh",
            "username": "alice",
            "authMethod": "private-key"
        })
    );
}

#[tokio::test]
async fn local_connection_crud_emits_one_connection_mutation_and_hook_failure_rolls_back() {
    let (bus, _db, effects) = bus_with_hook(None, true).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let ssh = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "SSH One",
            "private-key",
            Some("C:\\Users\\alice\\.ssh\\id_ed25519"),
            None,
        ))
        .await
        .unwrap();
    let created = mutations_for(&effects, "ssh.connection.save");
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].entity.entity_type, DomainEntityType::Connection);
    assert_eq!(created[0].entity.entity_id, ssh.id);
    assert_eq!(created[0].operation, MutationOperation::Upsert);
    assert_eq!(created[0].revision, 1);
    assert!(created[0].entity.parent_entity_id.is_none());

    effects.lock().unwrap().clear();
    let updated = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            Some(ssh.id.clone()),
            "SSH Updated",
            "private-key",
            Some("C:\\Users\\alice\\.ssh\\id_ed25519"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(updated.revision, 2);
    let updated_mutations = mutations_for(&effects, "ssh.connection.save");
    assert_eq!(updated_mutations.len(), 1);
    assert_eq!(updated_mutations[0].revision, 2);

    effects.lock().unwrap().clear();
    let credential_ref = format!("unfour-test:{workspace_id}:database:one");
    let database = bus
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Database One",
            "postgres",
            Some(&credential_ref),
        ))
        .await
        .unwrap();
    let database_mutations = mutations_for(&effects, "database.connection.save");
    assert_eq!(database_mutations.len(), 1);
    assert_eq!(
        database_mutations[0].entity.entity_type,
        DomainEntityType::Connection
    );
    assert_eq!(database_mutations[0].entity.entity_id, database.id);
    assert_eq!(database_mutations[0].revision, 1);

    effects.lock().unwrap().clear();
    let updated_database = bus
        .save_database_connection(database_input(
            &workspace_id,
            Some(database.id.clone()),
            "Database Updated",
            "postgres",
            Some(&credential_ref),
        ))
        .await
        .unwrap();
    assert_eq!(updated_database.revision, 2);
    let database_update_mutations = mutations_for(&effects, "database.connection.save");
    assert_eq!(database_update_mutations.len(), 1);
    assert_eq!(
        database_update_mutations[0].operation,
        MutationOperation::Upsert
    );
    assert_eq!(database_update_mutations[0].revision, 2);

    let (rejecting_bus, rejecting_db, _) = bus_with_hook(Some("ssh.connection.save"), false).await;
    let rejecting_workspace = rejecting_bus
        .list_workspaces()
        .await
        .unwrap()
        .active_workspace_id;
    rejecting_bus
        .save_ssh_connection(ssh_input(
            &rejecting_workspace,
            None,
            "Must Roll Back",
            "private-key",
            Some("C:\\device\\key"),
            None,
        ))
        .await
        .expect_err("hook must reject connection save");
    let counts: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
          (SELECT COUNT(*) FROM connections WHERE name = 'Must Roll Back'),
          (SELECT COUNT(*) FROM ssh_connections sub
             INNER JOIN connections c ON c.id = sub.connection_id
             WHERE c.name = 'Must Roll Back')
        "#,
    )
    .fetch_one(rejecting_db.pool())
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
}

#[tokio::test]
async fn external_connection_apply_rolls_back_when_the_transactional_hook_rejects_it() {
    let (bus, db, _) = bus_with_hook(Some("workspace.external.apply_page"), false).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;

    bus.apply_external_connections(vec![external_database(
        "rejected-external-database",
        &workspace_id,
        "Rejected External Database",
        "mysql",
        "2026-08-21T00:00:00Z",
        "2026-08-21T00:00:00Z",
    )])
    .await
    .expect_err("external apply must remain inside the hook-owned transaction");

    let counts: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
          (SELECT COUNT(*) FROM connections WHERE id = 'rejected-external-database'),
          (SELECT COUNT(*) FROM database_connections WHERE connection_id = 'rejected-external-database')
        "#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
}

#[tokio::test]
async fn rejected_ssh_save_compensates_rotated_and_new_keychain_secrets() {
    let db = database().await;
    let secret_store = SecretStore::in_memory("unfour-test");
    let initial_bus = CommandBus::from_db_with_secret_store(db.clone(), secret_store.clone())
        .await
        .unwrap();
    let workspace_id = initial_bus
        .list_workspaces()
        .await
        .unwrap()
        .active_workspace_id;
    let credential = secret_store
        .create_credential(
            workspace_id.clone(),
            "ssh-password".to_string(),
            "Existing SSH password".to_string(),
            "old-password".to_string(),
        )
        .await
        .unwrap();
    let saved = initial_bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Password SSH",
            "password",
            None,
            Some(&credential.credential_ref),
        ))
        .await
        .unwrap();

    let captured_ref = Arc::new(Mutex::new(None));
    let rejecting_bus = CommandBus::from_db_with_secret_store_and_extensions(
        db.clone(),
        secret_store.clone(),
        CommandBusExtensions::new(vec![Arc::new(CaptureCredentialAndRejectHook {
            credential_ref: captured_ref.clone(),
        })]),
    )
    .await
    .unwrap();

    let mut rotated = ssh_input(
        &workspace_id,
        Some(saved.id.clone()),
        "Rotated Password SSH",
        "password",
        None,
        Some(&credential.credential_ref),
    );
    rotated.secret = Some("new-password".to_string());
    rejecting_bus
        .save_ssh_connection(rotated)
        .await
        .expect_err("hook must reject rotated credential save");
    assert_eq!(
        secret_store
            .read_secret(workspace_id.clone(), credential.credential_ref.clone())
            .await
            .unwrap(),
        "old-password"
    );
    let stored_after_rotation: (String, i64) =
        sqlx::query_as("SELECT name, revision FROM connections WHERE id = ?1")
            .bind(&saved.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stored_after_rotation, (saved.name, saved.revision));

    *captured_ref.lock().unwrap() = None;
    let mut created = ssh_input(
        &workspace_id,
        None,
        "Rejected New Password SSH",
        "password",
        None,
        None,
    );
    created.secret = Some("temporary-password".to_string());
    rejecting_bus
        .save_ssh_connection(created)
        .await
        .expect_err("hook must reject new credential save");
    let rejected_credential_ref = captured_ref
        .lock()
        .unwrap()
        .clone()
        .expect("hook captured generated credential reference");
    assert!(
        secret_store
            .read_secret(workspace_id.clone(), rejected_credential_ref)
            .await
            .is_err(),
        "generated credential must be removed after transaction rollback"
    );
    let rejected_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM connections WHERE name = ?1")
        .bind("Rejected New Password SSH")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(rejected_rows, 0);
}

#[tokio::test]
async fn snapshots_and_enumeration_exclude_all_device_local_connection_fields() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let ssh_ref = format!("unfour-test:{workspace_id}:ssh-key-passphrase:one");
    let ssh = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Snapshot SSH",
            "private-key",
            Some("C:\\Users\\alice\\.ssh\\snapshot-key"),
            Some(&ssh_ref),
        ))
        .await
        .unwrap();
    sqlx::query("UPDATE connections SET last_connected_at = 'device-last-used' WHERE id = ?1")
        .bind(&ssh.id)
        .execute(db.pool())
        .await
        .unwrap();
    let sqlite = bus
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Snapshot SQLite",
            "sqlite",
            None,
        ))
        .await
        .unwrap();

    let keys = bus
        .list_connection_domain_entities(workspace_id.clone())
        .await
        .unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.iter().all(|key| {
        key.entity_type == DomainEntityType::Connection && key.parent_entity_id.is_none()
    }));
    for key in keys {
        let snapshot = bus.read_domain_snapshot(&key).await.unwrap();
        let serialized = serde_json::to_string(&snapshot).unwrap();
        for excluded in [
            "credentialRef",
            "password",
            "keyPath",
            "sqlitePath",
            "secret",
            "snapshot-key",
            "device-only.sqlite",
            "lastConnectedAt",
            "device-last-used",
            "syncStatus",
            "remoteId",
        ] {
            assert!(
                !serialized.contains(excluded),
                "snapshot leaked excluded field/value: {excluded}"
            );
        }
    }

    let DomainSnapshot::Connection(ssh_snapshot) = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::Connection,
            &workspace_id,
            &ssh.id,
        ))
        .await
        .unwrap()
    else {
        panic!("expected SSH connection snapshot");
    };
    assert!(matches!(
        ssh_snapshot.config,
        ConnectionSnapshotConfig::Ssh { .. }
    ));
    let DomainSnapshot::Connection(database_snapshot) = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::Connection,
            &workspace_id,
            &sqlite.id,
        ))
        .await
        .unwrap()
    else {
        panic!("expected database connection snapshot");
    };
    assert!(matches!(
        database_snapshot.config,
        ConnectionSnapshotConfig::Database { ref driver, .. } if driver == "sqlite"
    ));
}

#[tokio::test]
async fn external_ssh_apply_preserves_compatible_local_material_and_clears_incompatible_material() {
    let (bus, db, effects) = bus_with_hook(None, true).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let credential_ref = format!("unfour-test:{workspace_id}:ssh-key-passphrase:one");
    let local = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Local SSH",
            "private-key",
            Some("C:\\Users\\alice\\.ssh\\id_ed25519"),
            Some(&credential_ref),
        ))
        .await
        .unwrap();
    effects.lock().unwrap().clear();

    let compatible = external_ssh(
        &local.id,
        &workspace_id,
        "Remote SSH",
        "private-key",
        &local.created_at,
        "2026-08-21T01:00:00Z",
    );
    let first = bus
        .apply_external_connections(vec![compatible.clone()])
        .await
        .unwrap();
    assert_eq!(first.applied_count, 1);
    let preserved = bus
        .list_ssh_connections(workspace_id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|connection| connection.id == local.id)
        .unwrap();
    assert_eq!(
        preserved.credential_ref.as_deref(),
        Some(credential_ref.as_str())
    );
    assert_eq!(
        preserved.key_path.as_deref(),
        Some("C:\\Users\\alice\\.ssh\\id_ed25519")
    );
    let revision_after_first = preserved.revision;
    let repeated = bus
        .apply_external_connections(vec![compatible])
        .await
        .unwrap();
    assert_eq!(repeated.applied_count, 0);
    let revision_after_repeat: i64 =
        sqlx::query_scalar("SELECT revision FROM connections WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(revision_after_repeat, revision_after_first);
    assert!(
        effects.lock().unwrap().is_empty(),
        "external apply echoed locally"
    );

    bus.apply_external_connections(vec![external_ssh(
        &local.id,
        &workspace_id,
        "Remote Password SSH",
        "password",
        &local.created_at,
        "2026-08-21T01:01:00Z",
    )])
    .await
    .unwrap();
    let changed: (Option<String>, String) = sqlx::query_as(
        r#"
        SELECT c.credential_ref, sub.config_json
        FROM connections c INNER JOIN ssh_connections sub ON sub.connection_id = c.id
        WHERE c.id = ?1
        "#,
    )
    .bind(&local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(changed.0.is_none());
    assert!(!changed.1.contains("id_ed25519"));

    let password_ref = format!("unfour-test:{workspace_id}:ssh-password:two");
    let password_local = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Local Password SSH",
            "password",
            None,
            Some(&password_ref),
        ))
        .await
        .unwrap();
    bus.apply_external_connections(vec![external_ssh(
        &password_local.id,
        &workspace_id,
        "Remote Password SSH",
        "password",
        &password_local.created_at,
        "2026-08-21T01:01:30Z",
    )])
    .await
    .unwrap();
    let preserved_password_ref: Option<String> =
        sqlx::query_scalar("SELECT credential_ref FROM connections WHERE id = ?1")
            .bind(&password_local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        preserved_password_ref.as_deref(),
        Some(password_ref.as_str())
    );

    bus.apply_external_connections(vec![external_ssh(
        &password_local.id,
        &workspace_id,
        "Changed To Private Key",
        "private-key",
        &password_local.created_at,
        "2026-08-21T01:01:31Z",
    )])
    .await
    .unwrap();
    let password_to_key: (Option<String>, String) = sqlx::query_as(
        r#"
        SELECT c.credential_ref, sub.config_json
        FROM connections c INNER JOIN ssh_connections sub ON sub.connection_id = c.id
        WHERE c.id = ?1
        "#,
    )
    .bind(&password_local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(password_to_key.0.is_none());
    assert_eq!(password_to_key.1, "{}");

    bus.apply_external_connections(vec![external_ssh(
        "external-private-key",
        &workspace_id,
        "New Device Private Key",
        "private-key",
        "2026-08-21T01:02:00Z",
        "2026-08-21T01:02:00Z",
    )])
    .await
    .expect("external private-key connection may omit local key path");
    let new_device = bus
        .list_ssh_connections(workspace_id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|connection| connection.id == "external-private-key")
        .unwrap();
    assert!(new_device.key_path.is_none());
    assert!(new_device.credential_ref.is_none());

    bus.delete_ssh_connection(workspace_id.clone(), local.id.clone())
        .await
        .unwrap();
    let deleted_status: String =
        sqlx::query_scalar("SELECT sync_status FROM connections WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(deleted_status, "deleted");
    bus.apply_external_connections(vec![external_ssh(
        &local.id,
        &workspace_id,
        "Resurrected SSH",
        "password",
        &local.created_at,
        "2026-08-21T01:03:00Z",
    )])
    .await
    .unwrap();
    let resurrected: (Option<String>, String) =
        sqlx::query_as("SELECT deleted_at, sync_status FROM connections WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(resurrected.0.is_none());
    assert_eq!(resurrected.1, "local");
}

#[tokio::test]
async fn external_database_apply_preserves_compatible_credentials_and_allows_pathless_sqlite() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let credential_ref = format!("unfour-test:{workspace_id}:database:one");
    let local = bus
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Local Postgres",
            "postgres",
            Some(&credential_ref),
        ))
        .await
        .unwrap();
    let compatible = external_database(
        &local.id,
        &workspace_id,
        "Remote Postgres",
        "postgres",
        &local.created_at,
        "2026-08-21T02:00:00Z",
    );
    let first = bus
        .apply_external_connections(vec![compatible.clone()])
        .await
        .unwrap();
    assert_eq!(first.applied_count, 1);
    let preserved = bus
        .list_database_connections(workspace_id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|connection| connection.id == local.id)
        .unwrap();
    assert_eq!(
        preserved.credential_ref.as_deref(),
        Some(credential_ref.as_str())
    );
    let preserved_revision = preserved.revision;
    let repeated = bus
        .apply_external_connections(vec![compatible])
        .await
        .unwrap();
    assert_eq!(repeated.applied_count, 0);
    let repeated_revision: i64 =
        sqlx::query_scalar("SELECT revision FROM connections WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(repeated_revision, preserved_revision);

    bus.apply_external_connections(vec![external_database(
        "external-mysql",
        &workspace_id,
        "Remote MySQL",
        "mysql",
        "2026-08-21T02:00:30Z",
        "2026-08-21T02:00:30Z",
    )])
    .await
    .unwrap();
    let mysql = bus
        .list_database_connections(workspace_id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|connection| connection.id == "external-mysql")
        .unwrap();
    assert_eq!(mysql.driver, "mysql");
    assert_eq!(mysql.port, Some(3306));
    assert!(mysql.sqlite_path.is_none());

    bus.apply_external_connections(vec![external_database(
        "external-sqlite",
        &workspace_id,
        "Remote SQLite",
        "sqlite",
        "2026-08-21T02:01:00Z",
        "2026-08-21T02:01:00Z",
    )])
    .await
    .expect("external SQLite connection may omit local file path");
    let sqlite = bus
        .list_database_connections(workspace_id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|connection| connection.id == "external-sqlite")
        .unwrap();
    assert_eq!(sqlite.driver, "sqlite");
    assert!(sqlite.sqlite_path.is_none());

    let local_sqlite = bus
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Local SQLite With Path",
            "sqlite",
            None,
        ))
        .await
        .unwrap();
    bus.apply_external_connections(vec![external_database(
        &local_sqlite.id,
        &workspace_id,
        "Changed To Postgres",
        "postgres",
        &local_sqlite.created_at,
        "2026-08-21T02:01:30Z",
    )])
    .await
    .unwrap();
    let sqlite_to_postgres: String =
        sqlx::query_scalar("SELECT config_json FROM database_connections WHERE connection_id = ?1")
            .bind(&local_sqlite.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(!sqlite_to_postgres.contains("device-only.sqlite"));

    bus.apply_external_connections(vec![external_database(
        &local.id,
        &workspace_id,
        "Changed To SQLite",
        "sqlite",
        &local.created_at,
        "2026-08-21T02:02:00Z",
    )])
    .await
    .unwrap();
    let changed: (Option<String>, String) = sqlx::query_as(
        r#"
        SELECT c.credential_ref, sub.config_json
        FROM connections c INNER JOIN database_connections sub ON sub.connection_id = c.id
        WHERE c.id = ?1
        "#,
    )
    .bind(&local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(changed.0.is_none());
    assert!(!changed.1.contains("device-only.sqlite"));

    bus.delete_database_connection(workspace_id.clone(), local.id.clone())
        .await
        .unwrap();
    let deleted_status: String =
        sqlx::query_scalar("SELECT sync_status FROM connections WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(deleted_status, "deleted");
    bus.apply_external_connections(vec![external_database(
        &local.id,
        &workspace_id,
        "Resurrected Database",
        "sqlite",
        &local.created_at,
        "2026-08-21T02:03:00Z",
    )])
    .await
    .unwrap();
    let resurrected: (Option<String>, String) =
        sqlx::query_as("SELECT deleted_at, sync_status FROM connections WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(resurrected.0.is_none());
    assert_eq!(resurrected.1, "local");
}

#[tokio::test]
async fn connection_delete_and_workspace_cascades_are_tombstoned_in_dependency_order() {
    let (bus, db, effects) = bus_with_hook(None, false).await;
    let target = bus
        .create_workspace("Connection Cascade Target".to_string())
        .await
        .unwrap();
    let connection = bus
        .save_ssh_connection(ssh_input(
            &target.id,
            None,
            "Cascade SSH",
            "private-key",
            Some("C:\\device\\cascade-key"),
            None,
        ))
        .await
        .unwrap();
    let database_connection = bus
        .save_database_connection(database_input(
            &target.id,
            None,
            "Cascade Database",
            "sqlite",
            None,
        ))
        .await
        .unwrap();
    effects.lock().unwrap().clear();
    bus.delete_workspace(target.id.clone()).await.unwrap();
    let workspace_delete = mutations_for(&effects, "workspace.delete");
    let connection_positions: Vec<usize> = workspace_delete
        .iter()
        .enumerate()
        .filter_map(|(index, mutation)| {
            (mutation.entity.entity_type == DomainEntityType::Connection).then_some(index)
        })
        .collect();
    let workspace_position = workspace_delete
        .iter()
        .position(|mutation| mutation.entity.entity_type == DomainEntityType::Workspace)
        .unwrap();
    assert_eq!(connection_positions.len(), 2);
    assert!(connection_positions
        .iter()
        .all(|position| *position < workspace_position));
    let timestamps: (
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
    ) = sqlx::query_as(
        r#"
        SELECT
          (SELECT deleted_at FROM connections WHERE id = ?1),
          (SELECT deleted_at FROM connections WHERE id = ?2),
          (SELECT sync_status FROM connections WHERE id = ?1),
          (SELECT sync_status FROM connections WHERE id = ?2),
          (SELECT deleted_at FROM workspaces WHERE id = ?3)
        "#,
    )
    .bind(&connection.id)
    .bind(&database_connection.id)
    .bind(&target.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(timestamps.0.is_some());
    assert_eq!(timestamps.0, timestamps.1);
    assert_eq!(timestamps.2, "deleted");
    assert_eq!(timestamps.3, "deleted");
    assert_eq!(timestamps.0, timestamps.4);

    let external_target = bus
        .create_workspace("External Connection Cascade".to_string())
        .await
        .unwrap();
    let external_connection = bus
        .save_database_connection(database_input(
            &external_target.id,
            None,
            "Leftover Database",
            "sqlite",
            None,
        ))
        .await
        .unwrap();
    let external_ssh_connection = bus
        .save_ssh_connection(ssh_input(
            &external_target.id,
            None,
            "Leftover SSH",
            "private-key",
            Some("C:\\device\\leftover-key"),
            None,
        ))
        .await
        .unwrap();
    let deleted_at = "2026-08-21T03:00:00Z".to_string();
    bus.apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
        entity: DomainEntityKey::new(
            DomainEntityType::Workspace,
            &external_target.id,
            &external_target.id,
        ),
        deleted_at: deleted_at.clone(),
    })])
    .await
    .unwrap();
    let external_tombstone: (Option<String>, i64, String) =
        sqlx::query_as("SELECT deleted_at, revision, sync_status FROM connections WHERE id = ?1")
            .bind(&external_connection.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(external_tombstone.0.as_deref(), Some(deleted_at.as_str()));
    assert_eq!(external_tombstone.1, external_connection.revision + 1);
    assert_eq!(external_tombstone.2, "deleted");
    let external_ssh_tombstone: (Option<String>, i64, String) =
        sqlx::query_as("SELECT deleted_at, revision, sync_status FROM connections WHERE id = ?1")
            .bind(&external_ssh_connection.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        external_ssh_tombstone.0.as_deref(),
        Some(deleted_at.as_str())
    );
    assert_eq!(
        external_ssh_tombstone.1,
        external_ssh_connection.revision + 1
    );
    assert_eq!(external_ssh_tombstone.2, "deleted");
}

#[tokio::test]
async fn connection_delete_is_revisioned_and_external_apply_rejects_aggregate_type_switches() {
    let (bus, db, effects) = bus_with_hook(None, true).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let database = bus
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Delete Database",
            "sqlite",
            None,
        ))
        .await
        .unwrap();
    effects.lock().unwrap().clear();
    bus.delete_database_connection(workspace_id.clone(), database.id.clone())
        .await
        .unwrap();
    let delete_mutations = mutations_for(&effects, "database.connection.delete");
    assert_eq!(delete_mutations.len(), 1);
    assert_eq!(delete_mutations[0].operation, MutationOperation::Delete);
    assert_eq!(delete_mutations[0].revision, database.revision + 1);
    assert!(matches!(
        bus.read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::Connection,
            &workspace_id,
            &database.id,
        ))
        .await
        .unwrap(),
        DomainSnapshot::Tombstone(_)
    ));

    let ssh = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Stable SSH Type",
            "private-key",
            Some("C:\\device\\stable-key"),
            None,
        ))
        .await
        .unwrap();
    let error = bus
        .apply_external_connections(vec![external_database(
            &ssh.id,
            &workspace_id,
            "Illegal Database Switch",
            "postgres",
            &ssh.created_at,
            "2026-08-21T04:00:00Z",
        )])
        .await
        .expect_err("connection aggregate type switch must be rejected");
    assert!(error.to_string().contains("cannot change"));
    let still_ssh = bus
        .list_ssh_connections(workspace_id)
        .await
        .unwrap()
        .into_iter()
        .find(|connection| connection.id == ssh.id)
        .unwrap();
    assert_eq!(still_ssh.auth_kind, "private-key");

    effects.lock().unwrap().clear();
    let deleted_at = "2026-08-21T04:01:00Z".to_string();
    let external_delete = ExternalConnectionApply::Delete(ExternalDelete {
        entity: DomainEntityKey::new(
            DomainEntityType::Connection,
            &still_ssh.workspace_id,
            &still_ssh.id,
        ),
        deleted_at,
    });
    let first_delete = bus
        .apply_external_connections(vec![external_delete.clone()])
        .await
        .unwrap();
    let repeated_delete = bus
        .apply_external_connections(vec![external_delete])
        .await
        .unwrap();
    assert_eq!(first_delete.applied_count, 1);
    assert_eq!(repeated_delete.applied_count, 0);
    let external_delete_status: String =
        sqlx::query_scalar("SELECT sync_status FROM connections WHERE id = ?1")
            .bind(&still_ssh.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(external_delete_status, "deleted");
    assert!(effects.lock().unwrap().is_empty());
    assert!(matches!(
        bus.read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::Connection,
            &still_ssh.workspace_id,
            &still_ssh.id,
        ))
        .await
        .unwrap(),
        DomainSnapshot::Tombstone(_)
    ));
}
