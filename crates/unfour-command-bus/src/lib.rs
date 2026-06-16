use serde::{Deserialize, Serialize};
use unfour_core::ai_reserved;
use unfour_core::models::{
    ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiResponse, ApiSavedRequest,
    CredentialCreateInput, CredentialDeleteInput, CredentialInspectInput, CredentialMetadata,
    CredentialRotateInput, DatabaseBrowseInput, DatabaseBrowseResult, DatabaseConnection,
    DatabaseConnectionInput, DatabaseQueryInput, DatabaseQueryResult, DatabaseSchema,
    DatabaseTestResult, KeyValue, SshCloseInput, SshConnectInput, SshConnection,
    SshConnectionInput, SshHostFingerprintInfo, SshHostKeyInput, SshKnownHostsExportResult,
    SshKnownHostsImportInput, SshKnownHostsImportResult, SshLogExport, SshLogExportInput,
    SshReconnectCancelInput, SshResizeInput, SshSessionEvent, SshSessionInput, SshSessionSummary,
    SystemHealth, Workspace, WorkspaceEnvironment, WorkspaceLayout, WorkspaceState,
};
use unfour_core::sync_reserved;
use unfour_core::AppResult;
use unfour_database_engine::DatabaseService;
use unfour_http_engine::ApiClientService;
use unfour_local_storage::{ActivityLogService, LocalDb};
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::SshService;
use unfour_workspace_engine::WorkspaceService;

pub const DEFAULT_APP_IDENTIFIER: &str = "com.unfour.workspace";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    All,
    Api,
    Database,
    Ssh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadCommand {
    CurrentWorkspace,
    ListConnections { connection_type: ConnectionType },
    ApiListCollections { workspace_id: Option<String> },
    ApiListRequests {
        workspace_id: Option<String>,
        collection_id: Option<String>,
    },
    ApiGetRequest { request_id: String },
}

