use super::*;

#[tokio::test]
async fn external_last_workspace_delete_and_fallback_roll_back_on_hook_failure() {
    let rejecting = Arc::new(SqlHook {
        name: "rejecting",
        fail_on: Some("workspace.external.apply_page"),
        local_only: false,
    });
    let (bus, db) = bus_with_hooks(vec![rejecting]).await;
    let original = bus.list_workspaces().await.unwrap().active_workspace_id;
    sqlx::query("DELETE FROM hook_effects")
        .execute(db.pool())
        .await
        .unwrap();

    bus.apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
        entity: DomainEntityKey::new(DomainEntityType::Workspace, &original, &original),
        deleted_at: "2026-07-24T00:01:31Z".to_string(),
    })])
    .await
    .expect_err("hook failure must roll back delete and fallback creation");

    let original_deleted_at: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspaces WHERE id = ?1")
            .bind(&original)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(original_deleted_at.is_none());
    let active_workspaces: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workspaces WHERE deleted_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(active_workspaces, 1);
    assert_eq!(
        bus.list_workspaces().await.unwrap().active_workspace_id,
        original
    );
    let hook_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(hook_rows, 0);
}

#[tokio::test]
async fn external_apply_updates_revision_preserves_local_secret_and_creates_no_echo_rows() {
    let outbox = Arc::new(SqlHook {
        name: "local-outbox",
        fail_on: None,
        local_only: true,
    });
    let (bus, db) = bus_with_hooks(vec![outbox]).await;
    let state = bus.list_workspaces().await.unwrap();
    let workspace_id = state.active_workspace_id.clone();
    let secret = bus
        .workspace_variable_create(
            workspace_id.clone(),
            input(None, "TOKEN", "device-secret", true),
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM hook_effects")
        .execute(db.pool())
        .await
        .unwrap();
    let before_revision = secret.revision;
    let now = "2026-07-23T09:15:37Z".to_string();
    let applied = bus
        .apply_external_page(ExternalApplyPage {
            workspace_variables: vec![ExternalWorkspaceVariableApply::Upsert(
                ExternalWorkspaceVariableUpsert {
                    id: secret.id.clone(),
                    workspace_id: workspace_id.clone(),
                    key: "TOKEN".to_string(),
                    value: ExternalVariableValue::PreserveLocal,
                    is_secret: true,
                    is_enabled: false,
                    description: Some("external metadata".to_string()),
                    sort_order: 0,
                    created_at: secret.created_at.clone(),
                    updated_at: now,
                },
            )],
            ..ExternalApplyPage::default()
        })
        .await
        .unwrap();
    assert_eq!(applied.applied_count, 1);
    assert_eq!(applied.secret_material_outcomes.len(), 1);
    assert_eq!(
        applied.secret_material_outcomes[0].status,
        unfour_core::domain::SecretMaterialStatus::Present
    );
    let echo_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(echo_rows, 0);
    let stored: (String, i64, bool) =
        sqlx::query_as("SELECT value, revision, is_enabled FROM workspace_variables WHERE id = ?1")
            .bind(&secret.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stored.0, "device-secret");
    assert_eq!(stored.1, before_revision + 1);
    assert!(!stored.2);

    let active_before = bus.list_workspaces().await.unwrap().active_workspace_id;
    bus.set_active_workspace(active_before.clone())
        .await
        .unwrap();
    let local_before: (bool, Option<String>) =
        sqlx::query_as("SELECT is_default, last_opened_at FROM workspaces WHERE id = ?1")
            .bind(&active_before)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let existing = bus
        .list_workspaces()
        .await
        .unwrap()
        .workspaces
        .into_iter()
        .find(|workspace| workspace.id == active_before)
        .unwrap();
    bus.apply_external_workspaces(vec![ExternalWorkspaceApply::Upsert(
        ExternalWorkspaceUpsert {
            id: active_before.clone(),
            name: "Remote Rename".to_string(),
            environment_type: existing.environment_type,
            mcp_policy: existing.mcp_policy,
            created_at: existing.created_at,
            updated_at: "2026-07-23T09:16:38Z".to_string(),
        },
    )])
    .await
    .unwrap();
    let local_after: (bool, Option<String>) =
        sqlx::query_as("SELECT is_default, last_opened_at FROM workspaces WHERE id = ?1")
            .bind(&active_before)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(local_after, local_before);
    bus.apply_external_workspaces(vec![ExternalWorkspaceApply::Upsert(
        ExternalWorkspaceUpsert {
            id: "external-workspace".to_string(),
            name: "External".to_string(),
            environment_type: "test".to_string(),
            mcp_policy: "guarded".to_string(),
            created_at: "2026-07-23T09:15:38Z".to_string(),
            updated_at: "2026-07-23T09:15:38Z".to_string(),
        },
    )])
    .await
    .unwrap();
    assert_eq!(
        bus.list_workspaces().await.unwrap().active_workspace_id,
        active_before
    );
}

#[tokio::test]
async fn external_workspace_changes_preserve_local_workspace_invariants() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let original = bus.list_workspaces().await.unwrap().active_workspace_id;
    let active = bus
        .create_workspace("External Delete Target".to_string())
        .await
        .unwrap();
    let deleted_at = "2026-07-24T00:00:00Z".to_string();

    bus.apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
        entity: DomainEntityKey::new(DomainEntityType::Workspace, &active.id, &active.id),
        deleted_at: deleted_at.clone(),
    })])
    .await
    .expect("delete active workspace externally");

    let stored_active: String =
        sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'active_workspace_id'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stored_active, original);
    let report = bus
        .apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
            entity: DomainEntityKey::new(DomainEntityType::Workspace, &original, &original),
            deleted_at,
        })])
        .await
        .expect("external apply should replace the last workspace with a local fallback");
    assert_eq!(report.applied_count, 2);
    assert_eq!(report.mutations.len(), 2);
    let original_tombstone: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspaces WHERE id = ?1")
            .bind(&original)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(original_tombstone.is_some());
    let fallback: (String, String, bool, String, String) = sqlx::query_as(
        r#"
        SELECT id, name, is_default, environment_type, mcp_policy
        FROM workspaces WHERE deleted_at IS NULL
        "#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_ne!(fallback.0, original);
    assert_eq!(fallback.1, "Default Workspace");
    assert!(fallback.2);
    assert_eq!(fallback.3, "dev");
    assert_eq!(fallback.4, "auto");
    let fallback_active: String =
        sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'active_workspace_id'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(fallback_active, fallback.0);
    let companions: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
          (SELECT COUNT(*) FROM workspace_settings WHERE workspace_id = ?1),
          (SELECT COUNT(*) FROM workspace_local_state WHERE workspace_id = ?1)
        "#,
    )
    .bind(&fallback.0)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(companions, (1, 1));

    let now = "2026-07-24T00:01:00Z".to_string();
    bus.apply_external_workspaces(vec![ExternalWorkspaceApply::Upsert(
        ExternalWorkspaceUpsert {
            id: "external-default".to_string(),
            name: "External Default".to_string(),
            environment_type: "dev".to_string(),
            mcp_policy: "auto".to_string(),
            created_at: now.clone(),
            updated_at: now,
        },
    )])
    .await
    .expect("apply new workspace without changing the local default");
    let defaults: Vec<(String,)> =
        sqlx::query_as("SELECT id FROM workspaces WHERE is_default = 1 AND deleted_at IS NULL")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(defaults, vec![(fallback.0,)]);
}

