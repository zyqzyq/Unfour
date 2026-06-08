use tauri::AppHandle;
use unfour_core::ai_reserved;
use unfour_core::models::{
    ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiResponse, ApiSavedRequest,
    CredentialCreateInput, CredentialDeleteInput, CredentialInspectInput, CredentialMetadata,
    CredentialRotateInput, DatabaseBrowseInput, DatabaseBrowseResult, DatabaseConnection,
    DatabaseConnectionInput, DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema,
    DatabaseTestResult, KeyValue, SshCloseInput, SshConnectInput, SshConnection,
    SshConnectionInput, SshLogExport, SshLogExportInput, SshResizeInput, SshSessionEvent,
    SshSessionInput, SshSessionSummary, SystemHealth, Workspace, WorkspaceEnvironment,
    WorkspaceLayout, WorkspaceState,
};
use unfour_core::sync_reserved;
use unfour_core::AppResult;
use unfour_database_engine::DatabaseService;
use unfour_http_engine::ApiClientService;
use unfour_local_storage::{ActivityLogService, LocalDb};
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::SshService;
use unfour_workspace_engine::WorkspaceService;

#[derive(Clone)]
pub struct CommandBus {
    api_client: ApiClientService,
    activity_log: ActivityLogService,
    database: DatabaseService,
    secret_store: SecretStore,
    ssh: SshService,
    workspace: WorkspaceService,
}

impl CommandBus {
    pub async fn new(app: AppHandle) -> AppResult<Self> {
        let db = LocalDb::connect(&app).await?;
        db.migrate().await?;

        let activity_log = ActivityLogService::new(db.clone());
        let secret_store = SecretStore::new("unfour-workspace");
        let workspace = WorkspaceService::new(db.clone());
        workspace.ensure_default_workspace().await?;

        Ok(Self {
            api_client: ApiClientService::new(db.clone()),
            activity_log,
            database: DatabaseService::new(db.clone()),
            secret_store: secret_store.clone(),
            ssh: SshService::new(db.clone(), secret_store),
            workspace,
        })
    }

    /// Construct a `CommandBus` from a pre-built `LocalDb` without requiring a
    /// Tauri `AppHandle`. This is the primary testability seam for integration
    /// tests that use in-memory SQLite.
    pub async fn from_db(db: LocalDb) -> AppResult<Self> {
        let activity_log = ActivityLogService::new(db.clone());
        let secret_store = SecretStore::in_memory("unfour-test");
        let workspace = WorkspaceService::new(db.clone());
        workspace.ensure_default_workspace().await?;

        Ok(Self {
            api_client: ApiClientService::new(db.clone()),
            activity_log,
            database: DatabaseService::new(db.clone()),
            secret_store: secret_store.clone(),
            ssh: SshService::new(db.clone(), secret_store),
            workspace,
        })
    }

    pub async fn system_health(&self) -> AppResult<SystemHealth> {
        Ok(SystemHealth {
            app_name: "Unfour Workspace".to_string(),
            storage_ready: true,
            command_bus_ready: true,
            ai_reserved_capabilities: ai_reserved::capability_ids(),
            sync_strategy: sync_reserved::default_policy().strategy,
        })
    }

    pub async fn list_workspaces(&self) -> AppResult<WorkspaceState> {
        self.workspace.state().await
    }

    pub async fn create_workspace(&self, name: String) -> AppResult<Workspace> {
        let workspace = self.workspace.create(name).await?;
        self.activity_log
            .record(
                Some(&workspace.id),
                "workspace.create",
                Some(&workspace.id),
                serde_json::json!({ "name": workspace.name }),
            )
            .await?;
        Ok(workspace)
    }

    pub async fn rename_workspace(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<Workspace> {
        let workspace = self.workspace.rename(workspace_id, name).await?;
        self.activity_log
            .record(
                Some(&workspace.id),
                "workspace.rename",
                Some(&workspace.id),
                serde_json::json!({ "name": workspace.name }),
            )
            .await?;
        Ok(workspace)
    }

    pub async fn delete_workspace(&self, workspace_id: String) -> AppResult<WorkspaceState> {
        let state = self.workspace.delete(workspace_id.clone()).await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "workspace.delete",
                Some(&workspace_id),
                serde_json::json!({ "softDelete": true }),
            )
            .await?;
        Ok(state)
    }

    pub async fn set_active_workspace(&self, workspace_id: String) -> AppResult<WorkspaceState> {
        self.workspace.set_active(workspace_id).await
    }

