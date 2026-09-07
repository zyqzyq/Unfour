//! Sequential pull apply: delete/restore convergence and payload field preservation.

use super::*;

#[tokio::test]
async fn delete_then_restore_converges_in_page_across_pages_and_after_old_redelivery() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let base = binding.last_pulled_cursor;
    let entity_id = "delete-restore-variable";
    let upsert = |cursor: i64, server_version: i64, value: &str| RemoteChange {
        cursor,
        operation_id: format!("restore-{cursor}"),
        entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
        entity_id: entity_id.into(),
        parent_entity_id: Some(workspace_id.clone()),
        operation: SyncOperation::Upsert,
        server_version,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(variable_payload(value)),
        deleted_at: None,
    };
    let delete = |cursor: i64, server_version: i64| RemoteChange {
        cursor,
        operation_id: format!("delete-{cursor}"),
        entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
        entity_id: entity_id.into(),
        parent_entity_id: Some(workspace_id.clone()),
        operation: SyncOperation::Delete,
        server_version,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: None,
        deleted_at: Some(format!("2026-09-06T00:00:{server_version:02}Z")),
    };

    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: base + 3,
        next_cursor: base + 3,
        changes: vec![
            upsert(base + 1, 1, "created"),
            delete(base + 2, 2),
            upsert(base + 3, 3, "same-page-restored"),
        ],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let same_page: (String, Option<String>) =
        sqlx::query_as("SELECT value, deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(entity_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(same_page, ("same-page-restored".into(), None));

    transport.changes.lock().unwrap().extend([
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: binding.cloud_workspace_id.clone(),
            current_cursor: base + 5,
            next_cursor: base + 4,
            changes: vec![delete(base + 4, 4)],
        },
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: binding.cloud_workspace_id.clone(),
            current_cursor: base + 5,
            next_cursor: base + 5,
            changes: vec![upsert(base + 5, 5, "cross-page-restored")],
        },
    ]);
    service.sync_workspace(&workspace_id).await.unwrap();
    let cross_page: (String, Option<String>) =
        sqlx::query_as("SELECT value, deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(entity_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(cross_page, ("cross-page-restored".into(), None));

    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: base + 6,
        next_cursor: base + 6,
        changes: vec![delete(base + 6, 4)],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let redelivered: (String, Option<String>) =
        sqlx::query_as("SELECT value, deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(entity_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(redelivered, ("cross-page-restored".into(), None));
    let retained: (i64, String) = sqlx::query_as(
        "SELECT server_version, operation FROM cloud_sync_remote_entity WHERE entity_id = ?1",
    )
    .bind(entity_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(retained, (5, "upsert".into()));
}

#[tokio::test]
async fn remote_api_request_changes_apply_each_settings_json_without_resetting_it() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let collection = seed
        .api_collection_create(workspace_id.clone(), "Remote API".into())
        .await
        .unwrap();
    let folder = seed
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Requests".into(),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    let base = binding.last_pulled_cursor;

    let remote_request = |cursor: i64, operation_id: &str, settings_json: &str| RemoteChange {
        cursor,
        operation_id: operation_id.into(),
        entity_type: SyncEntityType::ApiRequest.as_str().into(),
        entity_id: "remote-request".into(),
        parent_entity_id: Some(folder.id.clone()),
        operation: SyncOperation::Upsert,
        server_version: cursor,
        payload_schema_version: PAYLOAD_SCHEMA_VERSION,
        payload: Some(serde_json::json!({
            "collectionId": collection.id.clone(),
            "parentFolderId": folder.id.clone(),
            "name": "Remote request",
            "sortOrder": 0,
            "authJson": "{}",
            "method": "GET",
            "url": "https://example.test/remote",
            "headers": [],
            "query": [],
            "body": null,
            "bodyKind": "none",
            "settingsJson": settings_json,
            "preRequestScript": null,
            "postResponseScript": null,
            "scriptSchemaVersion": 1,
            "createdAt": "2026-08-13T00:00:00Z",
            "updatedAt": "2026-08-13T00:00:00Z"
        })),
        deleted_at: None,
    };

    let accepted = [
        (r#"{"timeoutMs":null}"#, serde_json::json!(null)),
        (r#"{"timeoutMs":0}"#, serde_json::json!(0)),
        (r#"{"timeoutMs":1}"#, serde_json::json!(1)),
        (r#"{"timeoutMs":30000}"#, serde_json::json!(30000)),
        (
            r#"{"timeoutMs":9007199254740991}"#,
            serde_json::json!(9007199254740991_u64),
        ),
    ];
    for (index, (settings_json, expected_timeout)) in accepted.iter().enumerate() {
        let cursor = base + index as i64 + 1;
        transport.changes.lock().unwrap().push_back(ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: binding.cloud_workspace_id.clone(),
            current_cursor: cursor,
            next_cursor: cursor,
            changes: vec![remote_request(
                cursor,
                &format!("remote-request-{index}"),
                settings_json,
            )],
        });
        service.sync_workspace(&workspace_id).await.unwrap();
        let stored_settings: String = sqlx::query_scalar(
            "SELECT settings_json FROM api_requests WHERE id = 'remote-request'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let stored_settings: serde_json::Value = serde_json::from_str(&stored_settings).unwrap();
        assert_eq!(stored_settings["timeoutMs"], *expected_timeout);
    }

    let invalid_cursor = base + accepted.len() as i64 + 1;
    for (index, settings_json) in [r#"{"timeoutMs":9007199254740992}"#, r#"{"timeoutMs":}"#]
        .iter()
        .enumerate()
    {
        transport.changes.lock().unwrap().push_back(ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: binding.cloud_workspace_id.clone(),
            current_cursor: invalid_cursor,
            next_cursor: invalid_cursor,
            changes: vec![remote_request(
                invalid_cursor,
                &format!("remote-request-invalid-{index}"),
                settings_json,
            )],
        });
        assert!(matches!(
            service.sync_workspace(&workspace_id).await,
            Err(SyncError::Core)
        ));
        let binding_after = service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap();
        assert_eq!(binding_after.last_pulled_cursor, invalid_cursor - 1);
        let stored_settings: String = sqlx::query_scalar(
            "SELECT settings_json FROM api_requests WHERE id = 'remote-request'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(stored_settings, r#"{"timeoutMs":9007199254740991}"#);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id = 'remote-request'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        0
    );
}
