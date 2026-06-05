use crate::ai_reserved;
use crate::app_error::AppResult;
use crate::audit_log::AuditLogService;
use crate::local_db::LocalDb;
use crate::models::{
    ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiResponse, ApiSavedRequest,
    CredentialCreateInput, CredentialDeleteInput, CredentialMetadata, DatabaseBrowseInput,
    DatabaseBrowseResult, DatabaseConnection, DatabaseConnectionInput, DatabaseQueryInput,
    DatabaseQueryResult, DatabaseSchema, DatabaseTestResult, SshConnection, SshConnectionInput,
    SystemHealth, Workspace, WorkspaceEnvironment, WorkspaceLayout, WorkspaceState,
};
use crate::services::api_client::ApiClientService;
use crate::services::database::DatabaseService;
use crate::services::secret_store::SecretStore;
use crate::services::ssh::SshService;
use crate::services::workspace::WorkspaceService;
use crate::sync_reserved;
use tauri::AppHandle;

#[derive(Clone)]
pub struct CommandBus {
    api_client: ApiClientService,
    audit_log: AuditLogService,
    database: DatabaseService,
    secret_store: SecretStore,
    ssh: SshService,
    workspace: WorkspaceService,
}

impl CommandBus {
    pub async fn new(app: AppHandle) -> AppResult<Self> {
        let db = LocalDb::connect(&app).await?;
        db.migrate().await?;

        let audit_log = AuditLogService::new(db.clone());
        let secret_store = SecretStore::new("unfour-workspace");
        let workspace = WorkspaceService::new(db.clone());
        workspace.ensure_default_workspace().await?;

        Ok(Self {
            api_client: ApiClientService::new(db.clone()),
            audit_log,
            database: DatabaseService::new(db.clone()),
            secret_store,
            ssh: SshService::new(db.clone()),
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
        self.audit_log
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
        self.audit_log
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
        self.audit_log
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
        variables: Vec<crate::models::KeyValue>,
    ) -> AppResult<WorkspaceEnvironment> {
        let environment = self
            .workspace
            .update_environment(workspace_id.clone(), variables)
            .await?;
        self.audit_log
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
        let layout = self
            .workspace
            .update_layout(workspace_id.clone(), layout)
            .await?;
        self.audit_log
            .record(
                Some(&workspace_id),
                "workspace.layout.update",
                Some(&workspace_id),
                serde_json::json!({
                    "activeTabId": layout.active_tab_id,
                    "tabCount": layout.tabs.len(),
                    "sidebarCollapsed": layout.sidebar_collapsed
                }),
            )
            .await?;
        Ok(layout)
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
        self.audit_log
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
        self.audit_log
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
        self.audit_log
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
        self.audit_log
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
        self.audit_log
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
        self.audit_log
            .record(
                Some(&input.workspace_id),
                "credential.delete",
                Some(&input.credential_ref),
                serde_json::json!({ "deleted": true }),
            )
            .await?;
        Ok(())
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
        self.audit_log
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
        self.audit_log
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
        let result = self
            .database
            .test_connection(workspace_id.clone(), connection_id.clone())
            .await?;
        self.audit_log
            .record(
                Some(&workspace_id),
                "database.connection.test",
                Some(&connection_id),
                serde_json::json!({ "ok": result.ok }),
            )
            .await?;
        Ok(result)
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
        self.audit_log
            .record(
                Some(&input.workspace_id),
                "database.query.execute",
                Some(&input.connection_id),
                serde_json::json!({
                    "columns": result.columns.len(),
                    "rows": result.rows.len(),
                    "affectedRows": result.affected_rows
                }),
            )
            .await?;
        Ok(result)
    }

    pub async fn browse_database_table(
        &self,
        input: DatabaseBrowseInput,
    ) -> AppResult<DatabaseBrowseResult> {
        let result = self.database.browse_table(input.clone()).await?;
        self.audit_log
            .record(
                Some(&input.workspace_id),
                "database.table.browse",
                Some(&input.connection_id),
                serde_json::json!({
                    "table": input.table_name,
                    "rows": result.result.rows.len(),
                    "limit": result.limit,
                    "offset": result.offset,
                    "totalRows": result.total_rows,
                    "readOnly": result.read_only
                }),
            )
            .await?;
        Ok(result)
    }

    pub async fn list_ssh_connections(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<SshConnection>> {
        self.ssh.list_connections(workspace_id).await
    }

    pub async fn save_ssh_connection(&self, input: SshConnectionInput) -> AppResult<SshConnection> {
        let connection = self.ssh.save_connection(input).await?;
        self.audit_log
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
        self.audit_log
            .record(
                Some(&workspace_id),
                "ssh.connection.delete",
                Some(&connection_id),
                serde_json::json!({ "softDelete": true }),
            )
            .await?;
        Ok(connections)
    }

    pub fn reserved_status(&self) -> serde_json::Value {
        serde_json::json!({
            "ssh": self.ssh.capability_summary(),
            "database": self.database.capability_summary(),
            "secrets": self.secret_store.capability_summary()
        })
    }
}
