use super::*;
use crate::CommandBusExtensions;
use std::sync::Arc;

fn options(policy: &str) -> WorkspaceBundleOptions {
    WorkspaceBundleOptions {
        mcp_policy: Some(policy.into()),
        ..Default::default()
    }
}

async fn persisted(bus: &CommandBus, id: &str) -> Workspace {
    bus.list_workspaces()
        .await
        .unwrap()
        .workspaces
        .into_iter()
        .find(|workspace| workspace.id == id)
        .unwrap()
}

#[tokio::test]
async fn imports_default_to_disabled_and_new_workspaces_still_default_to_auto() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let created = bus.create_workspace("New".into()).await.unwrap();
    assert_eq!(created.mcp_policy, "auto");
    for version in [1, 2] {
        let mut bundle = tests::fixture();
        bundle["version"] = json!(version);
        let workspace = bus
            .workspace_bundle_import_with_options(
                bundle.to_string(),
                "Default import".into(),
                WorkspaceBundleOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(workspace.mcp_policy, "disabled");
        assert_eq!(persisted(&bus, &workspace.id).await.mcp_policy, "disabled");
    }
}

#[tokio::test]
async fn imports_persist_only_the_explicit_local_policy_for_v1_and_v2() {
    let bus = CommandBus::ephemeral().await.unwrap();
    for version in [1, 2] {
        for policy in ["auto", "disabled", "read_only", "guarded", "full_access"] {
            let mut bundle = tests::fixture();
            bundle["version"] = json!(version);
            bundle["workspace"]["environmentType"] = json!("prod");
            let workspace = bus
                .workspace_bundle_import_with_options(
                    bundle.to_string(),
                    format!("Import {version} {policy}"),
                    options(policy),
                )
                .await
                .unwrap();
            assert_eq!(workspace.mcp_policy, policy);
            let saved = persisted(&bus, &workspace.id).await;
            assert_eq!(saved.mcp_policy, policy);
            assert_eq!(saved.environment_type, "prod");
        }
    }
}

#[tokio::test]
async fn import_rejects_embedded_permissions_and_invalid_local_choices_without_writes() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let baseline = bus.list_workspaces().await.unwrap().workspaces.len();
    for policy in ["auto", "disabled", "read_only", "guarded", "full_access"] {
        let mut bundle = tests::fixture();
        bundle["workspace"]["mcpPolicy"] = json!(policy);
        // Preserve the strict portable schema: policy fields in a file are never accepted.
        assert!(bus
            .workspace_bundle_preview_with_options(&bundle.to_string(), options("guarded"))
            .await
            .is_err());
        assert!(bus
            .workspace_bundle_import_with_options(
                bundle.to_string(),
                "Untrusted".into(),
                options("guarded"),
            )
            .await
            .is_err());
    }
    assert!(bus
        .workspace_bundle_preview_with_options(&tests::fixture().to_string(), options("invalid"))
        .await
        .is_err());
    assert!(bus
        .workspace_bundle_import_with_options(
            tests::fixture().to_string(),
            "Invalid choice".into(),
            options("invalid"),
        )
        .await
        .is_err());
    assert_eq!(
        bus.list_workspaces().await.unwrap().workspaces.len(),
        baseline
    );
}

#[tokio::test]
async fn imported_permissions_and_environment_can_be_changed_independently() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus
        .workspace_bundle_import_with_options(
            tests::fixture().to_string(),
            "Editable import".into(),
            WorkspaceBundleOptions::default(),
        )
        .await
        .unwrap();
    let updated = bus
        .update_workspace_mcp_policy(workspace.id.clone(), "full_access".into())
        .await
        .unwrap();
    assert_eq!(updated.environment_type, "dev");
    let updated = bus
        .update_workspace_environment(workspace.id.clone(), "prod".into())
        .await
        .unwrap();
    assert_eq!(updated.mcp_policy, "full_access");
    bus.update_workspace_mcp_policy(workspace.id.clone(), "auto".into())
        .await
        .unwrap();
    let saved = persisted(&bus, &workspace.id).await;
    assert_eq!(saved.environment_type, "prod");
    assert_eq!(saved.mcp_policy, "auto");
}

#[tokio::test]
async fn permission_save_failure_preserves_the_imported_workspace_and_revision() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let workspace = bus
        .workspace_bundle_import(tests::fixture().to_string(), "Editable".into())
        .await
        .unwrap();
    bus.extensions = CommandBusExtensions::new(vec![Arc::new(tests::FailCommit)]);
    assert!(bus
        .update_workspace_mcp_policy(workspace.id.clone(), "full_access".into())
        .await
        .is_err());
    let saved = persisted(&bus, &workspace.id).await;
    assert_eq!(saved.mcp_policy, "disabled");
    assert_eq!(saved.environment_type, workspace.environment_type);
    assert_eq!(saved.revision, workspace.revision);
}

#[tokio::test]
async fn explicit_import_policy_rolls_back_with_all_business_records_on_commit_failure() {
    let mut bus = CommandBus::ephemeral().await.unwrap();
    let tables = [
        "workspaces",
        "api_requests",
        "connections",
        "ssh_task",
        "saved_sql",
        "flow_definitions",
    ];
    let mut baseline = Vec::new();
    for table in tables {
        baseline.push(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(bus.db.pool())
                .await
                .unwrap(),
        );
    }
    bus.extensions = CommandBusExtensions::new(vec![Arc::new(tests::FailCommit)]);
    assert!(bus
        .workspace_bundle_import_with_options(
            tests::fixture().to_string(),
            "Failed import".into(),
            options("full_access"),
        )
        .await
        .is_err());
    for (table, count) in tables.into_iter().zip(baseline) {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(bus.db.pool())
                .await
                .unwrap(),
            count
        );
    }
}
