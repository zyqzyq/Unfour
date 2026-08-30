use super::support::*;
use unfour_cloud_sync::SyncService;

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
        deleted_at: Some("2026-07-28T02:00:00Z".into()),
    }
}

struct EnvironmentConflictFixture {
    db: LocalDb,
    service: SyncService,
    transport: Arc<MockTransport>,
    workspace_id: String,
    environment_id: String,
    variable_id: String,
    base_cursor: i64,
    original_variable_operation_id: String,
}

async fn environment_delete_conflict(outbox_status: &str) -> EnvironmentConflictFixture {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let environment = seed
        .workspace_environment_create(workspace_id.clone(), "Local environment".into())
        .await
        .unwrap();
    let environment_variable = seed
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            variable(None, "HOST", "before", false),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_environment_variable_update(
        workspace_id.clone(),
        environment.id.clone(),
        environment_variable.id.clone(),
        variable(
            Some(environment_variable.id.clone()),
            "HOST",
            "local-change",
            false,
        ),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE cloud_sync_outbox SET status = ?1 WHERE entity_id = ?2")
        .bind(outbox_status)
        .bind(&environment_variable.id)
        .execute(db.pool())
        .await
        .unwrap();
    let original_variable_operation_id: String =
        sqlx::query_scalar("SELECT operation_id FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&environment_variable.id)
            .fetch_one(db.pool())
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
        current_cursor: base_cursor + 2,
        next_cursor: base_cursor + 2,
        changes: vec![
            remote_delete_change(
                base_cursor + 1,
                "remote-delete-environment",
                SyncEntityType::WorkspaceEnvironment,
                &environment.id,
                Some(&workspace_id),
                7,
            ),
            remote_delete_change(
                base_cursor + 2,
                "remote-delete-environment-variable",
                SyncEntityType::WorkspaceEnvironmentVariable,
                &environment_variable.id,
                Some(&environment.id),
                5,
            ),
        ],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));

    EnvironmentConflictFixture {
        db,
        service,
        transport,
        workspace_id,
        environment_id: environment.id,
        variable_id: environment_variable.id,
        base_cursor,
        original_variable_operation_id,
    }
}

