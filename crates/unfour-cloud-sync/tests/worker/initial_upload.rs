//! Initial upload payloads, topology and restart checkpoints.

use super::support::*;

#[tokio::test]
async fn initial_upload_is_topological_and_matches_all_four_v1_payloads() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    seed.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "TOKEN", "must-never-sync", true),
    )
    .await
    .unwrap();
    let environment = seed
        .workspace_environment_create(workspace_id.clone(), "Test".into())
        .await
        .unwrap();
    seed.workspace_environment_variable_create(
        workspace_id.clone(),
        environment.id.clone(),
        variable(None, "HOST", "example.test", false),
    )
    .await
    .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();

    let pushes = transport.pushes.lock().unwrap();
    let operations = pushes
        .iter()
        .flat_map(|request| request.operations.iter())
        .collect::<Vec<_>>();
    assert_eq!(
        operations.first().unwrap().entity_type,
        SyncEntityType::Workspace
    );
    assert_eq!(
        operations.last().unwrap().entity_type,
        SyncEntityType::WorkspaceEnvironmentVariable
    );
    for operation in &operations {
        let payload = operation.payload.as_ref().unwrap();
        for forbidden in [
            "id",
            "workspaceId",
            "environmentId",
            "secretValue",
            "revision",
            "isDefault",
            "lastOpenedAt",
            "isActive",
        ] {
            assert!(payload.get(forbidden).is_none());
        }
        match operation.entity_type {
            SyncEntityType::Workspace => {
                assert!(operation.parent_entity_id.is_none());
                assert_payload_keys(
                    payload,
                    &[
                        "createdAt",
                        "deletedAt",
                        "environmentType",
                        "mcpPolicy",
                        "name",
                        "updatedAt",
                    ],
                );
            }
            SyncEntityType::WorkspaceVariable => {
                assert_eq!(
                    operation.parent_entity_id.as_deref(),
                    Some(workspace_id.as_str())
                );
                assert_payload_keys(
                    payload,
                    &[
                        "createdAt",
                        "deletedAt",
                        "description",
                        "isEnabled",
                        "isSecret",
                        "key",
                        "sortOrder",
                        "updatedAt",
                    ],
                );
            }
            SyncEntityType::WorkspaceEnvironment => {
                assert_eq!(
                    operation.parent_entity_id.as_deref(),
                    Some(workspace_id.as_str())
                );
                assert_payload_keys(
                    payload,
                    &["createdAt", "deletedAt", "name", "sortOrder", "updatedAt"],
                );
            }
            SyncEntityType::WorkspaceEnvironmentVariable => {
                assert_eq!(
                    operation.parent_entity_id.as_deref(),
                    Some(environment.id.as_str())
                );
                assert_payload_keys(
                    payload,
                    &[
                        "createdAt",
                        "deletedAt",
                        "description",
                        "isEnabled",
                        "isSecret",
                        "key",
                        "sortOrder",
                        "updatedAt",
                        "value",
                    ],
                );
            }
            SyncEntityType::Connection
            | SyncEntityType::ApiCollection
            | SyncEntityType::ApiFolder
            | SyncEntityType::ApiRequest
            | SyncEntityType::SshTask
            | SyncEntityType::SshTaskStep => {
                panic!("unexpected feature entity in workspace-only fixture")
            }
        }
    }
    let serialized = serde_json::to_string(&*pushes).unwrap();
    assert!(!serialized.contains("must-never-sync"));
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.binding.unwrap().state, "active");
    let stored: String = sqlx::query_scalar(
        "SELECT group_concat(coalesce(canonical_payload_json, ''), '') FROM cloud_sync_outbox",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!stored.contains("must-never-sync"));
}