#[tokio::test]
async fn external_last_workspace_fallback_has_no_local_echo() {
    let outbox = Arc::new(SqlHook {
        name: "local-outbox",
        fail_on: None,
        local_only: true,
    });
    let (bus, db) = bus_with_hooks(vec![outbox]).await;
    let original = bus.list_workspaces().await.unwrap().active_workspace_id;
    bus.workspace_variable_create(
        original.clone(),
        input(None, "ORIGINAL_ONLY", "value", false),
    )
    .await
    .unwrap();
    bus.workspace_environment_create(original.clone(), "Original Environment".to_string())
        .await
        .unwrap();
    sqlx::query("DELETE FROM hook_effects")
        .execute(db.pool())
        .await
        .unwrap();

    let report = bus
        .apply_external_workspaces(vec![ExternalWorkspaceApply::Delete(ExternalDelete {
            entity: DomainEntityKey::new(DomainEntityType::Workspace, &original, &original),
            deleted_at: "2026-07-24T00:01:30Z".to_string(),
        })])
        .await
        .unwrap();

    assert_eq!(report.mutations.len(), 2);
    assert!(report
        .mutations
        .iter()
        .all(|mutation| mutation.origin == MutationOrigin::External));
    let fallback_id = report
        .mutations
        .iter()
        .find(|mutation| mutation.operation == unfour_core::domain::MutationOperation::Upsert)
        .unwrap()
        .entity
        .entity_id
        .clone();
    let inherited: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
          (SELECT COUNT(*) FROM workspace_variables WHERE workspace_id = ?1),
          (SELECT COUNT(*) FROM workspace_environments WHERE workspace_id = ?1)
        "#,
    )
    .bind(&fallback_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(inherited, (0, 0));
    let echo_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hook_effects")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(echo_rows, 0);
}

