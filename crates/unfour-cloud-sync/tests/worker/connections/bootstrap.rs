//! Initial Connection upload and one-time v3-to-v4 reconciliation.

use super::*;
use unfour_core::domain::{DomainEntityKey, DomainEntityType};

#[tokio::test]
async fn initial_upload_bootstraps_existing_connections_from_core_snapshots() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let ssh_path = r"C:\Users\alice\.ssh\id_ed25519";
    let sqlite_path = r"D:\data\device-only.sqlite";
    let ssh = seed
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Local SSH",
            "private-key",
            Some(ssh_path),
        ))
        .await
        .unwrap();
    let database = seed
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Local SQLite",
            "sqlite",
            Some(sqlite_path),
        ))
        .await
        .unwrap();

    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();

    let operations = pushed_operations(&transport);
    let workspace_index = operations
        .iter()
        .position(|operation| operation.entity_type == SyncEntityType::Workspace)
        .unwrap();
    for id in [&ssh.id, &database.id] {
        let index = operations
            .iter()
            .position(|operation| operation.entity_id == *id)
            .unwrap();
        assert!(workspace_index < index);
        let operation = &operations[index];
        assert_eq!(operation.entity_type, SyncEntityType::Connection);
        assert!(operation.parent_entity_id.is_none());
        assert_eq!(
            operation.payload.as_ref().unwrap()["workspaceId"],
            workspace_id
        );
    }
    let encoded = serde_json::to_string(&operations).unwrap();
    for forbidden in [
        ssh_path,
        sqlite_path,
        "credentialRef",
        "password",
        "passphrase",
        "secret",
        "keyPath",
        "sqlitePath",
        "lastConnectedAt",
        "syncStatus",
        "remoteId",
    ] {
        assert!(!encoded.contains(forbidden), "leaked {forbidden}");
    }
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.connection_v4_bootstrap_state, "completed");
    assert_eq!(binding.initial_total, operations.len() as i64);
    assert_eq!(transport.snapshot_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn v3_binding_reconciles_connections_without_replaying_its_old_cursor() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);

    let connection = seed
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Pre-v4 SSH",
            "private-key",
            Some(r"C:\device\pre-v4-key"),
        ))
        .await
        .unwrap();
    let database_connection = seed
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Pre-v4 SQLite",
            "sqlite",
            Some(r"C:\device\pre-v4.sqlite"),
        ))
        .await
        .unwrap();
    let old_cursor = 37_i64;
    transport.cursor.store(old_cursor as u64, Ordering::SeqCst);
    sqlx::query(
        r#"UPDATE cloud_sync_workspace_bindings
           SET last_pulled_cursor = ?1, state = 'uploading',
               initial_total = 5, initial_confirmed = 1,
               connection_v4_bootstrap_state = 'pending'
           WHERE local_workspace_id = ?2"#,
    )
    .bind(old_cursor)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: old_cursor,
        current_cursor: old_cursor,
        items: Vec::new(),
        next_page_token: None,
    });

    service.sync_workspace(&workspace_id).await.unwrap();
    let operations = pushed_operations(&transport);
    assert!(operations.iter().any(|operation| {
        operation.entity_type == SyncEntityType::Connection && operation.entity_id == connection.id
    }));
    assert!(operations.iter().any(|operation| {
        operation.entity_type == SyncEntityType::Connection
            && operation.entity_id == database_connection.id
    }));
    let first_push_count = transport.pushes.lock().unwrap().len();
    service.sync_workspace(&workspace_id).await.unwrap();
    assert_eq!(transport.pushes.lock().unwrap().len(), first_push_count);
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.connection_v4_bootstrap_state, "completed");
    assert!(binding.last_pulled_cursor >= old_cursor);
}

