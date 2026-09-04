use super::support::*;
use unfour_cloud_sync::SyncService;
use unfour_core::domain::{
    ExternalApiCollectionApply, ExternalApiCollectionUpsert, ExternalApiFolderApply,
    ExternalApiFolderUpsert, ExternalApplyPage,
};

fn remote_delete_change(
    cursor: i64,
    operation_id: &str,
    entity_type: SyncEntityType,
    entity_id: &str,
    parent_entity_id: Option<&str>,
    server_version: i64,
) -> RemoteChange {
    RemoteChange {
        cursor,
        operation_id: operation_id.into(),
        entity_type,
        entity_id: entity_id.into(),
        parent_entity_id: parent_entity_id.map(str::to_string),
        operation: SyncOperation::Delete,
        server_version,
        payload_schema_version: 1,
        payload: None,
        deleted_at: Some("2026-08-13T02:00:00Z".into()),
    }
}

fn api_collection_payload(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "description": null,
        "createdAt": "2026-08-13T00:00:00Z",
        "updatedAt": "2026-08-13T00:00:00Z"
    })
}

fn api_request_payload(
    collection_id: &str,
    parent_folder_id: Option<&str>,
    name: &str,
) -> serde_json::Value {
    serde_json::json!({
        "collectionId": collection_id,
        "parentFolderId": parent_folder_id,
        "name": name,
        "sortOrder": 0,
        "authJson": "{}",
        "method": "GET",
        "url": "https://example.test/accounts",
        "headers": [],
        "query": [],
        "body": null,
        "bodyKind": "none",
        "preRequestScript": null,
        "postResponseScript": null,
        "scriptSchemaVersion": 1,
        "createdAt": "2026-08-13T00:00:00Z",
        "updatedAt": "2026-08-13T00:00:00Z"
    })
}

struct CollectionConflictFixture {
    db: LocalDb,
    service: SyncService,
    transport: Arc<MockTransport>,
    workspace_id: String,
    collection_id: String,
    request_id: String,
}

async fn nested_request_edit_conflicts_with_collection_delete() -> CollectionConflictFixture {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let collection = seed
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    let root = seed
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Root".into(),
        )
        .await
        .unwrap();
    let child = seed
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            Some(root.id.clone()),
            "Child".into(),
        )
        .await
        .unwrap();
    let request = seed
        .save_api_request(saved_api_request(
            &workspace_id,
            &collection.id,
            Some(child.id.clone()),
        ))
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.update_api_request(workspace_id.clone(), request.id.clone(), {
        let mut input = saved_api_request(&workspace_id, &collection.id, Some(child.id.clone()));
        input.name = Some("Local edit".into());
        input
    })
    .await
    .unwrap();
    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = NULL WHERE entity_id = ?1")
        .bind(&request.id)
        .execute(db.pool())
        .await
        .unwrap();
    let base_cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base_cursor + 1,
        next_cursor: base_cursor + 1,
        changes: vec![remote_delete_change(
            base_cursor + 1,
            "remote-delete-collection",
            SyncEntityType::ApiCollection,
            &collection.id,
            Some(&workspace_id),
            4,
        )],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    CollectionConflictFixture {
        db,
        service,
        transport,
        workspace_id,
        collection_id: collection.id,
        request_id: request.id,
    }
}

#[tokio::test]
async fn remote_collection_delete_conflicts_with_nested_request_edit_and_pauses_child_push() {
    let fixture = nested_request_edit_conflicts_with_collection_delete().await;
    let conflicts = fixture
        .service
        .conflicts(&fixture.workspace_id)
        .await
        .unwrap();
    assert!(conflicts.iter().any(|conflict| {
        conflict.entity_type == "apiCollection" && conflict.entity_id == fixture.collection_id
    }));
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id = ?1 AND status = 'pending'",
    )
    .bind(&fixture.request_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(pending, 1);
    fixture.transport.pushes.lock().unwrap().clear();
    assert!(matches!(
        fixture.service.sync_workspace(&fixture.workspace_id).await,
        Err(SyncError::Conflict)
    ));
    let pushed_request = fixture
        .transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|push| push.operations.iter())
        .any(|operation| operation.entity_id == fixture.request_id);
    assert!(
        !pushed_request,
        "nested request must stay paused while its collection is in delete conflict"
    );
    let still_live: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM api_collections WHERE id = ?1")
            .bind(&fixture.collection_id)
            .fetch_one(fixture.db.pool())
            .await
            .unwrap();
    assert!(still_live.is_none());
}

