//! Routine Connection synchronization: device-local material, conflicts and deletes.

use super::support::*;
use unfour_core::models::{DatabaseConnectionInput, SshConnectionInput};

#[path = "connections/bootstrap.rs"]
mod bootstrap;
#[path = "connections/failures.rs"]
mod failures;

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