#[tokio::test]
async fn v3_upgrade_snapshot_restores_remote_only_connections_crossed_by_old_cursor() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();

    let old_cursor = 102_i64;
    sqlx::query(
        r#"UPDATE cloud_sync_workspace_bindings
           SET last_pulled_cursor = ?1, state = 'uploading',
               initial_total = 5, initial_confirmed = 1,
               connection_v4_bootstrap_state = 'pending'
           WHERE local_workspace_id = ?2"#,
    )
    .bind(old_cursor)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.cursor.store(old_cursor as u64, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: 104,
        current_cursor: 104,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Connection,
            entity_id: "historical-ssh".into(),
            parent_entity_id: None,
            server_version: 7,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: connection_payload(
                &workspace_id,
                "historical-ssh",
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "deploy", "authMethod": "private-key"
                }),
            ),
        }],
        next_page_token: Some("next".into()),
    });
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: 104,
        current_cursor: 104,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Connection,
            entity_id: "historical-sqlite".into(),
            parent_entity_id: None,
            server_version: 4,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: connection_payload(
                &workspace_id,
                "historical-sqlite",
                "database",
                serde_json::json!({
                    "kind": "database", "driver": "sqlite", "databaseName": null,
                    "username": null, "sslMode": null, "readOnly": false
                }),
            ),
        }],
        next_page_token: None,
    });

    service.sync_workspace(&workspace_id).await.unwrap();

    let ssh = seed
        .list_ssh_connections(workspace_id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|value| value.id == "historical-ssh")
        .expect("remote-only SSH connection restored from snapshot");
    assert!(ssh.key_path.is_none());
    assert!(ssh.credential_ref.is_none());
    let database = seed
        .list_database_connections(workspace_id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|value| value.id == "historical-sqlite")
        .expect("remote-only SQLite connection restored from snapshot");
    assert!(database.sqlite_path.is_none());
    assert!(database.credential_ref.is_none());

    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.connection_v4_bootstrap_state, "completed");
    assert_eq!(binding.last_pulled_cursor, old_cursor);
    assert_eq!(transport.snapshot_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn connection_upgrade_keeps_old_cursor_for_following_incremental_changes() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();

    let old_cursor = 100_i64;
    let historical_payload = connection_payload(
        &workspace_id,
        "historical-connection",
        "ssh",
        serde_json::json!({
            "kind": "ssh", "username": "deploy", "authMethod": "none"
        }),
    );
    sqlx::query(
        r#"UPDATE cloud_sync_workspace_bindings
           SET last_pulled_cursor = ?1, connection_v4_bootstrap_state = 'pending'
           WHERE local_workspace_id = ?2"#,
    )
    .bind(old_cursor)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.cursor.store(102, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: 102,
        current_cursor: 102,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Connection,
            entity_id: "historical-connection".into(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: historical_payload.clone(),
        }],
        next_page_token: None,
    });
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: 102,
        next_cursor: 102,
        changes: vec![
            remote_connection(
                &workspace_id,
                101,
                "historical-connection-op",
                "historical-connection",
                historical_payload,
            ),
            remote_connection(
                &workspace_id,
                102,
                "new-connection-op",
                "new-after-upgrade",
                connection_payload(
                    &workspace_id,
                    "new-after-upgrade",
                    "database",
                    serde_json::json!({
                        "kind": "database", "driver": "sqlite", "databaseName": null,
                        "username": null, "sslMode": null, "readOnly": false
                    }),
                ),
            ),
        ],
    });

    service.sync_workspace(&workspace_id).await.unwrap();

    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.last_pulled_cursor, 102);
    assert!(seed
        .list_database_connections(workspace_id)
        .await
        .unwrap()
        .iter()
        .any(|value| value.id == "new-after-upgrade"));
}

#[tokio::test]
async fn connection_upgrade_timestamp_only_payload_difference_is_a_noop() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let local = seed
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Identical local",
            "private-key",
            Some(r"C:\device\identical-key"),
        ))
        .await
        .unwrap();
    let snapshot = seed
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::Connection,
            &workspace_id,
            &local.id,
        ))
        .await
        .unwrap();
    let mut payload = unfour_cloud_sync::canonical_payload(snapshot)
        .unwrap()
        .unwrap();
    payload["createdAt"] = serde_json::json!("2026-08-20T00:00:00Z");
    payload["updatedAt"] = serde_json::json!("2026-08-20T00:01:00Z");
    let old_cursor = 200_i64;
    sqlx::query(
        r#"UPDATE cloud_sync_workspace_bindings
           SET last_pulled_cursor = ?1, connection_v4_bootstrap_state = 'pending'
           WHERE local_workspace_id = ?2"#,
    )
    .bind(old_cursor)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.cursor.store(old_cursor as u64, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: old_cursor,
        current_cursor: old_cursor,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Connection,
            entity_id: local.id.clone(),
            parent_entity_id: None,
            server_version: 3,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload,
        }],
        next_page_token: None,
    });

    service.sync_workspace(&workspace_id).await.unwrap();

    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.conflict_count, 0);
    assert_eq!(status.pending_count, 0);
    let server_version: i64 = sqlx::query_scalar(
        "SELECT server_version FROM cloud_sync_entity_state WHERE entity_type = 'connection' AND entity_id = ?1",
    )
    .bind(&local.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(server_version, 3);
}

