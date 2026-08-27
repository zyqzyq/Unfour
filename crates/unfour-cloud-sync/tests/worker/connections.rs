use super::*;
use unfour_core::domain::{DomainEntityKey, DomainEntityType};
use unfour_core::models::{DatabaseConnectionInput, SshConnectionInput};

fn ssh_input(
    workspace_id: &str,
    id: Option<String>,
    name: &str,
    auth_kind: &str,
    key_path: Option<&str>,
) -> SshConnectionInput {
    SshConnectionInput {
        id,
        workspace_id: workspace_id.into(),
        name: name.into(),
        host: "ssh.example.test".into(),
        port: Some(22),
        username: "deploy".into(),
        auth_kind: auth_kind.into(),
        key_path: key_path.map(str::to_string),
        credential_ref: None,
        secret: None,
    }
}

fn database_input(
    workspace_id: &str,
    id: Option<String>,
    name: &str,
    driver: &str,
    sqlite_path: Option<&str>,
) -> DatabaseConnectionInput {
    let sqlite = driver == "sqlite";
    DatabaseConnectionInput {
        id,
        workspace_id: workspace_id.into(),
        name: name.into(),
        driver: driver.into(),
        host: (!sqlite).then(|| "db.example.test".into()),
        port: (!sqlite).then_some(5432),
        database: (!sqlite).then(|| "app".into()),
        username: (!sqlite).then(|| "app_user".into()),
        ssl_mode: (!sqlite).then(|| "require".into()),
        sqlite_path: sqlite_path.map(str::to_string),
        credential_ref: None,
        read_only: false,
    }
}

fn connection_payload(
    workspace_id: &str,
    id: &str,
    connection_type: &str,
    config: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "workspaceId": workspace_id,
        "connectionType": connection_type,
        "name": format!("Remote {id}"),
        "host": if connection_type == "ssh" { serde_json::json!("remote.example.test") } else { serde_json::Value::Null },
        "port": if connection_type == "ssh" { serde_json::json!(2222) } else { serde_json::Value::Null },
        "config": config,
        "createdAt": "2026-08-21T00:00:00Z",
        "updatedAt": "2026-08-21T01:00:00Z"
    })
}

fn remote_connection(
    _workspace_id: &str,
    cursor: i64,
    operation_id: &str,
    id: &str,
    payload: serde_json::Value,
) -> RemoteChange {
    RemoteChange {
        cursor,
        operation_id: operation_id.into(),
        entity_type: SyncEntityType::Connection,
        entity_id: id.into(),
        parent_entity_id: None,
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(payload),
        deleted_at: None,
    }
}

fn pushed_operations(transport: &MockTransport) -> Vec<unfour_cloud_sync::PushOperation> {
    transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|request| request.operations.iter().cloned())
        .collect()
}

fn clear_pushes(transport: &MockTransport) {
    transport.pushes.lock().unwrap().clear();
}

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

#[tokio::test]
async fn snapshot_download_creates_connection_aggregates_without_device_material() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    *transport.roots.lock().unwrap() = vec!["remote-workspace".into()];
    transport.cursor.store(1, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        items: vec![
            SnapshotItem {
                entity_type: SyncEntityType::Workspace,
                entity_id: "remote-workspace".into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: workspace_payload("Remote Workspace"),
            },
            SnapshotItem {
                entity_type: SyncEntityType::Connection,
                entity_id: "remote-snapshot-ssh".into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: connection_payload(
                    "remote-workspace",
                    "remote-snapshot-ssh",
                    "ssh",
                    serde_json::json!({
                        "kind": "ssh", "username": "deploy", "authMethod": "private-key"
                    }),
                ),
            },
        ],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    let workspace_id = service
        .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
        .await
        .unwrap();
    assert_eq!(workspace_id, "remote-workspace");
    let bus = CommandBus::from_db(db).await.unwrap();
    let connection = bus
        .list_ssh_connections(workspace_id)
        .await
        .unwrap()
        .into_iter()
        .find(|value| value.id == "remote-snapshot-ssh")
        .unwrap();
    assert!(connection.key_path.is_none());
    assert!(connection.credential_ref.is_none());
    let binding = service
        .status("remote-workspace")
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.connection_v4_bootstrap_state, "completed");
}