#[tokio::test]
async fn remote_environment_delete_is_scoped_over_local_descendants_and_survives_restart() {
    let fixture = environment_delete_conflict("pending").await;
    let environment_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspace_environments WHERE id = ?1")
            .bind(&fixture.environment_id)
            .fetch_one(fixture.db.pool())
            .await
            .unwrap();
    let variable: (String, Option<String>) = sqlx::query_as(
        "SELECT value, deleted_at FROM workspace_environment_variables WHERE id = ?1",
    )
    .bind(&fixture.variable_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(environment_deleted, None);
    assert_eq!(variable, ("local-change".into(), None));

    let stored: (String, i64, String, Option<String>) = sqlx::query_as(
        r#"SELECT sync_status, server_version, conflict_operation_id, conflict_deleted_at
           FROM cloud_sync_entity_state
           WHERE entity_type = 'workspaceEnvironment' AND entity_id = ?1"#,
    )
    .bind(&fixture.environment_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(
        stored,
        (
            "conflict".into(),
            7,
            "remote-delete-environment".into(),
            Some("2026-07-28T02:00:00Z".into()),
        )
    );
    assert_eq!(
        fixture
            .service
            .status(&fixture.workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        fixture.base_cursor + 2
    );

    let (restarted, _, _) = SyncRuntime::build(fixture.db.clone(), fixture.transport.clone());
    assert!(matches!(
        restarted.sync_workspace(&fixture.workspace_id).await,
        Err(SyncError::Conflict)
    ));
    assert_eq!(
        restarted
            .conflicts(&fixture.workspace_id)
            .await
            .unwrap()
            .len(),
        2
    );

    restarted
        .use_remote(
            &fixture.workspace_id,
            SyncEntityType::WorkspaceEnvironment,
            &fixture.environment_id,
        )
        .await
        .unwrap();
    let deleted: (Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT environment.deleted_at, variable.deleted_at
           FROM workspace_environments AS environment
           JOIN workspace_environment_variables AS variable
             ON variable.environment_id = environment.id
           WHERE environment.id = ?1 AND variable.id = ?2"#,
    )
    .bind(&fixture.environment_id)
    .bind(&fixture.variable_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(
        deleted,
        (
            Some("2026-07-28T02:00:00Z".into()),
            Some("2026-07-28T02:00:00Z".into())
        )
    );
    let remaining: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM cloud_sync_outbox), (SELECT COUNT(*) FROM cloud_sync_entity_state WHERE sync_status = 'conflict')",
    )
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(remaining, (0, 0));
}

#[tokio::test]
async fn remote_parent_delete_blocks_on_every_durable_outbox_state() {
    for status in ["uncertain", "in_flight", "dead"] {
        let fixture = environment_delete_conflict(status).await;
        let environment_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM workspace_environments WHERE id = ?1")
                .bind(&fixture.environment_id)
                .fetch_one(fixture.db.pool())
                .await
                .unwrap();
        assert_eq!(environment_deleted, None, "outbox status {status}");
        assert!(fixture
            .service
            .conflicts(&fixture.workspace_id)
            .await
            .unwrap()
            .iter()
            .any(|conflict| {
                conflict.entity_type == "workspaceEnvironment"
                    && conflict.entity_id == fixture.environment_id
            }));
    }
}

#[tokio::test]
async fn keep_local_recreates_parent_and_child_intents_with_remote_base_versions() {
    let fixture = environment_delete_conflict("pending").await;
    fixture
        .service
        .keep_local(
            &fixture.workspace_id,
            SyncEntityType::WorkspaceEnvironment,
            &fixture.environment_id,
        )
        .await
        .unwrap();

    let pushes = fixture.transport.pushes.lock().unwrap();
    let resolution = pushes.last().unwrap();
    assert_eq!(resolution.operations.len(), 2);
    assert_eq!(
        resolution.operations[0].entity_type,
        SyncEntityType::WorkspaceEnvironment
    );
    assert_eq!(resolution.operations[0].base_version, 7);
    assert_eq!(resolution.operations[0].operation, SyncOperation::Upsert);
    assert_eq!(
        resolution.operations[1].entity_type,
        SyncEntityType::WorkspaceEnvironmentVariable
    );
    assert_eq!(resolution.operations[1].base_version, 5);
    assert_eq!(resolution.operations[1].operation, SyncOperation::Upsert);
    assert_ne!(
        resolution.operations[1].operation_id,
        fixture.original_variable_operation_id
    );
    drop(pushes);

    let live: (Option<String>, String, Option<String>) = sqlx::query_as(
        r#"SELECT environment.deleted_at, variable.value, variable.deleted_at
           FROM workspace_environments AS environment
           JOIN workspace_environment_variables AS variable
             ON variable.environment_id = environment.id
           WHERE environment.id = ?1 AND variable.id = ?2"#,
    )
    .bind(&fixture.environment_id)
    .bind(&fixture.variable_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(live, (None, "local-change".into(), None));
    assert!(fixture
        .service
        .conflicts(&fixture.workspace_id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn remote_workspace_delete_conflicts_with_any_descendant_and_keep_local_pushes_root_first() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let local = seed
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "ROOT_CHILD", "before", false),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_variable_update(
        workspace_id.clone(),
        local.id.clone(),
        variable(Some(local.id.clone()), "ROOT_CHILD", "local-change", false),
    )
    .await
    .unwrap();
    let cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor + 1,
        next_cursor: cursor + 1,
        changes: vec![remote_delete_change(
            cursor + 1,
            "remote-delete-workspace",
            SyncEntityType::Workspace,
            &workspace_id,
            None,
            9,
        )],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    let deleted_at: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspaces WHERE id = ?1")
            .bind(&workspace_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(deleted_at, None);
    assert_eq!(
        service.conflicts(&workspace_id).await.unwrap()[0].entity_type,
        "workspace"
    );

    service
        .keep_local(&workspace_id, SyncEntityType::Workspace, &workspace_id)
        .await
        .unwrap();
    let pushes = transport.pushes.lock().unwrap();
    let resolution = pushes.last().unwrap();
    assert_eq!(resolution.operations.len(), 2);
    assert_eq!(
        resolution.operations[0].entity_type,
        SyncEntityType::Workspace
    );
    assert_eq!(resolution.operations[0].operation, SyncOperation::Upsert);
    assert_eq!(resolution.operations[0].base_version, 9);
    assert_eq!(
        resolution.operations[1].entity_type,
        SyncEntityType::WorkspaceVariable
    );
    drop(pushes);

    bus.workspace_variable_update(
        workspace_id.clone(),
        local.id.clone(),
        variable(
            Some(local.id.clone()),
            "ROOT_CHILD",
            "abandoned-local-change",
            false,
        ),
    )
    .await
    .unwrap();
    let cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor + 1,
        next_cursor: cursor + 1,
        changes: vec![remote_delete_change(
            cursor + 1,
            "remote-delete-workspace-use-remote",
            SyncEntityType::Workspace,
            &workspace_id,
            None,
            11,
        )],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    service
        .use_remote(&workspace_id, SyncEntityType::Workspace, &workspace_id)
        .await
        .unwrap();
    let accepted: (Option<String>, i64, i64, i64) = sqlx::query_as(
        r#"SELECT deleted_at,
                  (SELECT COUNT(*) FROM workspaces WHERE deleted_at IS NULL),
                  (SELECT COUNT(*) FROM cloud_sync_outbox),
                  (SELECT COUNT(*) FROM cloud_sync_entity_state WHERE sync_status = 'conflict')
           FROM workspaces WHERE id = ?1"#,
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(accepted, (Some("2026-07-28T02:00:00Z".into()), 1, 0, 0));
}

#[tokio::test]
async fn failed_external_apply_rolls_back_hierarchy_conflict_and_cursor_before_restart() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let environment = seed
        .workspace_environment_create(workspace_id.clone(), "Atomic".into())
        .await
        .unwrap();
    let local = seed
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            variable(None, "ATOMIC", "before", false),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_environment_variable_update(
        workspace_id.clone(),
        environment.id.clone(),
        local.id.clone(),
        variable(Some(local.id.clone()), "ATOMIC", "local-change", false),
    )
    .await
    .unwrap();
    let base_cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    // Orphan env-var upserts now skip instead of failing the page, so use an
    // environment upsert with an empty name as the apply poison pill.
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base_cursor + 2,
        next_cursor: base_cursor + 2,
        changes: vec![
            remote_delete_change(
                base_cursor + 1,
                "remote-delete-atomic-environment",
                SyncEntityType::WorkspaceEnvironment,
                &environment.id,
                Some(&workspace_id),
                4,
            ),
            RemoteChange {
                cursor: base_cursor + 2,
                operation_id: "invalid-environment-upsert".into(),
                entity_type: SyncEntityType::WorkspaceEnvironment,
                entity_id: "invalid-environment".into(),
                parent_entity_id: Some(workspace_id.clone()),
                operation: SyncOperation::Upsert,
                server_version: 1,
                payload_schema_version: 1,
                payload: Some(serde_json::json!({
                    "name": "",
                    "sortOrder": 0,
                    "createdAt": "2026-07-28T00:00:00Z",
                    "updatedAt": "2026-07-28T00:00:00Z",
                    "deletedAt": null
                })),
                deleted_at: None,
            },
        ],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Core)
    ));
    let rolled_back: (i64, i64, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT binding.last_pulled_cursor,
                  (SELECT COUNT(*) FROM cloud_sync_entity_state WHERE sync_status = 'conflict'),
                  environment.deleted_at, variable.deleted_at
           FROM cloud_sync_workspace_bindings AS binding
           JOIN workspace_environments AS environment ON environment.id = ?1
           JOIN workspace_environment_variables AS variable ON variable.id = ?2
           WHERE binding.local_workspace_id = ?3"#,
    )
    .bind(&environment.id)
    .bind(&local.id)
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rolled_back, (base_cursor, 0, None, None));

    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base_cursor + 1,
        next_cursor: base_cursor + 1,
        changes: vec![remote_delete_change(
            base_cursor + 1,
            "remote-delete-atomic-environment",
            SyncEntityType::WorkspaceEnvironment,
            &environment.id,
            Some(&workspace_id),
            4,
        )],
    });
    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport);
    assert!(matches!(
        restarted.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    let after_restart = restarted.status(&workspace_id).await.unwrap();
    assert_eq!(
        after_restart.binding.unwrap().last_pulled_cursor,
        base_cursor + 1
    );
    assert_eq!(after_restart.conflict_count, 1);
}

/// A remote workspace aggregate delete conflicting with pending local API
/// edits used to abort in `repository::keep_local` because
/// `entity_revision_on`/`entity_is_deleted_on` did not know the API tables.
#[tokio::test]
async fn keep_local_on_workspace_delete_conflict_repushes_api_descendants() {
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
        .api_collection_create(workspace_id.clone(), "Original".into())
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();

    bus.api_collection_rename(
        workspace_id.clone(),
        collection.id.clone(),
        "Local rename".into(),
    )
    .await
    .unwrap();
    let base_cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    // Remote aggregate delete: children first, then the workspace root.
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base_cursor + 2,
        next_cursor: base_cursor + 2,
        changes: vec![
            remote_delete_change(
                base_cursor + 1,
                "remote-aggregate-delete",
                SyncEntityType::ApiCollection,
                &collection.id,
                Some(&workspace_id),
                4,
            ),
            remote_delete_change(
                base_cursor + 2,
                "remote-aggregate-delete",
                SyncEntityType::Workspace,
                &workspace_id,
                None,
                7,
            ),
        ],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    assert_eq!(
        service.status(&workspace_id).await.unwrap().conflict_count,
        2
    );

    service
        .keep_local(&workspace_id, SyncEntityType::Workspace, &workspace_id)
        .await
        .unwrap();
    let repushed = transport
        .pushes
        .lock()
        .unwrap()
        .iter()
        .flat_map(|push| &push.operations)
        .filter(|operation| operation.entity_id == collection.id)
        .next_back()
        .cloned()
        .expect("keep_local must re-push the API collection");
    assert_eq!(repushed.base_version, 4);
    assert_eq!(repushed.payload.as_ref().unwrap()["name"], "Local rename");
    let resolved = service.status(&workspace_id).await.unwrap();
    assert_eq!(resolved.conflict_count, 0);
    assert_eq!(resolved.dead_count, 0);
    assert_eq!(resolved.binding.unwrap().state, "active");
    let survived: (Option<String>, Option<String>, String) = sqlx::query_as(
        r#"SELECT
             (SELECT deleted_at FROM workspaces WHERE id = ?1),
             (SELECT deleted_at FROM api_collections WHERE id = ?2),
             (SELECT name FROM api_collections WHERE id = ?2)"#,
    )
    .bind(&workspace_id)
    .bind(&collection.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(survived, (None, None, "Local rename".into()));
}

#[tokio::test]
async fn pull_skips_environment_variable_orphans_under_deleted_parents() {
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
            operation_id: "orphan-env-var".into(),
            entity_type: SyncEntityType::WorkspaceEnvironmentVariable,
            entity_id: "orphan-1".into(),
            parent_entity_id: Some("missing-environment".into()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: 1,
            payload: Some(variable_payload("orphan")),
            deleted_at: None,
        }],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let advanced: (i64, i64) = sqlx::query_as(
        r#"SELECT binding.last_pulled_cursor,
                  (SELECT COUNT(*) FROM workspace_environment_variables WHERE id = 'orphan-1')
           FROM cloud_sync_workspace_bindings AS binding
           WHERE binding.local_workspace_id = ?1"#,
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(advanced, (base_cursor + 1, 0));
}

#[tokio::test]
async fn pull_workspace_delete_cascades_live_local_descendants() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let collection = seed
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    let environment = seed
        .workspace_environment_create(workspace_id.clone(), "Development".into())
        .await
        .unwrap();
    let variable = seed
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            variable(None, "TOKEN", "value", false),
        )
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();
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
            "remote-workspace-delete",
            SyncEntityType::Workspace,
            &workspace_id,
            None,
            7,
        )],
    });
    service.sync_workspace(&workspace_id).await.unwrap();
    let tombstoned: (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
    ) = sqlx::query_as(
        r#"SELECT
                 (SELECT deleted_at FROM workspaces WHERE id = ?1),
                 (SELECT deleted_at FROM api_collections WHERE id = ?2),
                 (SELECT deleted_at FROM workspace_environments WHERE id = ?3),
                 (SELECT deleted_at FROM workspace_environment_variables WHERE id = ?4),
                 (SELECT last_pulled_cursor FROM cloud_sync_workspace_bindings
                  WHERE local_workspace_id = ?1)"#,
    )
    .bind(&workspace_id)
    .bind(&collection.id)
    .bind(&environment.id)
    .bind(&variable.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(tombstoned.0.is_some());
    assert!(tombstoned.1.is_some());
    assert!(tombstoned.2.is_some());
    assert!(tombstoned.3.is_some());
    assert_eq!(tombstoned.4, base_cursor + 1);
}
