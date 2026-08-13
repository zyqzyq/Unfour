use super::*;

#[tokio::test]
async fn snapshot_external_apply_round_trip_preserves_tree_without_secrets_or_echo() {
    let source_db = database().await;
    let source = CommandBus::from_db(source_db.clone()).await.unwrap();
    let workspace_id = source.list_workspaces().await.unwrap().active_workspace_id;
    source
        .rename_workspace(workspace_id.clone(), "Source Workspace".to_string())
        .await
        .unwrap();
    let workspace = source
        .list_workspaces()
        .await
        .unwrap()
        .workspaces
        .into_iter()
        .find(|workspace| workspace.id == workspace_id)
        .unwrap();
    let collection = source
        .api_collection_create(workspace_id.clone(), "Commerce".to_string())
        .await
        .unwrap();
    let parent = source
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Orders".to_string(),
        )
        .await
        .unwrap();
    let child = source
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            Some(parent.id.clone()),
            "Refunds".to_string(),
        )
        .await
        .unwrap();
    let request = source
        .save_api_request(request_input(
            &workspace_id,
            &collection.id,
            Some(child.id.clone()),
        ))
        .await
        .unwrap();

    let collection_snapshot = api_collection_snapshot(&source, &workspace_id, &collection.id).await;
    let parent_snapshot = api_folder_snapshot(&source, &workspace_id, &parent.id).await;
    let child_snapshot = api_folder_snapshot(&source, &workspace_id, &child.id).await;
    let request_snapshot = api_request_snapshot(&source, &workspace_id, &request.id).await;

    let (target, target_db) = bus_with_hook(RecordingHook {
        local_only: true,
        fail_on: None,
    })
    .await;
    sqlx::query("DELETE FROM api_hook_effects")
        .execute(target_db.pool())
        .await
        .unwrap();
    let page = ExternalApplyPage {
        workspaces: vec![ExternalWorkspaceApply::Upsert(ExternalWorkspaceUpsert {
            id: workspace.id.clone(),
            name: workspace.name,
            environment_type: workspace.environment_type,
            mcp_policy: workspace.mcp_policy,
            created_at: workspace.created_at,
            updated_at: workspace.updated_at,
        })],
        api_collections: vec![collection_apply(&collection_snapshot)],
        // Intentionally child-before-parent: Core must topologically apply folders.
        api_folders: vec![
            folder_apply(&child_snapshot),
            folder_apply(&parent_snapshot),
        ],
        api_requests: vec![request_apply(&request_snapshot)],
        ..ExternalApplyPage::default()
    };
    let first = target.apply_external_page(page.clone()).await.unwrap();
    assert_eq!(first.applied_count, 5);
    assert!(first
        .mutations
        .iter()
        .all(|mutation| mutation.origin == MutationOrigin::External));
    let echo_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_hook_effects")
        .fetch_one(target_db.pool())
        .await
        .unwrap();
    assert_eq!(echo_count, 0);
    let second = target.apply_external_page(page).await.unwrap();
    assert_eq!(second.applied_count, 0, "external upsert must be a no-op");

    let mut target_collection =
        api_collection_snapshot(&target, &workspace_id, &collection.id).await;
    let mut target_parent = api_folder_snapshot(&target, &workspace_id, &parent.id).await;
    let mut target_child = api_folder_snapshot(&target, &workspace_id, &child.id).await;
    let mut target_request = api_request_snapshot(&target, &workspace_id, &request.id).await;
    let mut source_collection = collection_snapshot.clone();
    let mut source_parent = parent_snapshot.clone();
    let mut source_child = child_snapshot.clone();
    let mut source_request = request_snapshot.clone();
    source_collection.revision = 0;
    target_collection.revision = 0;
    source_parent.revision = 0;
    target_parent.revision = 0;
    source_child.revision = 0;
    target_child.revision = 0;
    source_request.revision = 0;
    target_request.revision = 0;
    assert_eq!(target_collection, source_collection);
    assert_eq!(target_parent, source_parent);
    assert_eq!(target_child, source_child);
    assert_eq!(target_request, source_request);

    let stored: (String, String, String, Option<String>, String) = sqlx::query_as(
        r#"
        SELECT auth_json, headers_json, query_json, body, url
        FROM api_requests WHERE id = ?1
        "#,
    )
    .bind(&request.id)
    .fetch_one(target_db.pool())
    .await
    .unwrap();
    let stored_serialized = serde_json::to_string(&stored).unwrap();
    for forbidden in [
        "auth-device-secret",
        "header-device-secret",
        "query-device-secret",
        "body-device-secret",
        "url-device-secret",
        "<redacted>",
    ] {
        assert!(
            !stored_serialized.contains(forbidden),
            "external apply stored secret marker/material: {forbidden}"
        );
    }

    let folder_delete = ExternalApiFolderApply::Delete(ExternalDelete {
        entity: DomainEntityKey::new(DomainEntityType::ApiFolder, &workspace_id, &parent.id)
            .with_parent_entity_id(&collection.id),
        deleted_at: "2026-08-12T12:00:00Z".to_string(),
    });
    let deleted = target
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![folder_delete.clone()],
            ..ExternalApplyPage::default()
        })
        .await
        .unwrap();
    assert_eq!(deleted.applied_count, 3);
    assert!(matches!(
        target
            .read_domain_snapshot(&DomainEntityKey::new(
                DomainEntityType::ApiRequest,
                &workspace_id,
                &request.id,
            ))
            .await
            .unwrap(),
        DomainSnapshot::Tombstone(_)
    ));
    let deleted_again = target
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![folder_delete],
            ..ExternalApplyPage::default()
        })
        .await
        .unwrap();
    assert_eq!(deleted_again.applied_count, 0);

    let collection_delete = ExternalApiCollectionApply::Delete(ExternalDelete {
        entity: DomainEntityKey::new(
            DomainEntityType::ApiCollection,
            &workspace_id,
            &collection.id,
        ),
        deleted_at: "2026-08-12T12:01:00Z".to_string(),
    });
    assert_eq!(
        target
            .apply_external_page(ExternalApplyPage {
                api_collections: vec![collection_delete.clone()],
                ..ExternalApplyPage::default()
            })
            .await
            .unwrap()
            .applied_count,
        1
    );
    assert_eq!(
        target
            .apply_external_page(ExternalApplyPage {
                api_collections: vec![collection_delete],
                ..ExternalApplyPage::default()
            })
            .await
            .unwrap()
            .applied_count,
        0
    );
}

