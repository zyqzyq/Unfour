use super::*;

#[tokio::test]
async fn api_hook_failure_rolls_back_business_row_activity_and_hook_effects() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: false,
        fail_on: Some("api.collection.create"),
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let error = bus
        .api_collection_create(workspace_id, "Rollback".to_string())
        .await
        .expect_err("hook must reject API collection creation");
    assert!(error.to_string().contains("hook rejected"));
    let collection_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_collections")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let activity_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action = 'api.collection.create'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let hook_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(collection_count, 0);
    assert_eq!(activity_count, 0);
    assert_eq!(hook_count, 0);
}