#[tokio::test]
async fn connection_upgrade_unresolved_difference_enters_existing_conflict_flow() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let local = seed
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Local unresolved",
            "private-key",
            Some(r"C:\device\unresolved-key"),
        ))
        .await
        .unwrap();
    let old_cursor = 300_i64;
    sqlx::query(
        r#"UPDATE cloud_sync_workspace_bindings
           SET last_pulled_cursor = ?1, connection_v4_bootstrap_state = 'pending'
           WHERE local_workspace_id = ?2"#,
    )
    .bind(old_cursor)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.cursor.store(old_cursor as u64, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: old_cursor,
        current_cursor: old_cursor,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Connection,
            entity_id: local.id.clone(),
            parent_entity_id: None,
            server_version: 9,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: connection_payload(
                &workspace_id,
                &local.id,
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "remote", "authMethod": "password"
                }),
            ),
        }],
        next_page_token: None,
    });

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Conflict
    );
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.conflict_count, 1);
    assert_eq!(
        status.binding.unwrap().connection_v4_bootstrap_state,
        "completed"
    );
    let current = seed
        .list_ssh_connections(workspace_id)
        .await
        .unwrap()
        .into_iter()
        .find(|value| value.id == local.id)
        .unwrap();
    assert_eq!(current.host, "ssh.example.test");
}

#[tokio::test]
async fn connection_upgrade_failure_keeps_pending_and_retry_is_idempotent() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let old_cursor = 400_i64;
    sqlx::query(
        r#"UPDATE cloud_sync_workspace_bindings
           SET last_pulled_cursor = ?1, connection_v4_bootstrap_state = 'pending'
           WHERE local_workspace_id = ?2"#,
    )
    .bind(old_cursor)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.cursor.store(old_cursor as u64, Ordering::SeqCst);

    assert!(service.sync_workspace(&workspace_id).await.is_err());
    let pending = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(pending.connection_v4_bootstrap_state, "pending");

    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: old_cursor,
        current_cursor: old_cursor,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Connection,
            entity_id: "retry-ssh".into(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: connection_payload(
                &workspace_id,
                "retry-ssh",
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "deploy", "authMethod": "none"
                }),
            ),
        }],
        next_page_token: None,
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
    assert_eq!(transport.snapshot_calls.load(Ordering::SeqCst), 2);
    assert!(seed
        .list_ssh_connections(workspace_id)
        .await
        .unwrap()
        .iter()
        .any(|value| value.id == "retry-ssh"));
}

#[tokio::test]
async fn connection_upgrade_core_apply_failure_rolls_back_prepare_and_retries() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let blocked = seed
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Blocked database",
            "sqlite",
            Some(r"C:\device\blocked.sqlite"),
        ))
        .await
        .unwrap();
    seed.delete_database_connection(workspace_id.clone(), blocked.id.clone())
        .await
        .unwrap();

    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let old_cursor = 500_i64;
    sqlx::query(
        r#"UPDATE cloud_sync_workspace_bindings
           SET last_pulled_cursor = ?1, state = 'active',
               initial_total = 1, initial_confirmed = 1,
               connection_v4_bootstrap_state = 'pending'
           WHERE local_workspace_id = ?2"#,
    )
    .bind(old_cursor)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.cursor.store(old_cursor as u64, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: old_cursor,
        current_cursor: old_cursor,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Connection,
            entity_id: blocked.id.clone(),
            parent_entity_id: None,
            server_version: 4,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: connection_payload(
                &workspace_id,
                &blocked.id,
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "deploy", "authMethod": "none"
                }),
            ),
        }],
        next_page_token: None,
    });

    assert_eq!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Core)
    );
    let marker: String = sqlx::query_scalar(
        "SELECT connection_v4_bootstrap_state FROM cloud_sync_workspace_bindings WHERE local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(marker, "pending");
    let prepared_state_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_entity_state WHERE entity_type = 'connection' AND entity_id = ?1",
    )
    .bind(&blocked.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(prepared_state_count, 0);
    let blocked_row: (String, Option<String>) =
        sqlx::query_as("SELECT connection_type, deleted_at FROM connections WHERE id = ?1")
            .bind(&blocked.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(blocked_row.0, "database");
    assert!(blocked_row.1.is_some());

    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        at_cursor: old_cursor,
        current_cursor: old_cursor,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Connection,
            entity_id: "recovered-after-core-failure".into(),
            parent_entity_id: None,
            server_version: 5,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: connection_payload(
                &workspace_id,
                "recovered-after-core-failure",
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "deploy", "authMethod": "none"
                }),
            ),
        }],
        next_page_token: None,
    });
    service.sync_workspace(&workspace_id).await.unwrap();

    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.connection_v4_bootstrap_state, "completed");
    assert!(seed
        .list_ssh_connections(workspace_id)
        .await
        .unwrap()
        .iter()
        .any(|value| value.id == "recovered-after-core-failure"));
    assert_eq!(transport.snapshot_calls.load(Ordering::SeqCst), 2);
}
