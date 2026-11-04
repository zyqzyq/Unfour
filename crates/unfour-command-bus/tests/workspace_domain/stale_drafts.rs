use super::*;

#[tokio::test]
async fn stale_workspace_rename_and_security_edits_are_rejected() {
    let (bus, _) = bus_with_hooks(vec![]).await;
    let state = bus.list_workspaces().await.unwrap();
    let baseline = state
        .workspaces
        .into_iter()
        .find(|w| w.id == state.active_workspace_id)
        .unwrap();
    let current = bus
        .rename_workspace(baseline.id.clone(), "External".into())
        .await
        .unwrap();
    assert!(matches!(
        bus.rename_workspace_checked(baseline.id.clone(), "Stale".into(), Some(baseline.name))
            .await
            .unwrap_err(),
        AppError::RowConflict(_)
    ));
    bus.rename_workspace_checked(
        current.id.clone(),
        "Fresh".into(),
        Some(current.name.clone()),
    )
    .await
    .unwrap();
    let updated = bus
        .update_workspace_environment(current.id.clone(), "prod".into())
        .await
        .unwrap();
    assert!(matches!(
        bus.update_workspace_mcp_policy_checked(
            current.id.clone(),
            "full_access".into(),
            Some(current.clone())
        )
        .await
        .unwrap_err(),
        AppError::RowConflict(_)
    ));
    assert!(matches!(
        bus.update_workspace_environment_checked(current.id.clone(), "dev".into(), Some(current))
            .await
            .unwrap_err(),
        AppError::RowConflict(_)
    ));
    let saved = bus
        .update_workspace_mcp_policy_checked(updated.id.clone(), "read_only".into(), Some(updated))
        .await
        .unwrap();
    assert_eq!(saved.environment_type, "prod");
    assert_eq!(saved.mcp_policy, "read_only");
}

#[tokio::test]
async fn stale_environment_child_snapshot_cannot_overwrite_or_rename() {
    let (bus, db) = bus_with_hooks(vec![]).await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let env = bus
        .workspace_environment_create(ws.clone(), "Dev".into())
        .await
        .unwrap();
    let baseline = bus
        .workspace_environment_update(
            ws.clone(),
            env.id.clone(),
            env.name,
            vec![input(None, "HOST", "old", false)],
        )
        .await
        .unwrap();
    // Simulate another process changing only a child, without touching parent timestamps.
    sqlx::query("UPDATE workspace_environment_variables SET value = 'external', revision = revision + 1 WHERE id = ?")
        .bind(&baseline.variables[0].id).execute(db.pool()).await.unwrap();
    let error = bus
        .workspace_environment_update_checked(
            ws.clone(),
            baseline.id.clone(),
            "stale rename".into(),
            vec![],
            Some(baseline.clone()),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::RowConflict(_)));
    let current = bus
        .workspace_environments_list(ws.clone())
        .await
        .unwrap()
        .remove(0);
    assert_eq!(current.name, "Dev");
    assert_eq!(current.variables[0].value, "external");
    let saved = bus
        .workspace_environment_update_checked(
            ws.clone(),
            current.id.clone(),
            "Fresh".into(),
            vec![],
            Some(current),
        )
        .await
        .unwrap();
    assert_eq!(saved.name, "Fresh");
    bus.workspace_environment_delete(ws.clone(), saved.id.clone())
        .await
        .unwrap();
    assert!(bus
        .workspace_environment_update_checked(
            ws,
            saved.id.clone(),
            "resurrect".into(),
            vec![],
            Some(saved)
        )
        .await
        .is_err());
}

#[tokio::test]
async fn stale_workspace_variable_snapshot_cannot_delete_external_additions() {
    let (bus, _) = bus_with_hooks(vec![]).await;
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let baseline = bus.workspace_variables_list(ws.clone()).await.unwrap();
    bus.workspace_variable_create(ws.clone(), input(None, "HOST", "external", false))
        .await
        .unwrap();
    assert!(matches!(
        bus.workspace_variables_replace_checked(ws.clone(), vec![], Some(baseline))
            .await
            .unwrap_err(),
        AppError::RowConflict(_)
    ));
    let current = bus.workspace_variables_list(ws.clone()).await.unwrap();
    assert_eq!(current.len(), 1);
    assert!(bus
        .workspace_variables_replace_checked(ws, vec![], Some(current))
        .await
        .unwrap()
        .is_empty());
}