#[tokio::test]
async fn keep_local_on_collection_delete_conflict_pushes_collection_before_nested_request() {
    let fixture = nested_request_edit_conflicts_with_collection_delete().await;
    fixture.transport.pushes.lock().unwrap().clear();
    fixture
        .service
        .keep_local(
            &fixture.workspace_id,
            SyncEntityType::ApiCollection,
            &fixture.collection_id,
        )
        .await
        .unwrap();
    assert!(fixture
        .service
        .conflicts(&fixture.workspace_id)
        .await
        .unwrap()
        .is_empty());
    let operations = fixture
        .transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|push| push.operations.iter())
        .cloned()
        .collect::<Vec<_>>();
    let collection_index = operations
        .iter()
        .position(|operation| operation.entity_id == fixture.collection_id)
        .expect("collection keep-local push");
    let request_index = operations
        .iter()
        .position(|operation| operation.entity_id == fixture.request_id)
        .expect("nested request push after conflict clear");
    assert!(collection_index < request_index);
}

#[tokio::test]
async fn use_remote_on_collection_delete_conflict_cascades_and_abandons_child_outbox() {
    let fixture = nested_request_edit_conflicts_with_collection_delete().await;
    fixture
        .service
        .use_remote(
            &fixture.workspace_id,
            SyncEntityType::ApiCollection,
            &fixture.collection_id,
        )
        .await
        .unwrap();
    let deleted: (Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT
             (SELECT deleted_at FROM api_collections WHERE id = ?1),
             (SELECT deleted_at FROM api_requests WHERE id = ?2)"#,
    )
    .bind(&fixture.collection_id)
    .bind(&fixture.request_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert!(deleted.0.is_some());
    assert!(deleted.1.is_some());
    let leftover: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id = ?1 OR entity_id = ?2",
    )
    .bind(&fixture.collection_id)
    .bind(&fixture.request_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(leftover, 0);
    assert_eq!(
        fixture
            .service
            .conflicts(&fixture.workspace_id)
            .await
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn nested_folder_upserts_push_parent_before_child_even_when_child_id_sorts_first() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let ts = "2026-08-13T00:00:00Z";
    seed.apply_external_page(ExternalApplyPage {
        api_collections: vec![ExternalApiCollectionApply::Upsert(
            ExternalApiCollectionUpsert {
                id: "collection-1".into(),
                workspace_id: workspace_id.clone(),
                name: "Accounts".into(),
                description: None,
                created_at: ts.into(),
                updated_at: ts.into(),
            },
        )],
        api_folders: vec![
            ExternalApiFolderApply::Upsert(ExternalApiFolderUpsert {
                id: "a-child".into(),
                workspace_id: workspace_id.clone(),
                collection_id: "collection-1".into(),
                parent_folder_id: Some("z-root".into()),
                name: "Child".into(),
                sort_order: 1,
                created_at: ts.into(),
                updated_at: ts.into(),
            }),
            ExternalApiFolderApply::Upsert(ExternalApiFolderUpsert {
                id: "z-root".into(),
                workspace_id: workspace_id.clone(),
                collection_id: "collection-1".into(),
                parent_folder_id: None,
                name: "Root".into(),
                sort_order: 0,
                created_at: ts.into(),
                updated_at: ts.into(),
            }),
        ],
        ..ExternalApplyPage::default()
    })
    .await
    .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let folder_ops = transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|push| push.operations.iter())
        .filter(|operation| {
            operation.entity_type == SyncEntityType::ApiFolder
                && (operation.entity_id == "z-root" || operation.entity_id == "a-child")
        })
        .map(|operation| operation.entity_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        folder_ops,
        vec!["z-root".to_string(), "a-child".to_string()]
    );
}

#[tokio::test]
async fn operation_level_parent_failure_only_dead_letters_the_failed_request() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    let folder_a = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Folder A".into(),
        )
        .await
        .unwrap();
    let request = bus
        .save_api_request(saved_api_request(
            &workspace_id,
            &collection.id,
            Some(folder_a.id.clone()),
        ))
        .await
        .unwrap();
    let folder_b = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Folder B".into(),
        )
        .await
        .unwrap();
    let request_operation_id: String = sqlx::query_scalar(
        "SELECT operation_id FROM cloud_sync_outbox WHERE entity_type = 'apiRequest' AND entity_id = ?1",
    )
    .bind(&request.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    // Keep the local hierarchy valid, but make the wire parent invalid to
    // model the server rejecting this one operation inside an atomic batch.
    sqlx::query(
        "UPDATE cloud_sync_outbox SET parent_entity_id = 'missing-parent' WHERE operation_id = ?1",
    )
    .bind(&request_operation_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.fail_operation_once(&request.id, "invalid_parent_entity");

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));

    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 1);
    assert_eq!(status.pending_count, 3);
    assert_eq!(status.dead_letters.len(), 1);
    assert_eq!(status.dead_letters[0].entity_id, request.id);
    assert_eq!(status.dead_letters[0].error_code, "invalid_parent_entity");
    for entity_id in [&collection.id, &folder_a.id, &folder_b.id] {
        let row: (String, Option<String>) =
            sqlx::query_as("SELECT status, last_error FROM cloud_sync_outbox WHERE entity_id = ?1")
                .bind(entity_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(row, ("pending".into(), None));
    }
}

