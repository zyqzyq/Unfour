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
    // Shape-aware auth redaction is asymmetric on a first sync: the producer
    // marks its local secret material with `<redacted>` while the receiver
    // has no local material to restore and stores empty strings. Compare the
    // auth payload structurally and everything else byte-for-byte.
    let source_auth: serde_json::Value = serde_json::from_str(&source_request.auth_json).unwrap();
    let target_auth: serde_json::Value = serde_json::from_str(&target_request.auth_json).unwrap();
    assert_eq!(source_auth["type"], "bearer");
    assert_eq!(source_auth["token"], "<redacted>");
    assert_eq!(target_auth["type"], "bearer");
    assert_eq!(target_auth["token"], "");
    assert_eq!(target_auth["prefix"], "");
    source_request.auth_json = String::new();
    target_request.auth_json = String::new();
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
async fn external_apply_skips_upserts_under_absent_or_deleted_parents() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let deleted = bus
        .api_collection_create(workspace_id.clone(), "Doomed".to_string())
        .await
        .unwrap();
    bus.api_collection_delete(workspace_id.clone(), deleted.id.clone())
        .await
        .unwrap();

    // Folder upsert under a soft-deleted collection: skipped without a row or
    // a mutation. The server cascades deletes, so the folder is guaranteed to
    // be tombstoned at a later cursor.
    let report = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![external_folder(
                "orphan-folder",
                &workspace_id,
                &deleted.id,
                None,
                "Orphan",
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .expect("doomed orphan folder must be skipped, not fail the page");
    assert_eq!(report.applied_count, 0);
    assert!(report.mutations.is_empty());
    assert_eq!(folder_row_count(&db, "orphan-folder").await, 0);

    // Nested batch listing the child before its in-batch parent: the child is
    // deferred, the parent is skipped, and the deferred child cascades into a
    // skip instead of a "cyclic or unavailable parent" error.
    let report = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![
                external_folder(
                    "folder-b",
                    &workspace_id,
                    &deleted.id,
                    Some("folder-a"),
                    "B",
                ),
                external_folder("folder-a", &workspace_id, &deleted.id, None, "A"),
            ],
            ..ExternalApplyPage::default()
        })
        .await
        .expect("nested doomed orphans must be skipped");
    assert_eq!(report.applied_count, 0);
    assert_eq!(folder_row_count(&db, "folder-a").await, 0);
    assert_eq!(folder_row_count(&db, "folder-b").await, 0);

    // Live collection but absent parent folder: the folder upsert and the
    // request targeting the same absent folder are both skipped.
    let live = bus
        .api_collection_create(workspace_id.clone(), "Live".to_string())
        .await
        .unwrap();
    let report = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![external_folder(
                "under-missing",
                &workspace_id,
                &live.id,
                Some("missing-parent"),
                "Orphan",
            )],
            api_requests: vec![external_request(
                "request-under-missing",
                &workspace_id,
                &live.id,
                Some("missing-parent"),
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .expect("upserts under an absent parent must be skipped");
    assert_eq!(report.applied_count, 0);
    assert_eq!(folder_row_count(&db, "under-missing").await, 0);
    assert_eq!(request_row_count(&db, "request-under-missing").await, 0);

    // Request upsert directly under the soft-deleted collection: skipped.
    let report = bus
        .apply_external_page(ExternalApplyPage {
            api_requests: vec![external_request(
                "request-under-deleted",
                &workspace_id,
                &deleted.id,
                None,
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .expect("request under a deleted collection must be skipped");
    assert_eq!(report.applied_count, 0);
    assert_eq!(request_row_count(&db, "request-under-deleted").await, 0);
}

#[tokio::test]
async fn external_apply_keeps_cross_collection_parent_as_hard_error() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let collection_a = bus
        .api_collection_create(workspace_id.clone(), "A".to_string())
        .await
        .unwrap();
    let collection_b = bus
        .api_collection_create(workspace_id.clone(), "B".to_string())
        .await
        .unwrap();
    let parent = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection_a.id.clone(),
            None,
            "Parent".to_string(),
        )
        .await
        .unwrap();
    let error = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![external_folder(
                "cross-collection",
                &workspace_id,
                &collection_b.id,
                Some(parent.id.as_str()),
                "Cross",
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .expect_err("cross-collection parents must stay rejected");
    assert!(matches!(error, AppError::Validation(_)));
    assert_eq!(folder_row_count(&db, "cross-collection").await, 0);
}

#[tokio::test]
async fn external_apply_is_lenient_on_names_but_rejects_blank_ones() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Live".to_string())
        .await
        .unwrap();

    // Strict-producer / lenient-consumer: the server enforces name limits at
    // push time, so a receiver must apply names local validation would reject.
    let oversized = format!("{}<>{}", "a".repeat(150), "b".repeat(150));
    let report = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![external_folder(
                "lenient-folder",
                &workspace_id,
                &collection.id,
                None,
                &oversized,
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .expect("external apply must not reject long names or special characters");
    assert_eq!(report.applied_count, 1);
    let stored: String =
        sqlx::query_scalar("SELECT name FROM api_collection_folders WHERE id = 'lenient-folder'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stored, oversized);

    let error = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![external_folder(
                "blank-folder",
                &workspace_id,
                &collection.id,
                None,
                "   ",
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .expect_err("blank external names must stay rejected");
    assert!(matches!(error, AppError::Validation(_)));
}

#[tokio::test]
async fn external_folder_upsert_cannot_move_live_folder_across_collections() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let original = bus
        .api_collection_create(workspace_id.clone(), "Original".to_string())
        .await
        .unwrap();
    let other = bus
        .api_collection_create(workspace_id.clone(), "Other".to_string())
        .await
        .unwrap();
    let folder = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            original.id.clone(),
            None,
            "Payments".to_string(),
        )
        .await
        .unwrap();

    let error = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![external_folder(
                folder.id.as_str(),
                &workspace_id,
                &other.id,
                None,
                "Payments",
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .expect_err("a live folder must not change collection");
    assert!(matches!(error, AppError::Validation(_)));

    // Resurrecting a soft-deleted row under a new collection is an external
    // re-create, not a move, and stays allowed.
    bus.api_collection_folder_delete(workspace_id.clone(), folder.id.clone())
        .await
        .unwrap();
    let report = bus
        .apply_external_page(ExternalApplyPage {
            api_folders: vec![external_folder(
                folder.id.as_str(),
                &workspace_id,
                &other.id,
                None,
                "Payments",
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .expect("resurrecting a deleted folder under a new collection must apply");
    assert_eq!(report.applied_count, 1);
    let (collection_id, deleted_at): (String, Option<String>) = sqlx::query_as(
        "SELECT collection_id, deleted_at FROM api_collection_folders WHERE id = ?1",
    )
    .bind(&folder.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(collection_id, other.id);
    assert!(deleted_at.is_none());
}

fn external_folder(
    id: &str,
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<&str>,
    name: &str,
) -> ExternalApiFolderApply {
    ExternalApiFolderApply::Upsert(ExternalApiFolderUpsert {
        id: id.to_string(),
        workspace_id: workspace_id.to_string(),
        collection_id: collection_id.to_string(),
        parent_folder_id: parent_folder_id.map(str::to_string),
        name: name.to_string(),
        sort_order: 0,
        created_at: "2026-08-12T00:00:00Z".to_string(),
        updated_at: "2026-08-12T00:00:00Z".to_string(),
    })
}

fn external_request(
    id: &str,
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<&str>,
) -> ExternalApiRequestApply {
    ExternalApiRequestApply::Upsert(Box::new(ExternalApiRequestUpsert {
        id: id.to_string(),
        workspace_id: workspace_id.to_string(),
        collection_id: collection_id.to_string(),
        parent_folder_id: parent_folder_id.map(str::to_string),
        name: format!("Request {id}"),
        sort_order: 0,
        auth_json: "<redacted>".to_string(),
        method: "GET".to_string(),
        url: "https://api.example.test/resource".to_string(),
        headers: Vec::new(),
        query: Vec::new(),
        body: None,
        body_kind: "none".to_string(),
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        created_at: "2026-08-12T00:00:00Z".to_string(),
        updated_at: "2026-08-12T00:00:00Z".to_string(),
    }))
}

async fn folder_row_count(db: &LocalDb, id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM api_collection_folders WHERE id = ?1")
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn request_row_count(db: &LocalDb, id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM api_requests WHERE id = ?1")
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap()
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