#[tokio::test]
async fn external_new_secret_preserves_metadata_and_reports_missing_material() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let now = "2026-07-24T00:02:00Z".to_string();
    let report = bus
        .apply_external_workspace_variables(vec![ExternalWorkspaceVariableApply::Upsert(
            ExternalWorkspaceVariableUpsert {
                id: "missing-local-secret".to_string(),
                workspace_id: workspace_id.clone(),
                key: "TOKEN".to_string(),
                value: ExternalVariableValue::PreserveLocal,
                is_secret: true,
                is_enabled: true,
                description: None,
                sort_order: 0,
                created_at: now.clone(),
                updated_at: now,
            },
        )])
        .await
        .expect("missing local secret metadata should be created");
    assert_eq!(report.applied_count, 1);
    assert_eq!(report.secret_material_outcomes.len(), 1);
    let outcome = &report.secret_material_outcomes[0];
    assert_eq!(outcome.entity.entity_id, "missing-local-secret");
    assert_eq!(
        outcome.status,
        unfour_core::domain::SecretMaterialStatus::Missing
    );
    let stored: (String, bool, String) = sqlx::query_as(
        "SELECT value, is_secret, key FROM workspace_variables WHERE id = 'missing-local-secret'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(stored.0, "");
    assert!(stored.1);
    assert_eq!(stored.2, "TOKEN");

    let secret = "must-not-leak";
    let error = bus
        .apply_external_workspace_variables(vec![ExternalWorkspaceVariableApply::Upsert(
            ExternalWorkspaceVariableUpsert {
                id: "rejected-secret-set".to_string(),
                workspace_id,
                key: "TOKEN_2".to_string(),
                value: ExternalVariableValue::Set(secret.to_string()),
                is_secret: true,
                is_enabled: true,
                description: None,
                sort_order: 1,
                created_at: "2026-07-24T00:02:01Z".to_string(),
                updated_at: "2026-07-24T00:02:01Z".to_string(),
            },
        )])
        .await
        .expect_err("external secret material must be rejected");
    assert!(!error.to_string().contains(secret));
    let rejected_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_variables WHERE id = 'rejected-secret-set'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rejected_rows, 0);
}