#[tokio::test]
async fn connection_conflicts_reuse_keep_local_and_use_remote_without_path_leaks() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local_path = r"C:\device\conflict-key";
    let connection = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Local Conflict",
            "private-key",
            Some(local_path),
        ))
        .await
        .unwrap();
    let base = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base + 1,
        next_cursor: base + 1,
        changes: vec![remote_connection(
            &workspace_id,
            base + 1,
            "remote-conflict-one",
            &connection.id,
            connection_payload(
                &workspace_id,
                &connection.id,
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "remote", "authMethod": "private-key"
                }),
            ),
        )],
    });
    transport.cursor.store((base + 1) as u64, Ordering::SeqCst);
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Conflict
    );
    let conflicts = service.conflicts(&workspace_id).await.unwrap();
    assert_eq!(conflicts.len(), 1);
    let encoded = serde_json::to_string(&conflicts).unwrap();
    for forbidden in [
        local_path,
        "credentialRef",
        "keyPath",
        "secret",
        "passphrase",
    ] {
        assert!(!encoded.contains(forbidden), "leaked {forbidden}");
    }

    clear_pushes(&transport);
    service
        .keep_local(&workspace_id, SyncEntityType::Connection, &connection.id)
        .await
        .unwrap();
    let kept = pushed_operations(&transport)
        .into_iter()
        .find(|operation| operation.entity_id == connection.id)
        .unwrap();
    assert_eq!(kept.entity_type, SyncEntityType::Connection);
    assert_eq!(kept.payload.as_ref().unwrap()["host"], "ssh.example.test");
    assert!(!serde_json::to_string(&kept).unwrap().contains(local_path));

    bus.save_ssh_connection(SshConnectionInput {
        id: Some(connection.id.clone()),
        workspace_id: workspace_id.clone(),
        name: "Second Local Edit".into(),
        host: "local-second.example.test".into(),
        port: Some(22),
        username: "deploy".into(),
        auth_kind: "private-key".into(),
        key_path: Some(local_path.into()),
        credential_ref: None,
        secret: None,
    })
    .await
    .unwrap();
    let cursor = base + 2;
    let mut remote = remote_connection(
        &workspace_id,
        cursor,
        "remote-conflict-two",
        &connection.id,
        connection_payload(
            &workspace_id,
            &connection.id,
            "ssh",
            serde_json::json!({
                "kind": "ssh", "username": "remote", "authMethod": "password"
            }),
        ),
    );
    remote.server_version = 3;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor,
        next_cursor: cursor,
        changes: vec![remote],
    });
    transport.cursor.store(cursor as u64, Ordering::SeqCst);
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Conflict
    );
    service
        .use_remote(&workspace_id, SyncEntityType::Connection, &connection.id)
        .await
        .unwrap();
    let applied = seed
        .list_ssh_connections(workspace_id.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|value| value.id == connection.id)
        .unwrap();
    assert_eq!(applied.host, "remote.example.test");
    assert_eq!(applied.auth_kind, "password");
    assert!(applied.key_path.is_none());
    let echo: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE local_workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(echo, 0);
}

#[tokio::test]
async fn permanent_connection_failure_isolated_from_batch_peers() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let connection = bus
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Dead Connection",
            "private-key",
            Some(r"C:\device\dead-key"),
        ))
        .await
        .unwrap();
    let variable = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "SURVIVOR", "value", false),
        )
        .await
        .unwrap();
    transport.fail_operation_once(&connection.id, "invalid_sync_entity");
    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Permanent
    );
    let failed = service.status(&workspace_id).await.unwrap();
    assert_eq!(failed.dead_count, 1);
    assert_eq!(failed.pending_count, 1);
    assert_eq!(failed.dead_letters[0].entity_type, "connection");
    assert_eq!(failed.dead_letters[0].entity_id, connection.id);

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::DeadLetterBlocked
    );
    let remaining = service.status(&workspace_id).await.unwrap();
    assert_eq!(remaining.dead_count, 1);
    assert_eq!(remaining.pending_count, 0);
    assert!(pushed_operations(&transport)
        .iter()
        .any(|operation| operation.entity_id == variable.id));
}

#[tokio::test]
async fn unknown_operation_id_keeps_the_existing_conservative_batch_fallback() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let bus = CommandBus::from_db_with_extensions(db, CommandBusExtensions::new(vec![hook]))
        .await
        .unwrap();
    bus.save_ssh_connection(ssh_input(
        &workspace_id,
        None,
        "Unknown Operation Connection",
        "private-key",
        Some(r"C:\device\unknown-operation-key"),
    ))
    .await
    .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "UNKNOWN_OPERATION_PEER", "value", false),
    )
    .await
    .unwrap();
    transport.fail_unknown_operation_once("not-in-this-batch", "invalid_sync_entity");

    assert_eq!(
        service.sync_workspace(&workspace_id).await.unwrap_err(),
        SyncError::Permanent
    );
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 2);
    assert_eq!(status.pending_count, 0);
}

