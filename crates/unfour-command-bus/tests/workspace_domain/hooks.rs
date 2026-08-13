use super::*;

#[tokio::test]
async fn hook_failure_rolls_back_domain_activity_and_hook_sql() {
    let hook = Arc::new(SqlHook {
        name: "rejecting",
        fail_on: Some("workspace.create"),
        local_only: false,
    });
    let (bus, db) = bus_with_hooks(vec![hook]).await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(db.pool())
        .await
        .unwrap();

    let error = bus
        .create_workspace("Must Roll Back".to_string())
        .await
        .expect_err("hook should reject command");
    assert!(error.to_string().contains("rejecting"));

    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let activity: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_events WHERE action = 'workspace.create'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let hook_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(after, before);
    assert_eq!(activity, 0);
    assert_eq!(hook_rows, 0);
}

#[tokio::test]
async fn later_hook_failure_rolls_back_earlier_hook_sql() {
    let first = Arc::new(SqlHook {
        name: "first",
        fail_on: None,
        local_only: false,
    });
    let second = Arc::new(SqlHook {
        name: "second",
        fail_on: Some("workspace.create"),
        local_only: false,
    });
    let (bus, db) = bus_with_hooks(vec![first, second]).await;

    bus.create_workspace("Rollback Both Hooks".to_string())
        .await
        .expect_err("second hook should reject command");
    let hook_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(hook_rows, 0);
}

#[tokio::test]
async fn community_without_hooks_keeps_normal_behavior_and_activity() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace = bus
        .create_workspace("Community".to_string())
        .await
        .expect("create without hooks");
    assert_eq!(workspace.name, "Community");
    let activity: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT workspace_id, target FROM activity_events WHERE action = 'workspace.create'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(activity.0.as_deref(), Some(workspace.id.as_str()));
    assert_eq!(activity.1.as_deref(), Some(workspace.id.as_str()));
}

#[tokio::test]
async fn entity_create_activities_include_generated_targets() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let variable = bus
        .workspace_variable_create(
            workspace_id.clone(),
            input(None, "BASE_URL", "https://example.test", false),
        )
        .await
        .unwrap();
    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Development".to_string())
        .await
        .unwrap();
    let environment_variable = bus
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            input(None, "TOKEN", "secret", true),
        )
        .await
        .unwrap();

    for (action, expected_target) in [
        ("workspace.variable.create", variable.id),
        ("workspace.environment.create", environment.id),
        (
            "workspace.environment_variable.create",
            environment_variable.id,
        ),
    ] {
        let activity: (Option<String>, Option<String>) =
            sqlx::query_as("SELECT workspace_id, target FROM activity_events WHERE action = ?1")
                .bind(action)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(activity.0.as_deref(), Some(workspace_id.as_str()));
        assert_eq!(activity.1.as_deref(), Some(expected_target.as_str()));
    }
}