#[tokio::test]
async fn external_delete_of_active_environment_selects_first_remaining_environment() {
    let db = database().await;
    let bus = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let active = bus
        .workspace_environment_create(workspace_id.clone(), "Active".to_string())
        .await
        .unwrap();
    let second = bus
        .workspace_environment_create(workspace_id.clone(), "Second".to_string())
        .await
        .unwrap();
    let first_fallback = bus
        .workspace_environment_create(workspace_id.clone(), "First Fallback".to_string())
        .await
        .unwrap();
    bus.workspace_environments_reorder(
        workspace_id.clone(),
        vec![
            first_fallback.id.clone(),
            second.id.clone(),
            active.id.clone(),
        ],
    )
    .await
    .unwrap();

    bus.apply_external_workspace_environments(vec![ExternalWorkspaceEnvironmentApply::Delete(
        ExternalDelete {
            entity: DomainEntityKey::new(
                DomainEntityType::WorkspaceEnvironment,
                &workspace_id,
                &active.id,
            ),
            deleted_at: "2026-07-24T00:02:30Z".to_string(),
        },
    )])
    .await
    .unwrap();

    let selected: Option<String> = sqlx::query_scalar(
        "SELECT active_environment_id FROM workspace_local_state WHERE workspace_id = ?1",
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(selected.as_deref(), Some(first_fallback.id.as_str()));
}

#[tokio::test]
async fn external_environment_delete_rolls_back_tombstones_and_fallback_together() {
    let rejecting = Arc::new(SqlHook {
        name: "rejecting",
        fail_on: Some("workspace.external.apply_page"),
        local_only: false,
    });
    let (bus, db) = bus_with_hooks(vec![rejecting]).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let active = bus
        .workspace_environment_create(workspace_id.clone(), "Active".to_string())
        .await
        .unwrap();
    let child = bus
        .workspace_environment_variable_create(
            workspace_id.clone(),
            active.id.clone(),
            input(None, "TOKEN", "local-secret", true),
        )
        .await
        .unwrap();
    let fallback = bus
        .workspace_environment_create(workspace_id.clone(), "Fallback".to_string())
        .await
        .unwrap();
    sqlx::query("DELETE FROM hook_effects")
        .execute(db.pool())
        .await
        .unwrap();

    bus.apply_external_workspace_environments(vec![ExternalWorkspaceEnvironmentApply::Delete(
        ExternalDelete {
            entity: DomainEntityKey::new(
                DomainEntityType::WorkspaceEnvironment,
                &workspace_id,
                &active.id,
            ),
            deleted_at: "2026-07-24T00:02:31Z".to_string(),
        },
    )])
    .await
    .expect_err("hook failure must roll back environment delete transaction");

    let environment_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspace_environments WHERE id = ?1")
            .bind(&active.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let child_deleted: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspace_environment_variables WHERE id = ?1")
            .bind(&child.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let selected: Option<String> = sqlx::query_scalar(
        "SELECT active_environment_id FROM workspace_local_state WHERE workspace_id = ?1",
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(environment_deleted.is_none());
    assert!(child_deleted.is_none());
    assert_eq!(selected.as_deref(), Some(active.id.as_str()));
    assert_ne!(selected.as_deref(), Some(fallback.id.as_str()));
}

#[tokio::test]
async fn external_environment_cascade_reports_returned_child_revision() {
    let capture = Arc::new(SqlHook {
        name: "capture",
        fail_on: None,
        local_only: false,
    });
    let (bus, db) = bus_with_hooks(vec![capture]).await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Cascade".to_string())
        .await
        .unwrap();
    let child = bus
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            input(None, "VALUE", "one", false),
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM hook_effects")
        .execute(db.pool())
        .await
        .unwrap();

    let report = bus
        .apply_external_workspace_environments(vec![ExternalWorkspaceEnvironmentApply::Delete(
            ExternalDelete {
                entity: DomainEntityKey::new(
                    DomainEntityType::WorkspaceEnvironment,
                    &workspace_id,
                    &environment.id,
                ),
                deleted_at: "2026-07-24T00:03:00Z".to_string(),
            },
        )])
        .await
        .unwrap();

    let stored_revision: i64 =
        sqlx::query_scalar("SELECT revision FROM workspace_environment_variables WHERE id = ?1")
            .bind(&child.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let hook_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM hook_effects WHERE command_name = 'workspace.external.apply_page' AND entity_id = ?1",
    )
    .bind(&child.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(hook_revision, stored_revision);
    let child_mutation = report
        .mutations
        .iter()
        .find(|mutation| mutation.entity.entity_id == child.id)
        .unwrap();
    assert_eq!(
        child_mutation.entity.parent_entity_id.as_deref(),
        Some(environment.id.as_str())
    );
    let selected: Option<String> = sqlx::query_scalar(
        "SELECT active_environment_id FROM workspace_local_state WHERE workspace_id = ?1",
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(selected.is_none());
    let snapshot = bus
        .read_domain_snapshot(&DomainEntityKey::new(
            DomainEntityType::WorkspaceEnvironmentVariable,
            &workspace_id,
            &child.id,
        ))
        .await
        .unwrap();
    let DomainSnapshot::Tombstone(tombstone) = snapshot else {
        panic!("expected child tombstone");
    };
    assert_eq!(
        tombstone.entity.parent_entity_id.as_deref(),
        Some(environment.id.as_str())
    );
}
