use unfour_core::domain::{DomainEntityKey, DomainEntityType};

use super::support::*;

#[tokio::test]
async fn device_local_connection_changes_preserve_cloud_metadata_and_skip_hooks() {
    let (bus, db, effects) = bus_with_hook(None, true).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;

    let remote_ssh_id = "remote-device-local-ssh";
    bus.apply_external_connections(vec![external_ssh(
        remote_ssh_id,
        &workspace_id,
        "Remote Device SSH",
        "private-key",
        "2026-08-21T05:00:00Z",
        "2026-08-21T05:00:00Z",
    )])
    .await
    .unwrap();
    let ssh_key = DomainEntityKey::new(DomainEntityType::Connection, &workspace_id, remote_ssh_id);
    let ssh_snapshot_before = bus.read_domain_snapshot(&ssh_key).await.unwrap();
    let mut key_path_only = ssh_input(
        &workspace_id,
        Some(remote_ssh_id.to_string()),
        "Remote Device SSH",
        "private-key",
        Some("C:\\Users\\alice\\.ssh\\id_ed25519"),
        None,
    );
    key_path_only.host = "remote-ssh.example.test".to_string();
    key_path_only.port = Some(2222);
    key_path_only.username = "remote-user".to_string();
    let saved_key_path = bus.save_ssh_connection(key_path_only).await.unwrap();
    assert_eq!(saved_key_path.revision, 1);
    assert_eq!(saved_key_path.updated_at, "2026-08-21T05:00:00Z");
    assert_eq!(saved_key_path.sync_status, "local");
    assert_eq!(
        saved_key_path.key_path.as_deref(),
        Some("C:\\Users\\alice\\.ssh\\id_ed25519")
    );
    assert!(effects.lock().unwrap().is_empty());
    let ssh_activity_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action = 'ssh.connection.save' AND target = ?1",
    )
    .bind(remote_ssh_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(ssh_activity_count, 1);
    assert_eq!(
        ssh_snapshot_before,
        bus.read_domain_snapshot(&ssh_key).await.unwrap()
    );

    let password_ref = format!("unfour-test:{workspace_id}:ssh-password:initial");
    let password = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Device Password SSH",
            "password",
            None,
            Some(&password_ref),
        ))
        .await
        .unwrap();
    effects.lock().unwrap().clear();
    let next_password_ref = format!("unfour-test:{workspace_id}:ssh-password:next");
    let password_updated = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            Some(password.id.clone()),
            "Device Password SSH",
            "password",
            None,
            Some(&next_password_ref),
        ))
        .await
        .unwrap();
    assert_eq!(password_updated.revision, password.revision);
    assert_eq!(password_updated.updated_at, password.updated_at);
    assert_eq!(
        password_updated.credential_ref.as_deref(),
        Some(next_password_ref.as_str())
    );
    assert!(effects.lock().unwrap().is_empty());
    let password_activity_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action = 'ssh.connection.save' AND target = ?1",
    )
    .bind(&password.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(password_activity_count, 2);

    let passphrase_ref = format!("unfour-test:{workspace_id}:ssh-key-passphrase:initial");
    let passphrase = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Device Passphrase SSH",
            "private-key",
            Some("C:\\device\\passphrase-key"),
            Some(&passphrase_ref),
        ))
        .await
        .unwrap();
    effects.lock().unwrap().clear();
    let next_passphrase_ref = format!("unfour-test:{workspace_id}:ssh-key-passphrase:next");
    let passphrase_updated = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            Some(passphrase.id.clone()),
            "Device Passphrase SSH",
            "private-key",
            Some("C:\\device\\passphrase-key"),
            Some(&next_passphrase_ref),
        ))
        .await
        .unwrap();
    assert_eq!(passphrase_updated.revision, passphrase.revision);
    assert_eq!(passphrase_updated.updated_at, passphrase.updated_at);
    assert_eq!(
        passphrase_updated.credential_ref.as_deref(),
        Some(next_passphrase_ref.as_str())
    );
    assert!(effects.lock().unwrap().is_empty());
    let passphrase_activity_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action = 'ssh.connection.save' AND target = ?1",
    )
    .bind(&passphrase.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(passphrase_activity_count, 2);

    let remote_sqlite_id = "remote-device-local-sqlite";
    bus.apply_external_connections(vec![external_database_with_read_only(
        remote_sqlite_id,
        &workspace_id,
        "Remote Device SQLite",
        "sqlite",
        "2026-08-21T05:01:00Z",
        "2026-08-21T05:01:00Z",
        false,
    )])
    .await
    .unwrap();
    let sqlite_key = DomainEntityKey::new(
        DomainEntityType::Connection,
        &workspace_id,
        remote_sqlite_id,
    );
    let sqlite_snapshot_before = bus.read_domain_snapshot(&sqlite_key).await.unwrap();
    let mut sqlite_path_only = database_input(
        &workspace_id,
        Some(remote_sqlite_id.to_string()),
        "Remote Device SQLite",
        "sqlite",
        None,
    );
    sqlite_path_only.sqlite_path = Some("D:\\data\\remote-device.sqlite".to_string());
    let saved_sqlite = bus
        .save_database_connection(sqlite_path_only)
        .await
        .unwrap();
    assert_eq!(saved_sqlite.revision, 1);
    assert_eq!(saved_sqlite.updated_at, "2026-08-21T05:01:00Z");
    assert_eq!(saved_sqlite.sync_status, "local");
    assert_eq!(
        saved_sqlite.sqlite_path.as_deref(),
        Some("D:\\data\\remote-device.sqlite")
    );
    assert!(effects.lock().unwrap().is_empty());
    let sqlite_activity_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action = 'database.connection.save' AND target = ?1",
    )
    .bind(remote_sqlite_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(sqlite_activity_count, 1);
    assert_eq!(
        sqlite_snapshot_before,
        bus.read_domain_snapshot(&sqlite_key).await.unwrap()
    );
    effects.lock().unwrap().clear();

    let mut sqlite_read_only = database_input(
        &workspace_id,
        Some(remote_sqlite_id.to_string()),
        "Remote Device SQLite",
        "sqlite",
        None,
    );
    sqlite_read_only.sqlite_path = Some("D:\\data\\remote-device.sqlite".to_string());
    sqlite_read_only.read_only = true;
    let read_only_sqlite = bus
        .save_database_connection(sqlite_read_only)
        .await
        .unwrap();
    assert_eq!(read_only_sqlite.revision, saved_sqlite.revision + 1);
    assert_eq!(read_only_sqlite.sync_status, "pending");
    assert_eq!(mutations_for(&effects, "database.connection.save").len(), 1);

    let postgres_ref = format!("unfour-test:{workspace_id}:database-password:initial");
    let postgres = bus
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Device Postgres",
            "postgres",
            Some(&postgres_ref),
        ))
        .await
        .unwrap();
    effects.lock().unwrap().clear();
    let next_postgres_ref = format!("unfour-test:{workspace_id}:database-password:next");
    let postgres_updated = bus
        .save_database_connection(database_input(
            &workspace_id,
            Some(postgres.id.clone()),
            "Device Postgres",
            "postgres",
            Some(&next_postgres_ref),
        ))
        .await
        .unwrap();
    assert_eq!(postgres_updated.revision, postgres.revision);
    assert_eq!(postgres_updated.updated_at, postgres.updated_at);
    assert_eq!(
        postgres_updated.credential_ref.as_deref(),
        Some(next_postgres_ref.as_str())
    );
    assert!(effects.lock().unwrap().is_empty());
    let postgres_activity_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action = 'database.connection.save' AND target = ?1",
    )
    .bind(&postgres.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(postgres_activity_count, 2);
}