#[derive(Debug, Clone)]
pub enum ReadCommandResult {
    CurrentWorkspace(CurrentWorkspaceResult),
    Connections(ConnectionListResult),
    ApiCollections(ApiCollectionListResult),
    ApiRequests(ApiRequestListResult),
    ApiRequest(ApiRequestDetailResult),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentWorkspaceResult {
    pub workspace_id: String,
    pub workspace_name: String,
    pub workspace_root: Option<String>,
    pub mode: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionListResult {
    pub connections: Vec<SafeConnection>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeConnection {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub connection_type: String,
    pub workspace_id: String,
    pub safe_summary: SafeConnectionSummary,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeConnectionSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionListResult {
    pub collections: Vec<ApiCollectionSummary>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionSummary {
    pub id: String,
    pub name: String,
    pub request_count: usize,
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestListResult {
    pub requests: Vec<ApiRequestSummary>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestSummary {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url_preview: String,
    pub collection_id: String,
    pub workspace_id: String,
    pub has_body: bool,
    pub header_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestDetailResult {
    pub request: ApiSavedRequest,
    pub source: String,
}

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
    pub async fn ephemeral() -> AppResult<Self> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let db = LocalDb::from_pool(pool);
        db.migrate().await?;

        Self::from_db(db).await
    }

    pub async fn from_existing_app_data_read_only() -> AppResult<Self> {
        let db = LocalDb::connect_existing_app_data_read_only(DEFAULT_APP_IDENTIFIER).await?;
        Self::from_existing_db_read_only(db).await
    }

    pub async fn from_existing_app_data() -> AppResult<Self> {
        let db = LocalDb::connect_existing_app_data(DEFAULT_APP_IDENTIFIER).await?;
        Self::from_existing_db_read_only(db).await
    }

    pub async fn from_existing_app_data_dir_read_only(
        app_data_dir: impl AsRef<std::path::Path>,
    ) -> AppResult<Self> {
        let db = LocalDb::connect_existing_read_only_path(
            app_data_dir.as_ref().join("unfour-workspace.sqlite"),
        )
        .await?;
        Self::from_existing_db_read_only(db).await
    }

    pub async fn from_db_with_secret_store(
        db: LocalDb,
        secret_store: SecretStore,
    ) -> AppResult<Self> {
        let activity_log = ActivityLogService::new(db.clone());
        let workspace = WorkspaceService::new(db.clone());
        workspace.ensure_default_workspace().await?;

        Ok(Self {
            api_client: ApiClientService::new(db.clone()),
            activity_log,
            database: DatabaseService::new(db.clone()).with_secret_store(secret_store.clone()),
            secret_store: secret_store.clone(),
            ssh: SshService::new(db, secret_store),
            workspace,
        })
    }

    /// Construct a `CommandBus` with an in-memory secret store for tests and
    /// local adapters that do not access credentials.
    pub async fn from_db(db: LocalDb) -> AppResult<Self> {
        let secret_store = SecretStore::in_memory("unfour-test");
        Self::from_db_with_secret_store(db, secret_store).await
    }

    pub async fn from_existing_db_read_only(db: LocalDb) -> AppResult<Self> {
        Self::from_db_without_workspace_seed(db, SecretStore::in_memory("unfour-mcp-read-only"))
            .await
    }

    async fn from_db_without_workspace_seed(
        db: LocalDb,
        secret_store: SecretStore,
    ) -> AppResult<Self> {
        let activity_log = ActivityLogService::new(db.clone());
        let workspace = WorkspaceService::new(db.clone());

        Ok(Self {
            api_client: ApiClientService::new(db.clone()),
            activity_log,
            database: DatabaseService::new(db.clone()).with_secret_store(secret_store.clone()),
            secret_store: secret_store.clone(),
            ssh: SshService::new(db, secret_store),
            workspace,
        })
    }

    #[cfg(feature = "ssh-native")]
    pub fn set_terminal_output_callback(
        &self,
        callback: unfour_ssh_engine::TerminalOutputCallback,
    ) {
        self.ssh.set_terminal_output_callback(callback);
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

    async fn read_workspace_state(&self) -> AppResult<WorkspaceState> {
        self.workspace.state_read_only().await
    }

    pub async fn execute_read(&self, command: ReadCommand) -> AppResult<ReadCommandResult> {
        match command {
            ReadCommand::CurrentWorkspace => {
                let state = self.read_workspace_state().await?;
                let workspace = state
                    .workspaces
                    .into_iter()
                    .find(|workspace| workspace.id == state.active_workspace_id)
                    .ok_or_else(|| {
                        unfour_core::AppError::NotFound(
                            "active workspace is not available".to_string(),
                        )
                    })?;

                Ok(ReadCommandResult::CurrentWorkspace(
                    CurrentWorkspaceResult {
                        workspace_id: workspace.id,
                        workspace_name: workspace.name,
                        workspace_root: None,
                        mode: "local".to_string(),
                        source: "command-bus".to_string(),
                    },
                ))
            }
            ReadCommand::ListConnections { connection_type } => {
                let state = self.read_workspace_state().await?;
                let workspace_id = state.active_workspace_id;
                let mut connections = Vec::new();

                if matches!(
                    connection_type,
                    ConnectionType::All | ConnectionType::Database
                ) {
                    connections.extend(
                        self.list_database_connections(workspace_id.clone())
                            .await?
                            .into_iter()
                            .map(|connection| SafeConnection {
                                id: connection.id,
                                name: connection.name,
                                connection_type: "database".to_string(),
                                workspace_id: connection.workspace_id,
                                safe_summary: SafeConnectionSummary {
                                    host: connection.host,
                                    database_type: Some(connection.driver),
                                    api_base_url: None,
                                },
                            }),
                    );
                }

                if matches!(connection_type, ConnectionType::All | ConnectionType::Ssh) {
                    connections.extend(
                        self.list_ssh_connections(workspace_id)
                            .await?
                            .into_iter()
                            .map(|connection| SafeConnection {
                                id: connection.id,
                                name: connection.name,
                                connection_type: "ssh".to_string(),
                                workspace_id: connection.workspace_id,
                                safe_summary: SafeConnectionSummary {
                                    host: Some(connection.host),
                                    database_type: None,
                                    api_base_url: None,
                                },
                            }),
                    );
                }

                Ok(ReadCommandResult::Connections(ConnectionListResult {
                    count: connections.len(),
                    connections,
                    source: "command-bus".to_string(),
                }))
            }
            ReadCommand::ApiListCollections { workspace_id } => {
                let state = self.read_workspace_state().await?;
                let ws_id = workspace_id.unwrap_or(state.active_workspace_id);
                let requests = self.api_client.list_saved_requests(ws_id.clone()).await?;

                let mut folder_counts: std::collections::BTreeMap<String, usize> =
                    std::collections::BTreeMap::new();
                for request in &requests {
                    let folder = request.folder_path.clone().unwrap_or_default();
                    *folder_counts.entry(folder).or_insert(0) += 1;
                }

                let collections = folder_counts
                    .into_iter()
                    .map(|(folder, count)| {
                        let name = if folder.is_empty() {
                            "General".to_string()
                        } else {
                            folder.clone()
                        };
                        let id = if folder.is_empty() {
                            String::new()
                        } else {
                            folder.clone()
                        };
                        ApiCollectionSummary {
                            id,
                            name,
                            request_count: count,
                            workspace_id: ws_id.clone(),
                        }
                    })
                    .collect::<Vec<_>>();

                Ok(ReadCommandResult::ApiCollections(
                    ApiCollectionListResult {
                        count: collections.len(),
                        collections,
                        source: "command-bus".to_string(),
                    },
                ))
            }
            ReadCommand::ApiListRequests {
                workspace_id,
                collection_id,
            } => {
                let state = self.read_workspace_state().await?;
                let ws_id = workspace_id.unwrap_or(state.active_workspace_id);
                let requests = self.api_client.list_saved_requests(ws_id.clone()).await?;

                let filtered: Vec<_> = if let Some(ref cid) = collection_id {
                    requests
                        .into_iter()
                        .filter(|r| r.folder_path.as_deref().unwrap_or("") == cid.as_str())
                        .collect()
                } else {
                    requests
                };

                let summaries = filtered
                    .into_iter()
                    .map(|r| {
                        let header_count = serde_json::from_str::<Vec<serde_json::Value>>(
                            &r.headers_json,
                        )
                        .map(|v| v.len())
                        .unwrap_or(0);
                        let has_body = r.body.as_ref().is_some_and(|b| !b.is_empty());
                        let url_preview = truncate_url_preview(&r.url);
                        ApiRequestSummary {
                            id: r.id,
                            name: r.name,
                            method: r.method,
                            url_preview,
                            collection_id: r.folder_path.unwrap_or_default(),
                            workspace_id: r.workspace_id,
                            has_body,
                            header_count,
                        }
                    })
                    .collect::<Vec<_>>();

                Ok(ReadCommandResult::ApiRequests(ApiRequestListResult {
                    count: summaries.len(),
                    requests: summaries,
                    source: "command-bus".to_string(),
                }))
            }
            ReadCommand::ApiGetRequest { request_id } => {
                let request = self.api_client.get_saved_request(&request_id).await?;
                Ok(ReadCommandResult::ApiRequest(
                    ApiRequestDetailResult {
                        request,
                        source: "command-bus".to_string(),
                    },
                ))
            }
        }
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

    pub async fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms_override: Option<u64>,
    ) -> AppResult<ApiResponse> {
        let state = self.read_workspace_state().await?;
        let workspace_id = state.active_workspace_id;
        let saved = self.api_client.get_saved_request(request_id).await?;

        if saved.workspace_id != workspace_id {
            return Err(unfour_core::AppError::NotFound(
                "api request".to_string(),
            ));
        }

        let headers: Vec<KeyValue> =
            serde_json::from_str(&saved.headers_json).unwrap_or_default();
        let query: Vec<KeyValue> =
            serde_json::from_str(&saved.query_json).unwrap_or_default();
        let timeout_ms = timeout_ms_override
            .map(|t| t.min(60_000));

        let input = ApiRequestInput {
            workspace_id: saved.workspace_id.clone(),
            name: Some(saved.name.clone()),
            folder_path: saved.folder_path.clone(),
            method: saved.method.clone(),
            url: saved.url.clone(),
            headers,
            query,
            body: saved.body.clone(),
            body_kind: saved.body_kind.clone(),
            timeout_ms,
        };

        let environment = self.workspace.environment(workspace_id).await?;
        self.api_client.send(input, &environment.variables).await
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
        self.ssh.list_sessions(workspace_id).await
    }

    pub async fn ssh_session_history(
        &self,
        input: SshCloseInput,
    ) -> AppResult<Vec<SshSessionEvent>> {
        self.ssh.session_history(input).await
    }

    pub async fn send_ssh_input(&self, input: SshSessionInput) -> AppResult<SshSessionEvent> {
        self.ssh.send_input(input).await
    }

    pub async fn resize_ssh_session(&self, input: SshResizeInput) -> AppResult<SshSessionEvent> {
        self.ssh.resize(input).await
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

    pub async fn cancel_ssh_reconnect(
        &self,
        input: SshReconnectCancelInput,
    ) -> AppResult<SshSessionSummary> {
        self.ssh.cancel_reconnect(input).await
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

    pub async fn get_ssh_host_fingerprint(
        &self,
        input: SshHostKeyInput,
    ) -> AppResult<Option<SshHostFingerprintInfo>> {
        self.ssh.get_host_fingerprint(input).await
    }

    pub async fn reset_ssh_host_fingerprint(&self, input: SshHostKeyInput) -> AppResult<bool> {
        let host = input.host.clone();
        let port = input.port;
        let deleted = self.ssh.reset_host_fingerprint(input).await?;
        if deleted {
            self.activity_log
                .record(
                    None,
                    "ssh.host_key.reset",
                    Some(&format!("{}:{}", host, port)),
                    serde_json::json!({ "host": host, "port": port }),
                )
                .await?;
        }
        Ok(deleted)
    }

    pub async fn list_all_ssh_fingerprints(&self) -> AppResult<Vec<SshHostFingerprintInfo>> {
        self.ssh.list_all_host_fingerprints().await
    }

    pub async fn import_ssh_known_hosts(
        &self,
        input: SshKnownHostsImportInput,
    ) -> AppResult<SshKnownHostsImportResult> {
        let result = self.ssh.import_known_hosts(input).await?;
        self.activity_log
            .record(
                None,
                "ssh.known_hosts.import",
                None,
                serde_json::json!({
                    "imported": result.imported,
                    "skipped": result.skipped,
                    "errors": result.errors.len(),
                }),
            )
            .await?;
        Ok(result)
    }

    pub async fn export_ssh_known_hosts(&self) -> AppResult<SshKnownHostsExportResult> {
        self.ssh.export_known_hosts().await
    }

    pub fn reserved_status(&self) -> serde_json::Value {
        serde_json::json!({
            "ssh": self.ssh.capability_summary(),
            "database": self.database.capability_summary(),
            "secrets": self.secret_store.capability_summary()
        })
    }
}

fn truncate_url_preview(url: &str) -> String {
    const MAX_LEN: usize = 200;
    if url.len() <= MAX_LEN {
        url.to_string()
    } else {
        let mut truncated: String = url.chars().take(MAX_LEN).collect();
        truncated.push_str("...");
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use unfour_core::models::{ApiRequestInput, DatabaseConnectionInput, SshConnectionInput};
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

    #[tokio::test]
    async fn read_commands_return_current_workspace_and_safe_connections() {
        let bus = test_bus().await;
        let workspace = bus
            .execute_read(ReadCommand::CurrentWorkspace)
            .await
            .expect("read current workspace");
        let ReadCommandResult::CurrentWorkspace(workspace) = workspace else {
            panic!("expected current workspace result");
        };
        assert_eq!(workspace.source, "command-bus");
        assert_eq!(workspace.workspace_root, None);

        bus.save_database_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: workspace.workspace_id.clone(),
            name: "Database".to_string(),
            driver: "postgres".to_string(),
            host: Some("db.internal".to_string()),
            port: Some(5432),
            database: Some("app".to_string()),
            username: Some("developer".to_string()),
            sqlite_path: None,
            credential_ref: Some("database-secret".to_string()),
        })
        .await
        .expect("save database connection");
        bus.save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace.workspace_id,
            name: "SSH".to_string(),
            host: "ssh.internal".to_string(),
            port: Some(22),
            username: "developer".to_string(),
            auth_kind: "private-key".to_string(),
            key_path: Some("C:\\sensitive\\id_ed25519".to_string()),
            credential_ref: Some("ssh-secret".to_string()),
        })
        .await
        .expect("save ssh connection");

        let result = bus
            .execute_read(ReadCommand::ListConnections {
                connection_type: ConnectionType::All,
            })
            .await
            .expect("list safe connections");
        let ReadCommandResult::Connections(result) = result else {
            panic!("expected connection list result");
        };
        assert_eq!(result.count, 2);
        assert_eq!(result.source, "command-bus");

        let json = serde_json::to_string(&result).expect("serialize safe result");
        assert!(!json.contains("credential"));
        assert!(!json.contains("developer"));
        assert!(!json.contains("id_ed25519"));
        assert!(json.contains("db.internal"));
        assert!(json.contains("ssh.internal"));
    }

    #[tokio::test]
    async fn api_connection_filter_is_empty_until_an_api_connection_model_exists() {
        let bus = test_bus().await;
        let result = bus
            .execute_read(ReadCommand::ListConnections {
                connection_type: ConnectionType::Api,
            })
            .await
            .expect("list api connections");
        let ReadCommandResult::Connections(result) = result else {
            panic!("expected connection list result");
        };

        assert!(result.connections.is_empty());
        assert_eq!(result.count, 0);
    }
}