#[tokio::test]
async fn external_apply_rejects_missing_or_cross_collection_parents_atomically() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Valid".to_string())
        .await
        .unwrap();
    let error = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![ExternalApiFolderApply::Upsert(ExternalApiFolderUpsert {
                id: "external-folder".to_string(),
                workspace_id: workspace_id.clone(),
                collection_id: collection.id,
                parent_folder_id: Some("missing-parent".to_string()),
                name: "Orphan".to_string(),
                sort_order: 0,
                created_at: "2026-08-12T00:00:00Z".to_string(),
                updated_at: "2026-08-12T00:00:00Z".to_string(),
            })],
            ..ExternalApplyPage::default()
        })
        .await
        .expect_err("missing external parent must be rejected");
    assert!(matches!(error, AppError::NotFound(_)));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM api_collection_folders WHERE id = 'external-folder'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
}

async fn api_collection_snapshot(
    bus: &CommandBus,
    workspace_id: &str,
    id: &str,
) -> ApiCollectionSnapshot {
    let snapshot = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::ApiCollection,
            workspace_id,
            id,
        ))
        .await
        .unwrap();
    let DomainSnapshot::ApiCollection(snapshot) = snapshot else {
        panic!("expected collection snapshot");
    };
    snapshot
}

async fn api_folder_snapshot(bus: &CommandBus, workspace_id: &str, id: &str) -> ApiFolderSnapshot {
    let snapshot = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::ApiFolder,
            workspace_id,
            id,
        ))
        .await
        .unwrap();
    let DomainSnapshot::ApiFolder(snapshot) = snapshot else {
        panic!("expected folder snapshot");
    };
    snapshot
}

async fn api_request_snapshot(
    bus: &CommandBus,
    workspace_id: &str,
    id: &str,
) -> ApiRequestSnapshot {
    let snapshot = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::ApiRequest,
            workspace_id,
            id,
        ))
        .await
        .unwrap();
    let DomainSnapshot::ApiRequest(snapshot) = snapshot else {
        panic!("expected request snapshot");
    };
    snapshot
}

fn collection_apply(snapshot: &ApiCollectionSnapshot) -> ExternalApiCollectionApply {
    ExternalApiCollectionApply::Upsert(ExternalApiCollectionUpsert {
        id: snapshot.id.clone(),
        workspace_id: snapshot.workspace_id.clone(),
        name: snapshot.name.clone(),
        description: snapshot.description.clone(),
        created_at: snapshot.created_at.clone(),
        updated_at: snapshot.updated_at.clone(),
    })
}

fn folder_apply(snapshot: &ApiFolderSnapshot) -> ExternalApiFolderApply {
    ExternalApiFolderApply::Upsert(ExternalApiFolderUpsert {
        id: snapshot.id.clone(),
        workspace_id: snapshot.workspace_id.clone(),
        collection_id: snapshot.collection_id.clone(),
        parent_folder_id: snapshot.parent_folder_id.clone(),
        name: snapshot.name.clone(),
        sort_order: snapshot.sort_order,
        created_at: snapshot.created_at.clone(),
        updated_at: snapshot.updated_at.clone(),
    })
}

fn request_apply(snapshot: &ApiRequestSnapshot) -> ExternalApiRequestApply {
    ExternalApiRequestApply::Upsert(Box::new(ExternalApiRequestUpsert {
        id: snapshot.id.clone(),
        workspace_id: snapshot.workspace_id.clone(),
        collection_id: snapshot.collection_id.clone(),
        parent_folder_id: snapshot.parent_folder_id.clone(),
        name: snapshot.name.clone(),
        sort_order: snapshot.sort_order,
        auth_json: snapshot.auth_json.clone(),
        method: snapshot.method.clone(),
        url: snapshot.url.clone(),
        headers: snapshot.headers.clone(),
        query: snapshot.query.clone(),
        body: snapshot.body.clone(),
        body_kind: snapshot.body_kind.clone(),
        pre_request_script: snapshot.pre_request_script.clone(),
        post_response_script: snapshot.post_response_script.clone(),
        script_schema_version: snapshot.script_schema_version,
        created_at: snapshot.created_at.clone(),
        updated_at: snapshot.updated_at.clone(),
    }))
}
