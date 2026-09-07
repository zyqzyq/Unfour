//! Future entity types, additive fields, and schema tombstones on pull.

use super::*;

#[tokio::test]
async fn pull_retains_future_entities_advances_cursor_and_blocks_unsafe_apply() {
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

    let mut first_payload = variable_payload("one");
    first_payload["key"] = serde_json::json!("FIRST");
    let future_api_payload = serde_json::json!({
        "collectionId": "future-collection",
        "parentFolderId": null,
        "name": "Future body mode",
        "sortOrder": 0,
        "authJson": "{}",
        "method": "POST",
        "url": "https://example.test/future",
        "headers": [],
        "query": [],
        "body": "future",
        "bodyKind": "multipart",
        "settingsJson": "{\"timeoutMs\":null}",
        "preRequestScript": null,
        "postResponseScript": null,
        "scriptSchemaVersion": 1,
        "createdAt": "2026-08-13T00:00:00Z",
        "updatedAt": "2026-08-13T00:00:00Z"
    });
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: base + 5,
        next_cursor: base + 5,
        changes: vec![
            RemoteChange {
                cursor: base + 1,
                operation_id: "known-before".into(),
                entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
                entity_id: "known-before".into(),
                parent_entity_id: Some(workspace_id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(first_payload),
                deleted_at: None,
            },
            RemoteChange {
                cursor: base + 2,
                operation_id: "future-entity".into(),
                entity_type: "flowNode".into(),
                entity_id: "future-node".into(),
                parent_entity_id: Some(workspace_id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(serde_json::json!({"future": true})),
                deleted_at: None,
            },
            RemoteChange {
                cursor: base + 3,
                operation_id: "future-subtype".into(),
                entity_type: SyncEntityType::ApiRequest.as_str().into(),
                entity_id: "future-api-request".into(),
                parent_entity_id: Some("future-collection".into()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(future_api_payload),
                deleted_at: None,
            },
            RemoteChange {
                cursor: base + 4,
                operation_id: "child-of-future-parent".into(),
                entity_type: SyncEntityType::WorkspaceEnvironmentVariable.as_str().into(),
                entity_id: "blocked-known-child".into(),
                parent_entity_id: Some("future-node".into()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: PAYLOAD_SCHEMA_VERSION,
                payload: Some(variable_payload("must-not-apply")),
                deleted_at: None,
            },
            remote_variable_change(
                &workspace_id,
                base + 5,
                "known-after",
                "known-after",
                "AFTER",
                "two",
            ),
        ],
    });

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::CompatibilityWaiting)
    ));

    let values: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, value FROM workspace_variables WHERE id IN ('known-before', 'known-after') ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(values, Vec::<(String, String)>::new());
    let skipped_business_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM api_requests WHERE id = 'future-api-request'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(skipped_business_rows, 0);
    let skipped_state_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_entity_state WHERE entity_id IN ('future-node', 'future-api-request', 'blocked-known-child')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(skipped_state_rows, 0);

    let retained: Vec<(String, String, i64, String)> = sqlx::query_as(
        r#"SELECT entity_type, entity_id, server_version, compatibility_state
           FROM cloud_sync_remote_entity
           WHERE cloud_workspace_id = ?1 ORDER BY entity_id"#,
    )
    .bind(&binding.cloud_workspace_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(retained.len(), 5);
    assert!(retained.iter().any(|row| {
        row.0 == "flowNode" && row.1 == "future-node" && row.3 == "deferred_compatibility"
    }));
    let status = service.status(&workspace_id).await.unwrap();
    let binding = status.binding.unwrap();
    assert_eq!(binding.last_pulled_cursor, base + 5);
    assert_eq!(binding.state, "error");
    assert_eq!(
        binding.last_error.as_deref(),
        Some("cloud_sync_compatibility_waiting")
    );
    assert_eq!(status.dead_count, 0);
    let diagnostics: Vec<(String, String, String)> = sqlx::query_as(
        r#"SELECT error_code, entity_type, entity_id
           FROM cloud_sync_diagnostics
           WHERE error_code = 'remote_entity_compatibility_deferred'
           ORDER BY id"#,
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        diagnostics,
        vec![
            (
                "remote_entity_compatibility_deferred".into(),
                "flowNode".into(),
                "future-node".into(),
            ),
            (
                "remote_entity_compatibility_deferred".into(),
                "apiRequest".into(),
                "future-api-request".into(),
            ),
        ]
    );

    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert!(matches!(
        restarted.sync_workspace(&workspace_id).await,
        Err(SyncError::CompatibilityWaiting)
    ));
    let retained_after_restart: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_remote_entity WHERE cloud_workspace_id = ?1",
    )
    .bind(&binding.cloud_workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(retained_after_restart, 5);
    assert_eq!(
        restarted
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        base + 5,
    );
}

#[tokio::test]
async fn additive_optional_payload_field_is_retained_and_applied_without_waiting() {
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
    let mut payload = variable_payload("applied");
    payload["key"] = serde_json::json!("ADDITIVE");
    payload["futureOptionalField"] = serde_json::json!({"preserved": true});
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: binding.last_pulled_cursor + 1,
        next_cursor: binding.last_pulled_cursor + 1,
        changes: vec![RemoteChange {
            cursor: binding.last_pulled_cursor + 1,
            operation_id: "additive-field".into(),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: "variable-additive".into(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: SyncEntityType::WorkspaceVariable.payload_schema_version(),
            payload: Some(payload),
            deleted_at: None,
        }],
    });

    service.sync_workspace(&workspace_id).await.unwrap();

    let value: String =
        sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = 'variable-additive'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(value, "applied");
    let retained: (String, String) = sqlx::query_as(
        r#"SELECT canonical_payload_json, compatibility_state
           FROM cloud_sync_remote_entity WHERE entity_id = 'variable-additive'"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(retained.0.contains("futureOptionalField"));
    assert_eq!(retained.1, "supported");
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(
        status.binding.as_ref().unwrap().last_pulled_cursor,
        binding.last_pulled_cursor + 1
    );
    assert_ne!(status.binding.as_ref().unwrap().state, "error");
    assert_eq!(status.dead_count, 0);
}

#[tokio::test]
async fn future_schema_tombstone_for_known_entity_applies_without_waiting() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let variable = seed
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "DELETE_ME", "local", false),
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
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: binding.cloud_workspace_id.clone(),
        current_cursor: binding.last_pulled_cursor + 1,
        next_cursor: binding.last_pulled_cursor + 1,
        changes: vec![RemoteChange {
            cursor: binding.last_pulled_cursor + 1,
            operation_id: "future-schema-delete".into(),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: variable.id.clone(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Delete,
            server_version: 2,
            payload_schema_version: SyncEntityType::WorkspaceVariable.payload_schema_version() + 1,
            payload: None,
            deleted_at: Some("2026-09-06T01:00:00Z".into()),
        }],
    });

    service.sync_workspace(&workspace_id).await.unwrap();

    let deleted_at: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(&variable.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(deleted_at.is_some());
    let compatibility_state: String = sqlx::query_scalar(
        "SELECT compatibility_state FROM cloud_sync_remote_entity WHERE entity_id = ?1",
    )
    .bind(&variable.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(compatibility_state, "supported");
    let status = service.status(&workspace_id).await.unwrap();
    assert_ne!(status.binding.as_ref().unwrap().state, "error");
    assert_eq!(status.dead_count, 0);
}
