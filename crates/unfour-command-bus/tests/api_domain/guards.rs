use super::*;
use unfour_core::network::{ApiRoute, ApiRouteEdit};

async fn activity_and_hook_counts(db: &LocalDb) -> (i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM activity_events), (SELECT COUNT(*) FROM api_hook_effects)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap()
}

async fn saved_requests(bus: &CommandBus, workspace_id: &str) -> serde_json::Value {
    serde_json::to_value(
        bus.list_saved_api_requests(workspace_id.to_string())
            .await
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn stale_request_revision_preserves_current_definition_activity_and_hooks() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: false,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Users".into())
        .await
        .unwrap();
    let input = request_input(&workspace_id, &collection.id, None);
    let saved = bus.save_api_request(input.clone()).await.unwrap();
    let mut newer = input.clone();
    newer.name = Some("Updated elsewhere".into());
    let current = bus
        .update_api_request_if_current(
            workspace_id.clone(),
            saved.id.clone(),
            newer,
            Some(saved.revision),
        )
        .await
        .unwrap();
    assert_eq!(current.revision, saved.revision + 1);
    let before = saved_requests(&bus, &workspace_id).await;
    let counts = activity_and_hook_counts(&db).await;
    let mut stale = input;
    stale.method = "DELETE".into();

    let error = bus
        .update_api_request_if_current(workspace_id.clone(), saved.id, stale, Some(saved.revision))
        .await
        .unwrap_err();

    assert!(matches!(error, AppError::RowConflict(_)));
    assert_eq!(saved_requests(&bus, &workspace_id).await, before);
    assert_eq!(activity_and_hook_counts(&db).await, counts);
}

#[tokio::test]
async fn changed_folder_subtree_rejects_delete_without_activity_or_hooks() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: false,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Users".into())
        .await
        .unwrap();
    let root = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Root".into(),
        )
        .await
        .unwrap();
    let child = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            Some(root.id.clone()),
            "Child".into(),
        )
        .await
        .unwrap();
    let mut input = request_input(&workspace_id, &collection.id, Some(child.id));
    let saved = bus.save_api_request(input.clone()).await.unwrap();
    let confirmed = bus
        .api_folder_delete_snapshot(workspace_id.clone(), root.id.clone())
        .await
        .unwrap();
    input.name = Some("Changed after confirmation".into());
    bus.update_api_request(workspace_id.clone(), saved.id, input)
        .await
        .unwrap();
    let current = bus
        .api_folder_delete_snapshot(workspace_id.clone(), root.id.clone())
        .await
        .unwrap();
    assert_ne!(confirmed, current);
    let requests = saved_requests(&bus, &workspace_id).await;
    let counts = activity_and_hook_counts(&db).await;

    let error = bus
        .api_collection_folder_delete_checked(
            workspace_id.clone(),
            root.id.clone(),
            Some(confirmed),
        )
        .await
        .unwrap_err();

    assert!(matches!(error, AppError::RowConflict(_)));
    assert_eq!(
        bus.api_folder_delete_snapshot(workspace_id.clone(), root.id)
            .await
            .unwrap(),
        current
    );
    assert_eq!(saved_requests(&bus, &workspace_id).await, requests);
    assert_eq!(activity_and_hook_counts(&db).await, counts);
}

#[tokio::test]
async fn invalid_routed_save_rolls_back_request_default_collection_activity_and_hooks() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: false,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = request_input(&workspace_id, "unused", None);
    input.collection_id = None;
    let counts = activity_and_hook_counts(&db).await;

    let error = bus
        .save_api_request_routed(
            input,
            Some(ApiRoute::Ssh {
                ssh_connection_id: "missing-jump".into(),
            }),
        )
        .await
        .unwrap_err();

    assert_eq!(error.code(), "SSH_TUNNEL_ROUTE_MISSING");
    let rows: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM api_collections),
                (SELECT COUNT(*) FROM api_requests),
                (SELECT COUNT(*) FROM api_request_local_routes)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rows, (0, 0, 0));
    assert_eq!(activity_and_hook_counts(&db).await, counts);
}

#[tokio::test]
async fn rejected_route_edit_rolls_back_changed_request_revision_activity_and_hooks() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: false,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Users".into())
        .await
        .unwrap();
    let mut input = request_input(&workspace_id, &collection.id, None);
    let saved = bus
        .save_api_request_routed(input.clone(), Some(ApiRoute::Direct))
        .await
        .unwrap();
    let before = saved_requests(&bus, &workspace_id).await;
    let counts = activity_and_hook_counts(&db).await;
    input.name = Some("Must roll back".into());
    input.method = "PATCH".into();

    for (edit, code) in [
        (
            ApiRouteEdit {
                expected: ApiRoute::Inherit,
                route: ApiRoute::Inherit,
            },
            "SSH_TUNNEL_ROUTE_CHANGED",
        ),
        (
            ApiRouteEdit {
                expected: ApiRoute::Direct,
                route: ApiRoute::Ssh {
                    ssh_connection_id: "missing-jump".into(),
                },
            },
            "SSH_TUNNEL_ROUTE_MISSING",
        ),
    ] {
        let error = bus
            .update_api_request_routed(
                workspace_id.clone(),
                saved.id.clone(),
                input.clone(),
                Some(saved.revision),
                Some(edit),
            )
            .await
            .unwrap_err();

        assert_eq!(error.code(), code);
        assert_eq!(saved_requests(&bus, &workspace_id).await, before);
        assert_eq!(activity_and_hook_counts(&db).await, counts);
        assert_eq!(
            bus.api_routes(&workspace_id, Some(&saved.id), None)
                .await
                .unwrap()
                .request,
            ApiRoute::Direct
        );
    }
}
