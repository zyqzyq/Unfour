use super::*;

#[tokio::test]
async fn environment_selection_has_no_mutation_and_delete_is_atomic_with_children() {
    let hook = Arc::new(SqlHook {
        name: "conditional",
        fail_on: Some("workspace.environment.delete"),
        local_only: false,
    });
    let (bus, db) = bus_with_hooks(vec![hook]).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Development".to_string())
        .await
        .unwrap();
    let environment = bus
        .workspace_environment_update(
            workspace_id.clone(),
            environment.id.clone(),
            environment.name,
            vec![input(None, "HOST", "localhost", false)],
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM hook_effects")
        .execute(db.pool())
        .await
        .unwrap();
    bus.workspace_environment_set_active(workspace_id.clone(), None)
        .await
        .unwrap();
    let selection_mutations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(selection_mutations, 0);

    bus.workspace_environment_delete(workspace_id.clone(), environment.id.clone())
        .await
        .expect_err("hook should roll back environment deletion");
    let environment_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspace_environments WHERE id = ?1")
            .bind(&environment.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let child_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspace_environment_variables WHERE id = ?1")
            .bind(&environment.variables[0].id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(environment_deleted.is_none());
    assert!(child_deleted.is_none());
}

#[tokio::test]
async fn legacy_environment_api_uses_the_same_coordinator() {
    let capture = Arc::new(SqlHook {
        name: "capture",
        fail_on: None,
        local_only: false,
    });
    let (bus, db) = bus_with_hooks(vec![capture]).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .api_environment_create(workspace_id.clone(), "Legacy".to_string())
        .await
        .unwrap();
    bus.api_environment_update(
        workspace_id.clone(),
        environment.id.clone(),
        "Legacy Updated".to_string(),
        Vec::new(),
    )
    .await
    .unwrap();
    bus.api_environment_delete(workspace_id, environment.id)
        .await
        .unwrap();
    let commands: Vec<(String,)> =
        sqlx::query_as("SELECT DISTINCT command_name FROM hook_effects ORDER BY command_name")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        commands,
        vec![
            ("workspace.environment.create".to_string(),),
            ("workspace.environment.delete".to_string(),),
            ("workspace.environment.update".to_string(),),
        ]
    );
}
