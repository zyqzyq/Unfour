use super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_core::domain::{
    CommandContext, DomainEntityKey, DomainEntityType, ExternalApplyPage, ExternalDelete,
    ExternalVariableValue, ExternalWorkspaceApply, ExternalWorkspaceEnvironmentVariableApply,
    ExternalWorkspaceEnvironmentVariableUpsert,
};
use unfour_core::models::{WorkspaceEnvironment, WorkspaceVariable, WorkspaceVariableInput};

struct TestWorkspaceService {
    inner: WorkspaceService,
    db: LocalDb,
}

impl TestWorkspaceService {
    async fn state(&self) -> AppResult<WorkspaceState> {
        self.inner.state().await
    }

    async fn state_read_only(&self) -> AppResult<WorkspaceState> {
        self.inner.state_read_only().await
    }

    async fn list_variables(&self, workspace_id: String) -> AppResult<Vec<WorkspaceVariable>> {
        self.inner.list_variables(workspace_id).await
    }

    async fn list_environments(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<WorkspaceEnvironment>> {
        self.inner.list_environments(workspace_id).await
    }

    async fn resolve_variables(
        &self,
        workspace_id: &str,
        environment_id: Option<&str>,
        input: &str,
    ) -> AppResult<String> {
        self.inner
            .resolve_variables(workspace_id, environment_id, input)
            .await
    }

    async fn active_environment_id(&self, workspace_id: &str) -> AppResult<Option<String>> {
        self.inner.active_environment_id(workspace_id).await
    }

    async fn layout(&self, workspace_id: String) -> AppResult<WorkspaceLayout> {
        self.inner.layout(workspace_id).await
    }

    async fn update_layout(
        &self,
        workspace_id: String,
        layout: WorkspaceLayout,
    ) -> AppResult<WorkspaceLayout> {
        self.inner.update_layout(workspace_id, layout).await
    }

    async fn create(&self, name: String) -> AppResult<Workspace> {
        self.create_with_options(name, None, None).await
    }

    async fn create_with_options(
        &self,
        name: String,
        environment_type: Option<String>,
        mcp_policy: Option<String>,
    ) -> AppResult<Workspace> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .create_with_options_on(
                &mut tx,
                &CommandContext::local("test.workspace.create"),
                name,
                environment_type,
                mcp_policy,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn rename(&self, workspace_id: String, name: String) -> AppResult<Workspace> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .rename_on(
                &mut tx,
                &CommandContext::local("test.workspace.rename"),
                workspace_id,
                name,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn set_active(&self, workspace_id: String) -> AppResult<WorkspaceState> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .set_active_on(
                &mut tx,
                &CommandContext::local("test.workspace.activate"),
                workspace_id,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn delete(&self, workspace_id: String) -> AppResult<WorkspaceState> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .delete_on(
                &mut tx,
                &CommandContext::local("test.workspace.delete"),
                workspace_id,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn replace_variables(
        &self,
        workspace_id: String,
        variables: Vec<WorkspaceVariableInput>,
    ) -> AppResult<Vec<WorkspaceVariable>> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .replace_variables_on(
                &mut tx,
                &CommandContext::local("test.workspace.variables.replace"),
                workspace_id,
                variables,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn create_environment(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<WorkspaceEnvironment> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .create_environment_on(
                &mut tx,
                &CommandContext::local("test.workspace.environment.create"),
                workspace_id,
                name,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn update_environment(
        &self,
        workspace_id: String,
        environment_id: String,
        name: String,
        variables: Vec<WorkspaceVariableInput>,
    ) -> AppResult<WorkspaceEnvironment> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .update_environment_on(
                &mut tx,
                &CommandContext::local("test.workspace.environment.update"),
                workspace_id,
                environment_id,
                name,
                variables,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn delete_environment(
        &self,
        workspace_id: String,
        environment_id: String,
    ) -> AppResult<Vec<WorkspaceEnvironment>> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .delete_environment_on(
                &mut tx,
                &CommandContext::local("test.workspace.environment.delete"),
                workspace_id,
                environment_id,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn set_active_environment(
        &self,
        workspace_id: String,
        environment_id: Option<String>,
    ) -> AppResult<Vec<WorkspaceEnvironment>> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .inner
            .set_active_environment_on(
                &mut tx,
                &CommandContext::local("test.workspace.environment.activate"),
                workspace_id,
                environment_id,
            )
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    async fn read_setting(&self, key: &str) -> AppResult<Option<String>> {
        let mut connection = self.db.pool().acquire().await?;
        read_setting_on(&mut connection, key).await
    }

    async fn apply_external_page(&self, page: ExternalApplyPage) -> AppResult<()> {
        let mut tx = self.db.pool().begin().await?;
        self.inner
            .apply_external_page_on(
                &mut tx,
                &CommandContext::external("test.workspace.external.apply_page"),
                page,
            )
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn variable(key: &str, value: &str) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id: None,
        key: key.to_string(),
        value: value.to_string(),
        is_secret: false,
        is_enabled: true,
        description: None,
        sort_order: 0,
    }
}

async fn service() -> TestWorkspaceService {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory sqlite");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("run migrations");
    let service = WorkspaceService::new(db.clone());
    let mut tx = db.pool().begin().await.expect("begin seed transaction");
    service
        .ensure_default_workspace_on(
            &mut tx,
            &CommandContext::migration("test.workspace.ensure_default"),
        )
        .await
        .expect("ensure default workspace");
    tx.commit().await.expect("commit seed transaction");
    TestWorkspaceService { inner: service, db }
}

#[tokio::test]
async fn layout_returns_defaults_for_new_workspace() {
    let service = service().await;
    let state = service.state().await.expect("workspace state");

    let layout = service
        .layout(state.active_workspace_id)
        .await
        .expect("workspace layout");

    assert_eq!(layout.active_tab_id, "api-main");
    assert!(!layout.sidebar_collapsed);
    assert_eq!(layout.tabs.len(), 3);
    assert!(layout
        .tabs
        .iter()
        .any(|tab| tab.id == "database-main" && tab.kind == "database"));
}

#[tokio::test]
async fn layout_update_persists_valid_layout() {
    let service = service().await;
    let state = service.state().await.expect("workspace state");
    let workspace_id = state.active_workspace_id;
    let mut layout = service
        .layout(workspace_id.clone())
        .await
        .expect("workspace layout");
    layout.sidebar_collapsed = true;
    layout.active_tab_id = "database-main".to_string();
    layout.selected_database_connection_id = Some("db-1".to_string());

    let updated = service
        .update_layout(workspace_id.clone(), layout)
        .await
        .expect("update layout");
    let loaded = service.layout(workspace_id).await.expect("reload layout");

    assert!(updated.sidebar_collapsed);
    assert_eq!(loaded.active_tab_id, "database-main");
    assert_eq!(
        loaded.selected_database_connection_id.as_deref(),
        Some("db-1")
    );
}

#[tokio::test]
async fn layout_update_rejects_active_tab_outside_open_tabs() {
    let service = service().await;
    let state = service.state().await.expect("workspace state");
    let workspace_id = state.active_workspace_id;
    let mut layout = service
        .layout(workspace_id.clone())
        .await
        .expect("workspace layout");
    layout.active_tab_id = "missing-tab".to_string();

    let result = service.update_layout(workspace_id, layout).await;

    assert!(matches!(result, Err(AppError::Validation(_))));
}

#[tokio::test]
async fn create_switch_rename_and_delete_preserve_active_workspace() {
    let service = service().await;
    let default_id = service
        .state()
        .await
        .expect("workspace state")
        .active_workspace_id;

    let created = service
        .create("  Client Ops  ".to_string())
        .await
        .expect("create workspace");
    let created_state = service.state().await.expect("state after create");
    assert_eq!(created.name, "Client Ops");
    assert_eq!(created.environment_type, "dev");
    assert_eq!(created.mcp_policy, "auto");
    assert_eq!(created_state.active_workspace_id, created.id);

    let renamed = service
        .rename(created.id.clone(), "Client Ops EU".to_string())
        .await
        .expect("rename workspace");
    assert_eq!(renamed.name, "Client Ops EU");
    assert!(renamed.revision > created.revision);

    let switched = service
        .set_active(default_id.clone())
        .await
        .expect("switch active workspace");
    assert_eq!(switched.active_workspace_id, default_id);

    let deleted = service
        .delete(created.id.clone())
        .await
        .expect("delete inactive workspace");
    assert_eq!(deleted.active_workspace_id, default_id);
    assert!(!deleted
        .workspaces
        .iter()
        .any(|workspace| workspace.id == created.id));
}

#[tokio::test]
async fn create_with_options_stores_environment_and_policy() {
    let service = service().await;

    let created = service
        .create_with_options(
            "Production".to_string(),
            Some("prod".to_string()),
            Some("read_only".to_string()),
        )
        .await
        .expect("create prod workspace");

    assert_eq!(created.environment_type, "prod");
    assert_eq!(created.mcp_policy, "read_only");
}

#[tokio::test]
async fn delete_active_workspace_falls_back_to_default() {
    let service = service().await;
    let default_id = service
        .state()
        .await
        .expect("workspace state")
        .active_workspace_id;
    let created = service
        .create("Scratch".to_string())
        .await
        .expect("create workspace");

    let state = service
        .delete(created.id)
        .await
        .expect("delete active workspace");

    assert_eq!(state.active_workspace_id, default_id);
    assert_eq!(state.workspaces.len(), 1);
}

#[tokio::test]
async fn state_read_only_does_not_write_fallback_active_workspace() {
    let service = service().await;
    sqlx::query("DELETE FROM app_settings WHERE key = 'active_workspace_id'")
        .execute(service.db.pool())
        .await
        .expect("remove active workspace setting");

    let state = service
        .state_read_only()
        .await
        .expect("read-only state should work");

    assert!(!state.active_workspace_id.is_empty());
    assert_eq!(
        service
            .read_setting("active_workspace_id")
            .await
            .expect("read active workspace setting"),
        None
    );
}

#[tokio::test]
async fn state_repairs_fallback_active_workspace_setting() {
    let service = service().await;
    sqlx::query(
        "UPDATE app_settings SET value = 'deleted-workspace' WHERE key = 'active_workspace_id'",
    )
    .execute(service.db.pool())
    .await
    .expect("dirty active workspace setting");

    let state = service.state().await.expect("state should repair setting");

    assert_eq!(
        service
            .read_setting("active_workspace_id")
            .await
            .expect("read repaired setting"),
        Some(state.active_workspace_id)
    );
}

#[tokio::test]
async fn workspace_variables_support_create_update_and_delete() {
    let service = service().await;
    let workspace_id = service.state().await.unwrap().active_workspace_id;

    let created = service
        .replace_variables(
            workspace_id.clone(),
            vec![variable("BASE_URL", "https://workspace.example")],
        )
        .await
        .expect("create workspace variable");
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].key, "BASE_URL");

    let mut updated = variable("BASE_URL", "https://updated.example");
    updated.id = Some(created[0].id.clone());
    updated.is_secret = true;
    updated.description = Some("Shared endpoint".to_string());
    let saved = service
        .replace_variables(workspace_id.clone(), vec![updated])
        .await
        .expect("update workspace variable");
    assert_eq!(saved[0].value, "https://updated.example");
    assert!(saved[0].is_secret);

    service
        .replace_variables(workspace_id.clone(), Vec::new())
        .await
        .expect("delete workspace variable");
    assert!(service
        .list_variables(workspace_id)
        .await
        .expect("list variables")
        .is_empty());
}

#[tokio::test]
async fn exact_replace_can_swap_existing_variable_keys() {
    let service = service().await;
    let workspace_id = service.state().await.unwrap().active_workspace_id;
    let created = service
        .replace_variables(
            workspace_id.clone(),
            vec![variable("FIRST", "one"), variable("SECOND", "two")],
        )
        .await
        .expect("create variables");
    let mut first = variable("SECOND", "one");
    first.id = Some(created[0].id.clone());
    let mut second = variable("FIRST", "two");
    second.id = Some(created[1].id.clone());

    let swapped = service
        .replace_variables(workspace_id, vec![first, second])
        .await
        .expect("swap variable keys");

    assert_eq!(swapped[0].id, created[0].id);
    assert_eq!(swapped[0].key, "SECOND");
    assert_eq!(swapped[1].id, created[1].id);
    assert_eq!(swapped[1].key, "FIRST");
}

#[tokio::test]
async fn environment_and_environment_variables_support_crud() {
    let service = service().await;
    let workspace_id = service.state().await.unwrap().active_workspace_id;
    let environment = service
        .create_environment(workspace_id.clone(), "Development".to_string())
        .await
        .expect("create environment");
    assert!(environment.variables.is_empty());

    let updated = service
        .update_environment(
            workspace_id.clone(),
            environment.id.clone(),
            "Local Development".to_string(),
            vec![variable("BASE_URL", "http://localhost:3000")],
        )
        .await
        .expect("update environment and variables");
    assert_eq!(updated.name, "Local Development");
    assert_eq!(updated.variables[0].value, "http://localhost:3000");

    let remaining = service
        .delete_environment(workspace_id, environment.id)
        .await
        .expect("delete environment");
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn variables_and_environments_are_isolated_by_workspace() {
    let service = service().await;
    let first_id = service.state().await.unwrap().active_workspace_id;
    let second = service.create("Second".to_string()).await.unwrap();
    service
        .replace_variables(first_id.clone(), vec![variable("VALUE", "first")])
        .await
        .unwrap();
    service
        .replace_variables(second.id.clone(), vec![variable("VALUE", "second")])
        .await
        .unwrap();
    service
        .create_environment(first_id.clone(), "Development".to_string())
        .await
        .unwrap();

    assert_eq!(
        service.list_variables(first_id.clone()).await.unwrap()[0].value,
        "first"
    );
    assert_eq!(
        service.list_variables(second.id.clone()).await.unwrap()[0].value,
        "second"
    );
    assert_eq!(service.list_environments(first_id).await.unwrap().len(), 1);
    assert!(service
        .list_environments(second.id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn resolver_enforces_workspace_ownership_and_precedence() {
    let service = service().await;
    let first_id = service.state().await.unwrap().active_workspace_id;
    let second = service.create("Second".to_string()).await.unwrap();
    service
        .replace_variables(
            first_id.clone(),
            vec![
                variable("BASE_URL", "https://workspace.example"),
                variable("SHARED_ONLY", "workspace"),
            ],
        )
        .await
        .unwrap();
    let environment = service
        .create_environment(first_id.clone(), "Development".to_string())
        .await
        .unwrap();
    service
        .update_environment(
            first_id.clone(),
            environment.id.clone(),
            environment.name,
            vec![variable("BASE_URL", "https://environment.example")],
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variables(
            &first_id,
            Some(&environment.id),
            "{{BASE_URL}}/{{SHARED_ONLY}}",
        )
        .await
        .expect("resolve layered variables");
    assert_eq!(resolved, "https://environment.example/workspace");

    let fallback = service
        .resolve_variables(&first_id, None, "{{BASE_URL}}")
        .await
        .expect("resolve workspace fallback");
    assert_eq!(fallback, "https://workspace.example");

    let cross_workspace = service
        .resolve_variables(&second.id, Some(&environment.id), "{{BASE_URL}}")
        .await;
    assert!(matches!(cross_workspace, Err(AppError::NotFound(_))));

    let unresolved = service
        .resolve_variables(&first_id, None, "{{MISSING}}")
        .await
        .expect_err("missing variable should fail");
    assert!(unresolved
        .to_string()
        .contains("unresolved variable: MISSING"));
}

#[tokio::test]
async fn deleting_active_environment_falls_back_to_first_available_environment() {
    let service = service().await;
    let workspace_id = service.state().await.unwrap().active_workspace_id;
    let first = service
        .create_environment(workspace_id.clone(), "Development".to_string())
        .await
        .unwrap();
    let second = service
        .create_environment(workspace_id.clone(), "Test".to_string())
        .await
        .unwrap();
    service
        .set_active_environment(workspace_id.clone(), Some(second.id.clone()))
        .await
        .unwrap();

    let remaining = service
        .delete_environment(workspace_id.clone(), second.id)
        .await
        .expect("delete active environment");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, first.id);
    assert!(remaining[0].is_active);
    assert_eq!(
        service.active_environment_id(&workspace_id).await.unwrap(),
        Some(first.id)
    );
}

#[tokio::test]
async fn first_created_environment_becomes_active() {
    let service = service().await;
    let workspace_id = service.state().await.unwrap().active_workspace_id;
    let first = service
        .create_environment(workspace_id.clone(), "Development".to_string())
        .await
        .expect("create first environment");
    let second = service
        .create_environment(workspace_id.clone(), "Test".to_string())
        .await
        .expect("create second environment");

    assert!(first.is_active);
    assert!(!second.is_active);
    assert_eq!(
        service.active_environment_id(&workspace_id).await.unwrap(),
        Some(first.id)
    );
}

#[tokio::test]
async fn variable_keys_are_case_insensitive() {
    let service = service().await;
    let workspace_id = service.state().await.unwrap().active_workspace_id;
    let error = service
        .replace_variables(
            workspace_id.clone(),
            vec![variable("BASE_URL", "a"), variable("base_url", "b")],
        )
        .await
        .expect_err("case-insensitive duplicate keys should fail");
    assert!(error
        .to_string()
        .contains("duplicate workspace variable key"));

    let environment = service
        .create_environment(workspace_id.clone(), "Development".to_string())
        .await
        .unwrap();
    let error = service
        .update_environment(
            workspace_id,
            environment.id,
            "Development".to_string(),
            vec![variable("TOKEN", "a"), variable("token", "b")],
        )
        .await
        .expect_err("case-insensitive environment keys should fail");
    assert!(error
        .to_string()
        .contains("duplicate workspace variable key"));
}

async fn live_descendant_counts(
    service: &TestWorkspaceService,
    workspace_id: &str,
) -> (i64, i64, i64) {
    sqlx::query_as(
        r#"
        SELECT
          (SELECT COUNT(*) FROM workspace_variables
           WHERE workspace_id = ?1 AND deleted_at IS NULL),
          (SELECT COUNT(*) FROM workspace_environments
           WHERE workspace_id = ?1 AND deleted_at IS NULL),
          (SELECT COUNT(*) FROM workspace_environment_variables
           WHERE workspace_id = ?1 AND deleted_at IS NULL)
        "#,
    )
    .bind(workspace_id)
    .fetch_one(service.db.pool())
    .await
    .expect("count live workspace descendants")
}

#[tokio::test]
async fn deleting_workspace_tombstones_live_variables_and_environments() {
    let service = service().await;
    let _keep = service.state().await.unwrap().active_workspace_id;
    let target = service
        .create("Scratch".to_string())
        .await
        .expect("create workspace to delete")
        .id;
    service
        .replace_variables(
            target.clone(),
            vec![variable("BASE_URL", "https://example")],
        )
        .await
        .unwrap();
    let environment = service
        .create_environment(target.clone(), "Development".to_string())
        .await
        .unwrap();
    service
        .update_environment(
            target.clone(),
            environment.id,
            "Development".to_string(),
            vec![variable("TOKEN", "secret")],
        )
        .await
        .unwrap();
    assert_eq!(live_descendant_counts(&service, &target).await, (1, 1, 1));

    service
        .delete(target.clone())
        .await
        .expect("delete workspace");

    assert_eq!(live_descendant_counts(&service, &target).await, (0, 0, 0));
    let tombstoned: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM workspaces WHERE id = ?1")
            .bind(&target)
            .fetch_one(service.db.pool())
            .await
            .unwrap();
    assert!(tombstoned.is_some());
}

#[tokio::test]
async fn applying_workspace_tombstone_cascades_leftover_live_children() {
    let service = service().await;
    let _keep = service.state().await.unwrap().active_workspace_id;
    let target = service
        .create("Scratch".to_string())
        .await
        .expect("create workspace to tombstone")
        .id;
    service
        .replace_variables(
            target.clone(),
            vec![variable("BASE_URL", "https://example")],
        )
        .await
        .unwrap();
    let environment = service
        .create_environment(target.clone(), "Development".to_string())
        .await
        .unwrap();
    service
        .update_environment(
            target.clone(),
            environment.id,
            "Development".to_string(),
            vec![variable("TOKEN", "secret")],
        )
        .await
        .unwrap();

    service
        .apply_external_page(ExternalApplyPage {
            workspaces: vec![ExternalWorkspaceApply::Delete(ExternalDelete {
                entity: DomainEntityKey::new(DomainEntityType::Workspace, &target, &target),
                deleted_at: "2026-08-19T00:00:00Z".into(),
            })],
            ..ExternalApplyPage::default()
        })
        .await
        .expect("apply parent-only workspace tombstone");

    assert_eq!(live_descendant_counts(&service, &target).await, (0, 0, 0));
}

#[tokio::test]
async fn applying_environment_variable_orphan_upsert_is_skipped() {
    let service = service().await;
    let workspace_id = service.state().await.unwrap().active_workspace_id;
    service
        .apply_external_page(ExternalApplyPage {
            workspace_environment_variables: vec![
                ExternalWorkspaceEnvironmentVariableApply::Upsert(
                    ExternalWorkspaceEnvironmentVariableUpsert {
                        id: "orphan-env-var".into(),
                        workspace_id: workspace_id.clone(),
                        environment_id: "missing-environment".into(),
                        key: "ORPHAN".into(),
                        value: ExternalVariableValue::Set("value".into()),
                        is_secret: false,
                        is_enabled: true,
                        description: None,
                        sort_order: 0,
                        created_at: "2026-08-19T00:00:00Z".into(),
                        updated_at: "2026-08-19T00:00:00Z".into(),
                    },
                ),
            ],
            ..ExternalApplyPage::default()
        })
        .await
        .expect("orphan env-var upsert should skip, not fail the page");

    let inserted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_environment_variables WHERE id = 'orphan-env-var'",
    )
    .fetch_one(service.db.pool())
    .await
    .unwrap();
    assert_eq!(inserted, 0);
}
