//! Cloud workspace discovery and atomic snapshot downloads.

use super::support::*;

#[tokio::test]
async fn cloud_workspace_list_uses_the_root_snapshot_name() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(1, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Workspace,
            entity_id: "workspace-remote".into(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: 1,
            payload: workspace_payload("Remote workspace"),
        }],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db, transport);

    let workspaces = service.list_cloud_workspaces().await.unwrap();

    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].name.as_deref(), Some("Remote workspace"));
}

#[tokio::test]
async fn download_reports_a_workspace_name_conflict_before_core_apply() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let local = seed
        .list_workspaces()
        .await
        .unwrap()
        .workspaces
        .into_iter()
        .next()
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(1, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Workspace,
            entity_id: "workspace-remote".into(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: 1,
            payload: workspace_payload(&local.name.to_ascii_lowercase()),
        }],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);

    assert!(matches!(
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await,
        Err(SyncError::WorkspaceNameConflict)
    ));
    let untouched: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM workspaces), (SELECT COUNT(*) FROM cloud_sync_workspace_bindings), (SELECT COUNT(*) FROM cloud_sync_snapshot_staging)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(untouched, (1, 0, 0));
}

#[tokio::test]
async fn download_is_paged_staged_atomic_and_refuses_an_existing_local_root() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let local_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    *transport.roots.lock().unwrap() = vec![local_id.clone()];
    transport.cursor.store(2, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 2,
        current_cursor: 2,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Workspace,
            entity_id: local_id.clone(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: 1,
            payload: workspace_payload("Remote"),
        }],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert!(matches!(
        service.enable(&local_id).await,
        Err(SyncError::CloudWorkspaceNotEmpty)
    ));
    assert!(matches!(
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await,
        Err(SyncError::LocalWorkspaceNotEmpty)
    ));
    let name: String = sqlx::query_scalar("SELECT name FROM workspaces WHERE id = ?1")
        .bind(&local_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_ne!(name, "Remote");
    let untouched: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM cloud_sync_workspace_bindings), (SELECT COUNT(*) FROM cloud_sync_outbox)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(untouched, (0, 0));

    *transport.roots.lock().unwrap() = vec!["workspace-new".into()];
    transport.snapshots.lock().unwrap().extend([
        SnapshotPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-1".into(),
            at_cursor: 2,
            current_cursor: 2,
            items: vec![SnapshotItem {
                entity_type: SyncEntityType::Workspace,
                entity_id: "workspace-new".into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: 1,
                payload: workspace_payload("Downloaded"),
            }],
            next_page_token: Some("page-2".into()),
        },
        SnapshotPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-1".into(),
            at_cursor: 2,
            current_cursor: 2,
            items: vec![SnapshotItem {
                entity_type: SyncEntityType::WorkspaceVariable,
                entity_id: "remote-variable".into(),
                parent_entity_id: Some("workspace-new".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: variable_payload("downloaded"),
            }],
            next_page_token: None,
        },
    ]);
    let downloaded = service
        .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
        .await
        .unwrap();
    assert_eq!(downloaded, "workspace-new");
    let value: String =
        sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = 'remote-variable'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(value, "downloaded");
    let staged: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_snapshot_staging")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(staged, 0);
}

#[tokio::test]
async fn full_snapshot_download_restores_api_tree_through_core_external_apply() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(1, Ordering::SeqCst);
    let workspace_id = "workspace-remote";
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        items: vec![
            SnapshotItem {
                entity_type: SyncEntityType::Workspace,
                entity_id: workspace_id.into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: 1,
                payload: workspace_payload("Remote API"),
            },
            SnapshotItem {
                entity_type: SyncEntityType::ApiCollection,
                entity_id: "collection-1".into(),
                parent_entity_id: Some(workspace_id.into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({
                    "name": "Accounts",
                    "description": null,
                    "createdAt": "2026-08-13T00:00:00Z",
                    "updatedAt": "2026-08-13T00:00:00Z"
                }),
            },
            // Deliberately child-before-parent; Core owns folder topology.
            SnapshotItem {
                entity_type: SyncEntityType::ApiFolder,
                entity_id: "a-child".into(),
                parent_entity_id: Some("z-root".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({
                    "collectionId": "collection-1",
                    "parentFolderId": "z-root",
                    "name": "Child",
                    "sortOrder": 1,
                    "createdAt": "2026-08-13T00:00:00Z",
                    "updatedAt": "2026-08-13T00:00:00Z"
                }),
            },
            SnapshotItem {
                entity_type: SyncEntityType::ApiFolder,
                entity_id: "z-root".into(),
                parent_entity_id: Some("collection-1".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({
                    "collectionId": "collection-1",
                    "parentFolderId": null,
                    "name": "Root",
                    "sortOrder": 0,
                    "createdAt": "2026-08-13T00:00:00Z",
                    "updatedAt": "2026-08-13T00:00:00Z"
                }),
            },
            SnapshotItem {
                entity_type: SyncEntityType::ApiRequest,
                entity_id: "request-1".into(),
                parent_entity_id: Some("a-child".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({
                    "collectionId": "collection-1",
                    "parentFolderId": "a-child",
                    "name": "List accounts",
                    "sortOrder": 0,
                    "authJson": "{}",
                    "method": "GET",
                    "url": "https://example.test/accounts",
                    "headers": [],
                    "query": [],
                    "body": null,
                    "bodyKind": "none",
                    "settingsJson": "{\"timeoutMs\":30000}",
                    "preRequestScript": null,
                    "postResponseScript": null,
                    "scriptSchemaVersion": 1,
                    "createdAt": "2026-08-13T00:00:00Z",
                    "updatedAt": "2026-08-13T00:00:00Z"
                }),
            },
        ],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    assert_eq!(
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await
            .unwrap(),
        workspace_id
    );
    let restored: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM api_collections WHERE workspace_id = ?1 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM api_collection_folders WHERE workspace_id = ?1 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM api_requests WHERE workspace_id = ?1 AND deleted_at IS NULL)"#,
    )
    .bind(workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(restored, (1, 2, 1));
    let location: (String, Option<String>, String) = sqlx::query_as(
        "SELECT collection_id, parent_folder_id, settings_json FROM api_requests WHERE id = 'request-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        location,
        (
            "collection-1".into(),
            Some("a-child".into()),
            r#"{"timeoutMs":30000}"#.into()
        )
    );
}