#[tokio::test]
async fn api_initial_upload_uses_core_snapshots_and_existing_push_pipeline() {
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
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();

    {
        let pushes = transport.pushes.lock().unwrap();
        let operations = pushes
            .iter()
            .flat_map(|push| push.operations.iter())
            .collect::<Vec<_>>();
        let collection_op = operations
            .iter()
            .find(|operation| operation.entity_id == collection.id)
            .expect("collection push");
        assert_eq!(collection_op.entity_type, SyncEntityType::ApiCollection);
        assert_eq!(
            collection_op.parent_entity_id.as_deref(),
            Some(workspace_id.as_str())
        );
        assert_payload_keys(
            collection_op.payload.as_ref().unwrap(),
            &["createdAt", "description", "name", "updatedAt"],
        );

        let root_op = operations
            .iter()
            .find(|operation| operation.entity_id == root.id)
            .expect("root folder push");
        assert_eq!(root_op.entity_type, SyncEntityType::ApiFolder);
        assert_eq!(
            root_op.parent_entity_id.as_deref(),
            Some(collection.id.as_str())
        );
        let child_op = operations
            .iter()
            .find(|operation| operation.entity_id == child.id)
            .expect("child folder push");
        assert_eq!(child_op.parent_entity_id.as_deref(), Some(root.id.as_str()));

        let request_op = operations
            .iter()
            .find(|operation| operation.entity_id == request.id)
            .expect("request push");
        assert_eq!(request_op.entity_type, SyncEntityType::ApiRequest);
        assert_eq!(
            request_op.parent_entity_id.as_deref(),
            Some(child.id.as_str())
        );
        assert_payload_keys(
            request_op.payload.as_ref().unwrap(),
            &[
                "authJson",
                "body",
                "bodyKind",
                "collectionId",
                "createdAt",
                "headers",
                "method",
                "name",
                "parentFolderId",
                "postResponseScript",
                "preRequestScript",
                "query",
                "scriptSchemaVersion",
                "settingsJson",
                "sortOrder",
                "updatedAt",
                "url",
            ],
        );
        assert_eq!(
            request_op.payload.as_ref().unwrap()["settingsJson"],
            serde_json::json!(r#"{"timeoutMs":12345}"#)
        );
        for forbidden in [
            "id",
            "workspaceId",
            "revision",
            "syncStatus",
            "remoteId",
            "timeoutMs",
            "temporaryVariables",
        ] {
            assert!(request_op
                .payload
                .as_ref()
                .unwrap()
                .get(forbidden)
                .is_none());
        }
        let serialized = serde_json::to_string(&*pushes).unwrap();
        for secret in [
            "raw-auth-secret",
            "raw-header-secret",
            "raw-query-secret",
            "raw-url-secret",
            "raw-body-secret",
            "raw-runtime-secret",
        ] {
            assert!(!serialized.contains(secret), "push leaked {secret}");
        }
    }
    assert_eq!(service.status(&workspace_id).await.unwrap().dead_count, 0);
}

#[tokio::test]
async fn empty_cloud_with_only_a_local_root_has_a_recoverable_initial_upload() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport);

    service.enable(&workspace_id).await.unwrap();
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.binding.unwrap().state, "active");
    assert_eq!(status.dead_count, 0);
}

#[tokio::test]
async fn initial_upload_cross_batch_checkpoint_resumes_after_restart() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    for index in 0..55 {
        seed.workspace_variable_create(
            workspace_id.clone(),
            variable(None, &format!("KEY_{index:02}"), "value", false),
        )
        .await
        .unwrap();
    }
    let transport = Arc::new(MockTransport::new());
    transport.fail_on_push_number.store(2, Ordering::SeqCst);
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert!(matches!(
        service.enable(&workspace_id).await,
        Err(SyncError::Transport)
    ));
    let failed = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(failed.state, "error");
    assert_eq!(failed.initial_total, 56);
    assert_eq!(failed.initial_confirmed, 50);
    assert!(failed.initialization_checkpoint.is_some());

    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = NULL")
        .execute(db.pool())
        .await
        .unwrap();
    let (restarted, _, _) = SyncRuntime::build(db, transport);
    restarted.sync_workspace(&workspace_id).await.unwrap();
    let completed = restarted
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(completed.state, "active");
    assert_eq!(completed.initial_confirmed, completed.initial_total);
}