#[tokio::test]
async fn shared_connection_changes_revision_once_and_include_device_local_updates() {
    let (bus, _db, effects) = bus_with_hook(None, true).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;

    let ssh = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Shared SSH",
            "private-key",
            Some("C:\\device\\shared-key"),
            None,
        ))
        .await
        .unwrap();
    effects.lock().unwrap().clear();

    let mut host_changed = ssh_input(
        &workspace_id,
        Some(ssh.id.clone()),
        "Shared SSH",
        "private-key",
        Some("C:\\device\\shared-key"),
        None,
    );
    host_changed.host = "changed-ssh.example.test".to_string();
    let host_updated = bus.save_ssh_connection(host_changed).await.unwrap();
    assert_eq!(host_updated.revision, ssh.revision + 1);
    assert_eq!(mutations_for(&effects, "ssh.connection.save").len(), 1);
    effects.lock().unwrap().clear();

    let password_ref = format!("unfour-test:{workspace_id}:ssh-password:shared");
    let mut auth_changed = ssh_input(
        &workspace_id,
        Some(ssh.id.clone()),
        "Shared SSH",
        "password",
        None,
        Some(&password_ref),
    );
    auth_changed.host = "changed-ssh.example.test".to_string();
    let auth_updated = bus.save_ssh_connection(auth_changed).await.unwrap();
    assert_eq!(auth_updated.revision, host_updated.revision + 1);
    assert_eq!(auth_updated.auth_kind, "password");
    assert_eq!(mutations_for(&effects, "ssh.connection.save").len(), 1);
    effects.lock().unwrap().clear();

    let combined_ref = format!("unfour-test:{workspace_id}:ssh-password:combined");
    let mut combined = ssh_input(
        &workspace_id,
        Some(ssh.id.clone()),
        "Shared SSH",
        "password",
        None,
        Some(&combined_ref),
    );
    combined.host = "combined-ssh.example.test".to_string();
    let combined_updated = bus.save_ssh_connection(combined).await.unwrap();
    assert_eq!(combined_updated.revision, auth_updated.revision + 1);
    assert_eq!(combined_updated.host, "combined-ssh.example.test");
    assert_eq!(mutations_for(&effects, "ssh.connection.save").len(), 1);
    effects.lock().unwrap().clear();

    let mut idempotent = ssh_input(
        &workspace_id,
        Some(ssh.id.clone()),
        "Shared SSH",
        "password",
        None,
        Some(&combined_ref),
    );
    idempotent.host = "combined-ssh.example.test".to_string();
    let idempotent_saved = bus.save_ssh_connection(idempotent).await.unwrap();
    assert_eq!(idempotent_saved.revision, combined_updated.revision);
    assert!(mutations_for(&effects, "ssh.connection.save").is_empty());
    effects.lock().unwrap().clear();

    let database_ref = format!("unfour-test:{workspace_id}:database-password:shared");
    let database_next_ref = format!("unfour-test:{workspace_id}:database-password:next");
    let database = bus
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Shared Database",
            "postgres",
            Some(&database_ref),
        ))
        .await
        .unwrap();
    effects.lock().unwrap().clear();
    let mut driver_changed = database_input(
        &workspace_id,
        Some(database.id.clone()),
        "Shared Database",
        "mysql",
        Some(&database_next_ref),
    );
    driver_changed.port = Some(3306);
    let driver_updated = bus.save_database_connection(driver_changed).await.unwrap();
    assert_eq!(driver_updated.revision, database.revision + 1);
    assert_eq!(driver_updated.driver, "mysql");
    assert_eq!(
        driver_updated.credential_ref.as_deref(),
        Some(database_next_ref.as_str())
    );
    assert_eq!(mutations_for(&effects, "database.connection.save").len(), 1);
    effects.lock().unwrap().clear();

    let mut ssl_changed = database_input(
        &workspace_id,
        Some(database.id.clone()),
        "Shared Database",
        "mysql",
        Some(&database_next_ref),
    );
    ssl_changed.port = Some(3306);
    ssl_changed.ssl_mode = Some("disable".to_string());
    let ssl_updated = bus.save_database_connection(ssl_changed).await.unwrap();
    assert_eq!(ssl_updated.revision, driver_updated.revision + 1);
    assert_eq!(mutations_for(&effects, "database.connection.save").len(), 1);
    effects.lock().unwrap().clear();

    let mut database_name_changed = database_input(
        &workspace_id,
        Some(database.id.clone()),
        "Shared Database",
        "mysql",
        Some(&database_next_ref),
    );
    database_name_changed.port = Some(3306);
    database_name_changed.ssl_mode = Some("disable".to_string());
    database_name_changed.database = Some("new_app".to_string());
    let database_name_updated = bus
        .save_database_connection(database_name_changed)
        .await
        .unwrap();
    assert_eq!(database_name_updated.revision, ssl_updated.revision + 1);
    assert_eq!(mutations_for(&effects, "database.connection.save").len(), 1);
    effects.lock().unwrap().clear();

    let mut database_idempotent = database_input(
        &workspace_id,
        Some(database.id.clone()),
        "Shared Database",
        "mysql",
        Some(&database_next_ref),
    );
    database_idempotent.port = Some(3306);
    database_idempotent.ssl_mode = Some("disable".to_string());
    database_idempotent.database = Some("new_app".to_string());
    let database_idempotent_saved = bus
        .save_database_connection(database_idempotent)
        .await
        .unwrap();
    assert_eq!(
        database_idempotent_saved.revision,
        database_name_updated.revision
    );
    assert!(mutations_for(&effects, "database.connection.save").is_empty());
}