#[tokio::test]
async fn dead_parent_blocks_children_until_parent_recovery() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    let folder = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Folder".into(),
        )
        .await
        .unwrap();
    let request = bus
        .save_api_request(saved_api_request(
            &workspace_id,
            &collection.id,
            Some(folder.id.clone()),
        ))
        .await
        .unwrap();
    transport.fail_operation_once(&collection.id, "invalid_parent_entity");

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let failed = service.status(&workspace_id).await.unwrap();
    assert_eq!(failed.dead_count, 1);
    assert_eq!(failed.dead_letters[0].entity_id, collection.id);
    let due = service
        .repository()
        .due_outbox(
            "account-a",
            &failed.binding.as_ref().unwrap().cloud_workspace_id,
            Utc::now(),
            50,
        )
        .await
        .unwrap();
    assert!(
        due.is_empty(),
        "dead collection descendants must stay out of the due queue"
    );
    let push_count = transport.pushes.lock().unwrap().len();

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::DeadLetterBlocked)
    ));
    assert_eq!(transport.pushes.lock().unwrap().len(), push_count);

    service
        .retry_dead_letter_current_local(&workspace_id, &failed.dead_letters[0].operation_id)
        .await
        .unwrap();
    let repaired = service.status(&workspace_id).await.unwrap();
    assert_eq!(repaired.dead_count, 0);
    assert_eq!(repaired.pending_count, 0);
    assert_eq!(repaired.binding.unwrap().state, "active");
    let repaired_batch = transport
        .pushes
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("parent recovery push");
    assert!(repaired_batch
        .operations
        .iter()
        .any(|operation| operation.entity_id == collection.id));
    assert!(repaired_batch
        .operations
        .iter()
        .any(|operation| operation.entity_id == folder.id));
    assert!(repaired_batch
        .operations
        .iter()
        .any(|operation| operation.entity_id == request.id));
}