#[tokio::test]
async fn local_workspace_delete_pushes_connection_tombstones_before_workspace() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace = seed.create_workspace("Local Delete".into()).await.unwrap();
    let connection = seed
        .save_database_connection(database_input(
            &workspace.id,
            None,
            "Delete First",
            "sqlite",
            Some(r"C:\device\delete.sqlite"),
        ))
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace.id).await.unwrap();
    clear_pushes(&transport);
    let bus = CommandBus::from_db_with_extensions(db, CommandBusExtensions::new(vec![hook]))
        .await
        .unwrap();
    bus.delete_workspace(workspace.id.clone()).await.unwrap();
    service.sync_workspace(&workspace.id).await.unwrap();
    let operations = pushed_operations(&transport);
    let connection_index = operations
        .iter()
        .position(|operation| {
            operation.entity_type == SyncEntityType::Connection
                && operation.entity_id == connection.id
                && operation.operation == SyncOperation::Delete
        })
        .unwrap();
    let workspace_index = operations
        .iter()
        .position(|operation| {
            operation.entity_type == SyncEntityType::Workspace
                && operation.operation == SyncOperation::Delete
        })
        .unwrap();
    assert!(connection_index < workspace_index);
}

#[tokio::test]
async fn remote_connections_allow_missing_device_material_and_do_not_echo() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let base = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    let changes = vec![
        remote_connection(
            &workspace_id,
            base + 1,
            "remote-private-key",
            "remote-private-key",
            connection_payload(
                &workspace_id,
                "remote-private-key",
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "deploy", "authMethod": "private-key"
                }),
            ),
        ),
        remote_connection(
            &workspace_id,
            base + 2,
            "remote-password",
            "remote-password",
            connection_payload(
                &workspace_id,
                "remote-password",
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "deploy", "authMethod": "password"
                }),
            ),
        ),
        remote_connection(
            &workspace_id,
            base + 3,
            "remote-sqlite",
            "remote-sqlite",
            connection_payload(
                &workspace_id,
                "remote-sqlite",
                "database",
                serde_json::json!({
                    "kind": "database", "driver": "sqlite", "databaseName": null,
                    "username": null, "sslMode": null, "readOnly": false
                }),
            ),
        ),
    ];
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base + 3,
        next_cursor: base + 3,
        changes,
    });
    transport.cursor.store((base + 3) as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();

    let ssh_connections = seed
        .list_ssh_connections(workspace_id.clone())
        .await
        .unwrap();
    for id in ["remote-private-key", "remote-password"] {
        let connection = ssh_connections.iter().find(|value| value.id == id).unwrap();
        assert!(connection.key_path.is_none());
        assert!(connection.credential_ref.is_none());
    }
    let databases = seed
        .list_database_connections(workspace_id.clone())
        .await
        .unwrap();
    let sqlite = databases
        .iter()
        .find(|value| value.id == "remote-sqlite")
        .unwrap();
    assert!(sqlite.sqlite_path.is_none());
    assert!(sqlite.credential_ref.is_none());
    assert!(pushed_operations(&transport).is_empty());

    let repeated_cursor = base + 4;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: repeated_cursor,
        next_cursor: repeated_cursor,
        changes: vec![remote_connection(
            &workspace_id,
            repeated_cursor,
            "remote-private-key-repeat",
            "remote-private-key",
            connection_payload(
                &workspace_id,
                "remote-private-key",
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "deploy", "authMethod": "private-key"
                }),
            ),
        )],
    });
    transport
        .cursor
        .store(repeated_cursor as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    let live_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM connections WHERE id = 'remote-private-key' AND deleted_at IS NULL",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(live_count, 1);
    assert!(pushed_operations(&transport).is_empty());

    let outbox_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE local_workspace_id = ?1")
            .bind(&workspace_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn remote_apply_preserves_compatible_paths_and_core_clears_incompatible_paths() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    clear_pushes(&transport);
    let ssh = seed
        .save_ssh_connection(ssh_input(
            &workspace_id,
            None,
            "Local SSH",
            "private-key",
            Some(r"C:\device\preserve-key"),
        ))
        .await
        .unwrap();
    let database = seed
        .save_database_connection(database_input(
            &workspace_id,
            None,
            "Local SQLite",
            "sqlite",
            Some(r"C:\device\preserve.sqlite"),
        ))
        .await
        .unwrap();

    let mut cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    let apply_page = |cursor: i64, changes: Vec<RemoteChange>| ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor,
        next_cursor: cursor,
        changes,
    };
    cursor += 1;
    transport.changes.lock().unwrap().push_back(apply_page(
        cursor,
        vec![remote_connection(
            &workspace_id,
            cursor,
            "preserve-ssh",
            &ssh.id,
            connection_payload(
                &workspace_id,
                &ssh.id,
                "ssh",
                serde_json::json!({
                    "kind": "ssh", "username": "remote", "authMethod": "private-key"
                }),
            ),
        )],
    ));
    transport.cursor.store(cursor as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    let preserved = seed
        .list_ssh_connections(workspace_id.clone())
        .await
        .unwrap();
    assert_eq!(
        preserved
            .iter()
            .find(|value| value.id == ssh.id)
            .unwrap()
            .key_path
            .as_deref(),
        Some(r"C:\device\preserve-key")
    );

    cursor += 1;
    let mut password_payload = connection_payload(
        &workspace_id,
        &ssh.id,
        "ssh",
        serde_json::json!({
            "kind": "ssh", "username": "remote", "authMethod": "password"
        }),
    );
    password_payload["updatedAt"] = serde_json::json!("2026-08-21T02:00:00Z");
    transport.changes.lock().unwrap().push_back(apply_page(
        cursor,
        vec![remote_connection(
            &workspace_id,
            cursor,
            "change-auth",
            &ssh.id,
            password_payload,
        )],
    ));
    transport.cursor.store(cursor as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    assert!(seed
        .list_ssh_connections(workspace_id.clone())
        .await
        .unwrap()
        .iter()
        .find(|value| value.id == ssh.id)
        .unwrap()
        .key_path
        .is_none());

    cursor += 1;
    transport.changes.lock().unwrap().push_back(apply_page(
        cursor,
        vec![remote_connection(
            &workspace_id,
            cursor,
            "preserve-sqlite",
            &database.id,
            connection_payload(
                &workspace_id,
                &database.id,
                "database",
                serde_json::json!({
                    "kind": "database", "driver": "sqlite", "databaseName": null,
                    "username": null, "sslMode": null, "readOnly": true
                }),
            ),
        )],
    ));
    transport.cursor.store(cursor as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    assert_eq!(
        seed.list_database_connections(workspace_id.clone())
            .await
            .unwrap()
            .iter()
            .find(|value| value.id == database.id)
            .unwrap()
            .sqlite_path
            .as_deref(),
        Some(r"C:\device\preserve.sqlite")
    );

    cursor += 1;
    transport.changes.lock().unwrap().push_back(apply_page(
        cursor,
        vec![remote_connection(
            &workspace_id,
            cursor,
            "change-driver",
            &database.id,
            connection_payload(
                &workspace_id,
                &database.id,
                "database",
                serde_json::json!({
                    "kind": "database", "driver": "postgres", "databaseName": "app",
                    "username": "remote", "sslMode": "require", "readOnly": true
                }),
            ),
        )],
    ));
    transport.cursor.store(cursor as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    assert!(seed
        .list_database_connections(workspace_id)
        .await
        .unwrap()
        .iter()
        .find(|value| value.id == database.id)
        .unwrap()
        .sqlite_path
        .is_none());
}

#[tokio::test]
async fn remote_workspace_delete_cascades_connections_without_local_echo() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace = seed.create_workspace("Delete Target".into()).await.unwrap();
    let ssh = seed
        .save_ssh_connection(ssh_input(
            &workspace.id,
            None,
            "Cascade SSH",
            "private-key",
            Some(r"C:\device\cascade-key"),
        ))
        .await
        .unwrap();
    let database = seed
        .save_database_connection(database_input(
            &workspace.id,
            None,
            "Cascade SQLite",
            "sqlite",
            Some(r"C:\device\cascade.sqlite"),
        ))
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace.id).await.unwrap();
    clear_pushes(&transport);
    let base = service
        .status(&workspace.id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    let deleted_at = "2026-08-21T03:00:00Z";
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base + 1,
        next_cursor: base + 1,
        changes: vec![RemoteChange {
            cursor: base + 1,
            operation_id: "remote-workspace-delete".into(),
            entity_type: SyncEntityType::Workspace,
            entity_id: workspace.id.clone(),
            parent_entity_id: None,
            operation: SyncOperation::Delete,
            server_version: 2,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION,
            payload: None,
            deleted_at: Some(deleted_at.into()),
        }],
    });
    transport.cursor.store((base + 1) as u64, Ordering::SeqCst);
    service.sync_workspace(&workspace.id).await.unwrap();

    for id in [ssh.id, database.id] {
        let value: String = sqlx::query_scalar("SELECT deleted_at FROM connections WHERE id = ?1")
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(value, deleted_at);
    }
    let echo: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE local_workspace_id = ?1")
            .bind(&workspace.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(echo, 0);
    assert!(pushed_operations(&transport).is_empty());
}