    pub async fn workspace_environment(
        &self,
        workspace_id: String,
    ) -> AppResult<WorkspaceEnvironment> {
        self.workspace.environment(workspace_id).await
    }

    pub async fn workspace_environment_update(
        &self,
        workspace_id: String,
        variables: Vec<KeyValue>,
    ) -> AppResult<WorkspaceEnvironment> {
        let environment = self
            .workspace
            .update_environment(workspace_id.clone(), variables)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "workspace.environment.update",
                Some(&workspace_id),
                serde_json::json!({ "variableCount": environment.variables.len() }),
            )
            .await?;
        Ok(environment)
    }

    pub async fn workspace_layout(&self, workspace_id: String) -> AppResult<WorkspaceLayout> {
        self.workspace.layout(workspace_id).await
    }

    pub async fn workspace_layout_update(
        &self,
        workspace_id: String,
        layout: WorkspaceLayout,
    ) -> AppResult<WorkspaceLayout> {
        self.workspace.update_layout(workspace_id, layout).await
    }

    pub async fn send_api_request(&self, input: ApiRequestInput) -> AppResult<ApiResponse> {
        let environment = self
            .workspace
            .environment(input.workspace_id.clone())
            .await?;
        let response = self
            .api_client
            .send(input.clone(), &environment.variables)
            .await?;
        self.activity_log
            .record(
                Some(&input.workspace_id),
                "api.send_request",
                Some(&response.history_id),
                serde_json::json!({
                    "method": input.method,
                    "url": input.url,
                    "status": response.status
                }),
            )
            .await?;
        Ok(response)
    }

    pub async fn list_api_history(
        &self,
        workspace_id: String,
        limit: Option<i64>,
    ) -> AppResult<Vec<ApiHistoryItem>> {
        self.api_client.list_history(workspace_id, limit).await
    }

    pub async fn api_history_detail(
        &self,
        workspace_id: String,
        history_id: String,
    ) -> AppResult<ApiHistoryDetail> {
        self.api_client
            .history_detail(workspace_id, history_id)
            .await
    }

    pub async fn save_api_request(&self, input: ApiRequestInput) -> AppResult<ApiSavedRequest> {
        let saved = self.api_client.save_request(input).await?;
        self.activity_log
            .record(
                Some(&saved.workspace_id),
                "api.save_request",
                Some(&saved.id),
                serde_json::json!({ "name": saved.name, "method": saved.method }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn list_saved_api_requests(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        self.api_client.list_saved_requests(workspace_id).await
    }

    pub async fn duplicate_api_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<ApiSavedRequest> {
        let saved = self
            .api_client
            .duplicate_request(workspace_id.clone(), request_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.duplicate_request",
                Some(&saved.id),
                serde_json::json!({ "sourceId": request_id, "name": saved.name }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn delete_api_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        let requests = self
            .api_client
            .delete_request(workspace_id.clone(), request_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.delete_request",
                Some(&request_id),
                serde_json::json!({ "softDelete": true }),
            )
            .await?;
        Ok(requests)
    }

    pub async fn create_credential(
        &self,
        input: CredentialCreateInput,
    ) -> AppResult<CredentialMetadata> {
        let credential = self
            .secret_store
            .create_credential(input.workspace_id, input.kind, input.label, input.secret)
            .await?;
        self.activity_log
            .record(
                Some(&credential.workspace_id),
                "credential.create",
                Some(&credential.credential_ref),
                serde_json::json!({
                    "kind": credential.kind,
                    "label": credential.label,
                    "secretStored": true
                }),
            )
            .await?;
        Ok(credential)
    }

    pub async fn delete_credential(&self, input: CredentialDeleteInput) -> AppResult<()> {
        self.secret_store
            .delete_credential(input.workspace_id.clone(), input.credential_ref.clone())
            .await?;
        self.activity_log
            .record(
                Some(&input.workspace_id),
                "credential.delete",
                Some(&input.credential_ref),
                serde_json::json!({ "deleted": true }),
            )
            .await?;
        Ok(())
    }

    pub async fn inspect_credential(
        &self,
        input: CredentialInspectInput,
    ) -> AppResult<CredentialMetadata> {
        self.secret_store
            .inspect_credential(input.workspace_id, input.credential_ref)
            .await
    }

    pub async fn rotate_credential(
        &self,
        input: CredentialRotateInput,
    ) -> AppResult<CredentialMetadata> {
        let credential = self
            .secret_store
            .rotate_credential(input.workspace_id, input.credential_ref, input.secret)
            .await?;
        self.activity_log
            .record(
                Some(&credential.workspace_id),
                "credential.rotate",
                Some(&credential.credential_ref),
                serde_json::json!({
                    "kind": credential.kind,
                    "secretStored": true
                }),
            )
            .await?;
        Ok(credential)
    }

    pub async fn list_database_connections(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        self.database.list_connections(workspace_id).await
    }

    pub async fn save_database_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> AppResult<DatabaseConnection> {
        let connection = self.database.save_connection(input).await?;
        self.activity_log
            .record(
                Some(&connection.workspace_id),
                "database.connection.save",
                Some(&connection.id),
                serde_json::json!({
                    "name": connection.name,
                    "driver": connection.driver,
                    "credentialRef": connection.credential_ref.is_some()
                }),
            )
            .await?;
        Ok(connection)
    }

    pub async fn delete_database_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<DatabaseConnection>> {
        let connections = self
            .database
            .delete_connection(workspace_id.clone(), connection_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "database.connection.delete",
                Some(&connection_id),
                serde_json::json!({ "softDelete": true }),
            )
            .await?;
        Ok(connections)
    }

    pub async fn test_database_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<DatabaseTestResult> {
        self.database
            .test_connection(workspace_id, connection_id)
            .await
    }

    pub async fn database_schema(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<DatabaseSchema> {
        self.database.schema(workspace_id, connection_id).await
    }

    pub async fn execute_database_query(
        &self,
        input: DatabaseQueryInput,
    ) -> AppResult<DatabaseQueryResult> {
        let result = self.database.execute_query(input.clone()).await?;
        if result.safety.classification != "read" {
            let classification = result.safety.classification.clone();
            self.activity_log
                .record(
                    Some(&input.workspace_id),
                    "database.query.execute",
                    Some(&input.connection_id),
                    serde_json::json!({
                        "classification": classification,
                        "confirmed": result.safety.confirmed,
                        "columns": result.columns.len(),
                        "rows": result.rows.len(),
                        "affectedRows": result.affected_rows
                    }),
                )
                .await?;
        }
        Ok(result)
    }

    pub async fn browse_database_table(
        &self,
        input: DatabaseBrowseInput,
    ) -> AppResult<DatabaseBrowseResult> {
        self.database.browse_table(input).await
    }

    pub async fn list_ssh_connections(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<SshConnection>> {
        self.ssh.list_connections(workspace_id).await
    }

    pub async fn save_ssh_connection(&self, input: SshConnectionInput) -> AppResult<SshConnection> {
        let connection = self.ssh.save_connection(input).await?;
        self.activity_log
            .record(
                Some(&connection.workspace_id),
                "ssh.connection.save",
                Some(&connection.id),
                serde_json::json!({
                    "name": connection.name,
                    "host": connection.host,
                    "authKind": connection.auth_kind,
                    "credentialRef": connection.credential_ref.is_some()
                }),
            )
            .await?;
        Ok(connection)
    }

    pub async fn delete_ssh_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<SshConnection>> {
        let connections = self
            .ssh
            .delete_connection(workspace_id.clone(), connection_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "ssh.connection.delete",
                Some(&connection_id),
                serde_json::json!({ "softDelete": true }),
            )
            .await?;
        Ok(connections)
    }

    pub async fn connect_ssh_session(
        &self,
        input: SshConnectInput,
    ) -> AppResult<SshSessionSummary> {
        let session = self.ssh.connect(input.clone()).await?;
        self.activity_log
            .record(
                Some(&input.workspace_id),
                "ssh.session.connect",
                Some(&session.session_id),
                serde_json::json!({
                    "connectionId": input.connection_id,
                    "authKind": session.auth_kind,
                    "host": session.host,
                    "pty": {
                        "cols": session.cols,
                        "rows": session.rows
                    }
                }),
            )
            .await?;
        Ok(session)
    }

    pub async fn list_ssh_sessions(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<SshSessionSummary>> {
        self.ssh.list_sessions(workspace_id)
    }

    pub async fn send_ssh_input(&self, input: SshSessionInput) -> AppResult<SshSessionEvent> {
        self.ssh.send_input(input)
    }

    pub async fn resize_ssh_session(&self, input: SshResizeInput) -> AppResult<SshSessionEvent> {
        self.ssh.resize(input)
    }

    pub async fn close_ssh_session(&self, input: SshCloseInput) -> AppResult<SshSessionSummary> {
        let session = self.ssh.close_session(input.clone()).await?;
        self.activity_log
            .record(
                Some(&input.workspace_id),
                "ssh.session.close",
                Some(&input.session_id),
                serde_json::json!({ "status": session.status }),
            )
            .await?;
        Ok(session)
    }

    pub async fn export_ssh_log(&self, input: SshLogExportInput) -> AppResult<SshLogExport> {
        let export = self.ssh.export_log(input.clone())?;
        self.activity_log
            .record(
                Some(&input.workspace_id),
                "ssh.session.log_export",
                Some(&input.session_id),
                serde_json::json!({
                    "lineCount": export.line_count,
                    "redacted": export.redacted
                }),
            )
            .await?;
        Ok(export)
    }

    pub fn reserved_status(&self) -> serde_json::Value {
        serde_json::json!({
            "ssh": self.ssh.capability_summary(),
            "database": self.database.capability_summary(),
            "secrets": self.secret_store.capability_summary()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use unfour_core::models::ApiRequestInput;
    use unfour_local_storage::LocalDb;

    async fn test_bus() -> CommandBus {
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
        CommandBus::from_db(db).await.expect("build command bus")
    }

    #[tokio::test]
    async fn workspace_create_and_list() {
        let bus = test_bus().await;

        // from_db seeds a default workspace, so list should have at least one
        let initial_state = bus.list_workspaces().await.expect("list workspaces");
        let initial_count = initial_state.workspaces.len();
        assert!(
            initial_count >= 1,
            "should have the default workspace seeded"
        );

        // Create a new workspace
        let created = bus
            .create_workspace("Integration Test WS".to_string())
            .await
            .expect("create workspace");
        assert_eq!(created.name, "Integration Test WS");
        assert!(!created.id.is_empty());

        // List should now include the new workspace
        let state = bus.list_workspaces().await.expect("list workspaces");
        assert_eq!(state.workspaces.len(), initial_count + 1);
        assert!(
            state.workspaces.iter().any(|w| w.id == created.id),
            "newly created workspace should appear in the list"
        );

        // The new workspace should be active (create sets it active)
        assert_eq!(state.active_workspace_id, created.id);
    }

    #[tokio::test]
    async fn save_and_list_api_requests() {
        let bus = test_bus().await;

        // Get the default workspace
        let state = bus.list_workspaces().await.expect("list workspaces");
        let workspace_id = state.active_workspace_id.clone();

        // Initially no saved requests
        let initial = bus
            .list_saved_api_requests(workspace_id.clone())
            .await
            .expect("list saved requests");
        assert!(initial.is_empty(), "no saved requests initially");

        // Save a request
        let input = ApiRequestInput {
            workspace_id: workspace_id.clone(),
            name: Some("Test GET request".to_string()),
            folder_path: None,
            method: "GET".to_string(),
            url: "https://httpbin.org/get".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "none".to_string(),
            timeout_ms: None,
        };

        let saved = bus.save_api_request(input).await.expect("save api request");
        assert_eq!(saved.name, "Test GET request");
        assert_eq!(saved.method, "GET");
        assert_eq!(saved.workspace_id, workspace_id);

        // List should now have one request
        let listed = bus
            .list_saved_api_requests(workspace_id.clone())
            .await
            .expect("list saved requests");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, saved.id);
        assert_eq!(listed[0].name, "Test GET request");

        // Save a second request
        let input2 = ApiRequestInput {
            workspace_id: workspace_id.clone(),
            name: Some("Test POST request".to_string()),
            folder_path: Some("auth".to_string()),
            method: "POST".to_string(),
            url: "https://httpbin.org/post".to_string(),
            headers: vec![],
            query: vec![],
            body: Some(r#"{"key":"value"}"#.to_string()),
            body_kind: "json".to_string(),
            timeout_ms: None,
        };

        let saved2 = bus
            .save_api_request(input2)
            .await
            .expect("save second api request");
        assert_eq!(saved2.folder_path.as_deref(), Some("auth"));

        let listed2 = bus
            .list_saved_api_requests(workspace_id)
            .await
            .expect("list saved requests after second save");
        assert_eq!(listed2.len(), 2);
    }

    #[tokio::test]
    async fn workspace_rename_updates_state() {
        let bus = test_bus().await;

        let created = bus
            .create_workspace("Rename Me".to_string())
            .await
            .expect("create workspace");

        let renamed = bus
            .rename_workspace(created.id.clone(), "Renamed Workspace".to_string())
            .await
            .expect("rename workspace");
        assert_eq!(renamed.name, "Renamed Workspace");
        assert_eq!(renamed.id, created.id);

        let state = bus.list_workspaces().await.expect("list workspaces");
        let ws = state
            .workspaces
            .iter()
            .find(|w| w.id == created.id)
            .expect("workspace should still exist");
        assert_eq!(ws.name, "Renamed Workspace");
    }
}