#[tokio::test]
async fn permanent_error_with_operation_details_only_dead_letters_the_failed_operation() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    bus.api_collection_folder_create(
        workspace_id.clone(),
        collection.id.clone(),
        None,
        "Folder".into(),
    )
    .await
    .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.dead_count, 1);
    assert_eq!(status.pending_count, 1);
}

#[tokio::test]
async fn failed_api_snapshot_apply_does_not_leave_a_workspace_without_binding() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(1, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        items: vec![
            SnapshotItem {
                entity_type: SyncEntityType::Workspace,
                entity_id: "workspace-remote".into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: 1,
                payload: workspace_payload("Remote API"),
            },
            // Orphans are skipped rather than failing apply, so trigger the
            // rollback with a record core still hard-rejects: a blank name.
            SnapshotItem {
                entity_type: SyncEntityType::ApiCollection,
                entity_id: "collection-bad".into(),
                parent_entity_id: Some("workspace-remote".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: api_collection_payload("   "),
            },
        ],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    assert!(matches!(
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await,
        Err(SyncError::Core)
    ));
    let leftover: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM workspaces WHERE id = 'workspace-remote'),
             (SELECT COUNT(*) FROM cloud_sync_workspace_bindings),
             (SELECT COUNT(*) FROM api_collections WHERE id = 'collection-bad')"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(leftover, (0, 0, 0));
}

#[tokio::test]
async fn failed_api_pull_rolls_back_collection_apply_and_cursor() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let base_cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base_cursor + 2,
        next_cursor: base_cursor + 2,
        changes: vec![
            RemoteChange {
                cursor: base_cursor + 1,
                operation_id: "remote-collection".into(),
                entity_type: SyncEntityType::ApiCollection,
                entity_id: "collection-1".into(),
                parent_entity_id: Some(workspace_id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: 1,
                payload: Some(api_collection_payload("Accounts")),
                deleted_at: None,
            },
            // Orphans are skipped rather than failing apply, so trigger the
            // rollback with a record core still hard-rejects: a blank name.
            RemoteChange {
                cursor: base_cursor + 2,
                operation_id: "remote-collection-bad".into(),
                entity_type: SyncEntityType::ApiCollection,
                entity_id: "collection-bad".into(),
                parent_entity_id: Some(workspace_id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: 1,
                payload: Some(api_collection_payload("   ")),
                deleted_at: None,
            },
        ],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Core)
    ));
    let rolled_back: (i64, i64) = sqlx::query_as(
        r#"SELECT binding.last_pulled_cursor,
                  (SELECT COUNT(*) FROM api_collections WHERE id = 'collection-1')
           FROM cloud_sync_workspace_bindings AS binding
           WHERE binding.local_workspace_id = ?1"#,
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rolled_back, (base_cursor, 0));
}

/// An upsert whose parent was concurrently deleted on this device is the
/// doomed-orphan race: the server cascades deletes, so the tombstone for this
/// entity is already at a later cursor. Core skips the write and the pull
/// must advance past the page instead of wedging on it forever.
#[tokio::test]
async fn pull_skips_api_orphans_under_concurrently_deleted_parents() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let base_cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base_cursor + 1,
        next_cursor: base_cursor + 1,
        changes: vec![RemoteChange {
            cursor: base_cursor + 1,
            operation_id: "orphan-request".into(),
            entity_type: SyncEntityType::ApiRequest,
            entity_id: "request-1".into(),
            parent_entity_id: Some("missing-collection".into()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: 1,
            payload: Some(api_request_payload("missing-collection", None, "Orphan")),
            deleted_at: None,
        }],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let advanced: (i64, i64) = sqlx::query_as(
        r#"SELECT binding.last_pulled_cursor,
                  (SELECT COUNT(*) FROM api_requests WHERE id = 'request-1')
           FROM cloud_sync_workspace_bindings AS binding
           WHERE binding.local_workspace_id = ?1"#,
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(advanced, (base_cursor + 1, 0));
}
