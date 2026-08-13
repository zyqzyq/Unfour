use super::*;

#[tokio::test]
async fn variable_replace_reports_only_real_diff_and_secret_snapshot_is_redacted() {
    let capture = Arc::new(SqlHook {
        name: "capture",
        fail_on: None,
        local_only: false,
    });
    let (bus, db) = bus_with_hooks(vec![capture]).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let initial = bus
        .workspace_variables_replace(
            workspace_id.clone(),
            vec![
                input(None, "PLAIN", "one", false),
                input(None, "TOKEN", "top-secret", true),
            ],
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM hook_effects")
        .execute(db.pool())
        .await
        .unwrap();
    let updated = bus
        .workspace_variables_replace(
            workspace_id.clone(),
            vec![
                input(Some(initial[0].id.clone()), "PLAIN", "two", false),
                input(Some(initial[1].id.clone()), "TOKEN", "top-secret", true),
            ],
        )
        .await
        .unwrap();
    let mutations: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM hook_effects WHERE command_name = 'workspace.variables.replace'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(mutations, 1);

    let snapshot = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::WorkspaceVariable,
            &workspace_id,
            &updated[1].id,
        ))
        .await
        .unwrap();
    let DomainSnapshot::WorkspaceVariable(snapshot) = snapshot else {
        panic!("expected variable snapshot");
    };
    assert_eq!(snapshot.value, SnapshotVariableValue::SecretRedacted);
    let serialized = serde_json::to_string(&snapshot).unwrap();
    let debug = format!("{snapshot:?}");
    assert!(!serialized.contains("top-secret"));
    assert!(!debug.contains("top-secret"));

    bus.workspace_variable_delete(workspace_id.clone(), updated[1].id.clone())
        .await
        .unwrap();
    assert!(matches!(
        bus.read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::WorkspaceVariable,
            workspace_id,
            updated[1].id.clone(),
        ))
        .await
        .unwrap(),
        DomainSnapshot::Tombstone(_)
    ));
}
