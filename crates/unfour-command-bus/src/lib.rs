mod activity_summary;
mod storage_paths;

use activity_summary::{command_activity_kind, truncate_url_preview};
use serde::{Deserialize, Serialize};
pub use storage_paths::{default_database_path, DEFAULT_DATABASE_FILE, DEFAULT_PRODUCT_DATA_DIR};
use unfour_core::ai_reserved;
use unfour_core::models::{
    ApiCollection, ApiCollectionFolder, ApiEnvironment, ApiHistoryDetail, ApiHistoryItem,
    ApiRequestInput, ApiResponse, ApiSavedRequest, CredentialCreateInput, CredentialDeleteInput,
    CredentialInspectInput, CredentialMetadata, CredentialRotateInput, DatabaseBrowseInput,
    DatabaseBrowseResult, DatabaseConnection, DatabaseConnectionInput, DatabaseQueryInput,
    DatabaseQueryResult, DatabaseRowMutationInput, DatabaseRowMutationResult, DatabaseSchema,
    DatabaseTableStructure, DatabaseTableStructureInput, DatabaseTestResult, DbQueryHistoryEntry,
    DbQueryHistoryRecordInput, KeyValue, SavedSql, SavedSqlInput, SshCloseInput, SshConnectInput,
    SshConnection, SshConnectionInput, SshDiagnosticInput, SshDiagnosticResult,
    SshHostFingerprintInfo, SshHostKeyInput, SshKnownHostsExportInput, SshKnownHostsExportResult,
    SshKnownHostsImportInput, SshKnownHostsImportResult, SshLogExport, SshLogExportInput,
    SshReconnectCancelInput, SshResizeInput, SshSessionEvent, SshSessionInput, SshSessionSummary,
    SystemHealth, Workspace, WorkspaceLayout, WorkspaceState,
};
use unfour_core::sync_reserved;
use unfour_core::AppResult;
use unfour_database_engine::DatabaseService;
use unfour_http_engine::ApiClientService;
use unfour_local_storage::{ActivityLogService, LocalDb};
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::SshService;
use unfour_workspace_engine::WorkspaceService;

/// OS keychain service name under which credentials are stored. Must match the
/// value the desktop app passes to `SecretStore::new` (see
/// `apps/desktop/src-tauri/src/lib.rs`) so satellite processes read the same
/// credential entries.
pub const DEFAULT_SECRET_SERVICE: &str = "unfour";

/// Default and maximum number of activity events returned by `ListActivity`.
const DEFAULT_ACTIVITY_LIMIT: i64 = 50;
const MAX_ACTIVITY_LIMIT: i64 = 200;

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
    ListWorkspaces,
    ListConnections {
        connection_type: ConnectionType,
    },
    ApiListCollections {
        workspace_id: Option<String>,
    },
    ApiListRequests {
        workspace_id: Option<String>,
        collection_id: Option<String>,
    },
    ApiGetRequest {
        request_id: String,
    },
    ApiListHistory {
        workspace_id: Option<String>,
        limit: Option<i64>,
    },
    ApiGetHistory {
        workspace_id: Option<String>,
        history_id: String,
    },
    ApiListEnvironments {
        workspace_id: Option<String>,
    },
    ListActivity {
        workspace_id: Option<String>,
        limit: Option<i64>,
    },
}

#[derive(Debug, Clone)]
pub enum ReadCommandResult {
    CurrentWorkspace(CurrentWorkspaceResult),
    Workspaces(WorkspaceListResult),
    Connections(ConnectionListResult),
    ApiCollections(ApiCollectionListResult),
    ApiRequests(ApiRequestListResult),
    ApiRequest(ApiRequestDetailResult),
    ApiHistory(ApiHistoryListResult),
    ApiHistoryDetailResult(ApiHistoryDetailResult),
    ApiEnvironments(ApiEnvironmentListResult),
    Activity(ActivityListResult),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentWorkspaceResult {
    pub workspace_id: String,
    pub workspace_name: String,
    pub environment_type: String,
    pub mcp_policy: String,
    pub workspace_root: Option<String>,
    pub mode: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListResult {
    pub workspaces: Vec<WorkspaceSummary>,
    pub active_workspace_id: String,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_active: bool,
    pub environment_type: String,
    pub mcp_policy: String,
    pub last_opened_at: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiHistoryListResult {
    pub history: Vec<ApiHistoryItem>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiHistoryDetailResult {
    pub detail: ApiHistoryDetail,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEnvironmentListResult {
    pub environments: Vec<ApiEnvironment>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListResult {
    pub activity: Vec<ActivityItem>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: String,
    pub workspace_id: Option<String>,
    pub action: String,
    pub target: Option<String>,
    /// Redacted summary payload recorded with the event. Consumers that surface
    /// this to an LLM apply an additional masking pass as defense-in-depth.
    pub details: serde_json::Value,
    pub created_at: String,
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
        // Use the real OS keychain (same service the desktop app writes to) so
        // satellite processes such as the MCP server can resolve saved
        // credentials for database connections. Only read operations are
        // exercised here; no tool creates, rotates, or deletes credentials.
        Self::from_db_without_workspace_seed(db, SecretStore::new(DEFAULT_SECRET_SERVICE)).await
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
            app_name: "Unfour".to_string(),
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
                        environment_type: workspace.environment_type,
                        mcp_policy: workspace.mcp_policy,
                        workspace_root: None,
                        mode: "local".to_string(),
                        source: "command-bus".to_string(),
                    },
                ))
            }
            ReadCommand::ListWorkspaces => {
                let state = self.read_workspace_state().await?;
                let active_workspace_id = state.active_workspace_id.clone();
                let workspaces = state
                    .workspaces
                    .into_iter()
                    .map(|workspace| WorkspaceSummary {
                        is_active: workspace.id == active_workspace_id,
                        id: workspace.id,
                        name: workspace.name,
                        is_default: workspace.is_default,
                        last_opened_at: workspace.last_opened_at,
                        environment_type: workspace.environment_type,
                        mcp_policy: workspace.mcp_policy,
                    })
                    .collect::<Vec<_>>();

                Ok(ReadCommandResult::Workspaces(WorkspaceListResult {
                    count: workspaces.len(),
                    workspaces,
                    active_workspace_id,
                    source: "command-bus".to_string(),
                }))
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
                let collections = self.api_client.list_collections(ws_id.clone()).await?;
                let requests = self.api_client.list_saved_requests(ws_id.clone()).await?;

                let mut request_counts: std::collections::BTreeMap<String, usize> =
                    std::collections::BTreeMap::new();
                for request in &requests {
                    *request_counts
                        .entry(request.collection_id.clone())
                        .or_insert(0) += 1;
                }

                let summaries = collections
                    .into_iter()
                    .map(|collection| {
                        let count = request_counts.remove(&collection.id).unwrap_or(0);
                        ApiCollectionSummary {
                            id: collection.id,
                            name: collection.name,
                            request_count: count,
                            workspace_id: collection.workspace_id,
                        }
                    })
                    .collect::<Vec<_>>();

                Ok(ReadCommandResult::ApiCollections(ApiCollectionListResult {
                    count: summaries.len(),
                    collections: summaries,
                    source: "command-bus".to_string(),
                }))
            }
            ReadCommand::ApiListRequests {
                workspace_id,
                collection_id,
            } => {
                let state = self.read_workspace_state().await?;
                let ws_id = workspace_id.unwrap_or(state.active_workspace_id);
                let requests = self.api_client.list_saved_requests(ws_id.clone()).await?;

                let filtered: Vec<_> = if let Some(ref cid) = collection_id {
                    let cid = cid.trim();
                    requests
                        .into_iter()
                        .filter(|r| r.collection_id == cid)
                        .collect()
                } else {
                    requests
                };

                let summaries = filtered
                    .into_iter()
                    .map(|r| {
                        let header_count =
                            serde_json::from_str::<Vec<serde_json::Value>>(&r.headers_json)
                                .map(|v| v.len())
                                .unwrap_or(0);
                        let has_body = r.body.as_ref().is_some_and(|b| !b.is_empty());
                        let url_preview = truncate_url_preview(&r.url);
                        ApiRequestSummary {
                            id: r.id,
                            name: r.name,
                            method: r.method,
                            url_preview,
                            collection_id: r.collection_id,
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
                Ok(ReadCommandResult::ApiRequest(ApiRequestDetailResult {
                    request,
                    source: "command-bus".to_string(),
                }))
            }
            ReadCommand::ApiListHistory {
                workspace_id,
                limit,
            } => {
                let state = self.read_workspace_state().await?;
                let ws_id = workspace_id.unwrap_or(state.active_workspace_id);
                let history = self.api_client.list_history(ws_id, limit).await?;
                Ok(ReadCommandResult::ApiHistory(ApiHistoryListResult {
                    count: history.len(),
                    history,
                    source: "command-bus".to_string(),
                }))
            }
            ReadCommand::ApiGetHistory {
                workspace_id,
                history_id,
            } => {
                let state = self.read_workspace_state().await?;
                let ws_id = workspace_id.unwrap_or(state.active_workspace_id);
                let detail = self.api_client.history_detail(ws_id, history_id).await?;
                Ok(ReadCommandResult::ApiHistoryDetailResult(
                    ApiHistoryDetailResult {
                        detail,
                        source: "command-bus".to_string(),
                    },
                ))
            }
            ReadCommand::ApiListEnvironments { workspace_id } => {
                let state = self.read_workspace_state().await?;
                let ws_id = workspace_id.unwrap_or(state.active_workspace_id);
                let environments = self.api_client.list_environments(ws_id).await?;
                Ok(ReadCommandResult::ApiEnvironments(
                    ApiEnvironmentListResult {
                        count: environments.len(),
                        environments,
                        source: "command-bus".to_string(),
                    },
                ))
            }
            ReadCommand::ListActivity {
                workspace_id,
                limit,
            } => {
                let state = self.read_workspace_state().await?;
                let ws_id = workspace_id.unwrap_or(state.active_workspace_id);
                let limit = limit
                    .unwrap_or(DEFAULT_ACTIVITY_LIMIT)
                    .clamp(1, MAX_ACTIVITY_LIMIT);
                let entries = self.activity_log.list_recent(Some(&ws_id), limit).await?;
                let activity = entries
                    .into_iter()
                    .map(|entry| ActivityItem {
                        id: entry.id,
                        workspace_id: entry.workspace_id,
                        action: entry.action,
                        target: entry.target,
                        details: serde_json::from_str(&entry.details_json)
                            .unwrap_or(serde_json::Value::Null),
                        created_at: entry.created_at,
                    })
                    .collect::<Vec<_>>();
                Ok(ReadCommandResult::Activity(ActivityListResult {
                    count: activity.len(),
                    activity,
                    source: "command-bus".to_string(),
                }))
            }
        }
    }

    pub async fn create_workspace(&self, name: String) -> AppResult<Workspace> {
        self.create_workspace_with_options(name, None, None).await
    }

    pub async fn create_workspace_with_options(
        &self,
        name: String,
        environment_type: Option<String>,
        mcp_policy: Option<String>,
    ) -> AppResult<Workspace> {
        let workspace = self
            .workspace
            .create_with_options(name, environment_type, mcp_policy)
            .await?;
        self.activity_log
            .record(
                Some(&workspace.id),
                "workspace.create",
                Some(&workspace.id),
                serde_json::json!({
                    "name": workspace.name,
                    "environment_type": workspace.environment_type,
                    "mcp_policy": workspace.mcp_policy,
                }),
            )
            .await?;
        Ok(workspace)
    }

    pub async fn update_workspace_environment(
        &self,
        workspace_id: String,
        environment_type: String,
    ) -> AppResult<Workspace> {
        let workspace = self
            .workspace
            .update_environment(workspace_id, environment_type)
            .await?;
        self.activity_log
            .record(
                Some(&workspace.id),
                "workspace.environment.update",
                Some(&workspace.id),
                serde_json::json!({
                    "environment_type": workspace.environment_type,
                    "mcp_policy": workspace.mcp_policy,
                }),
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

    pub async fn api_environments_list(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<ApiEnvironment>> {
        self.api_client.list_environments(workspace_id).await
    }

    pub async fn api_environment_create(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiEnvironment> {
        let environment = self
            .api_client
            .create_environment(workspace_id.clone(), name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.environment.create",
                Some(&environment.id),
                serde_json::json!({ "name": environment.name }),
            )
            .await?;
        Ok(environment)
    }

    pub async fn api_environment_update(
        &self,
        workspace_id: String,
        environment_id: String,
        name: String,
        variables: Vec<KeyValue>,
    ) -> AppResult<ApiEnvironment> {
        let environment = self
            .api_client
            .update_environment(workspace_id.clone(), environment_id, name, variables)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.environment.update",
                Some(&environment.id),
                serde_json::json!({ "variableCount": environment.variables.len() }),
            )
            .await?;
        Ok(environment)
    }

    pub async fn api_environment_delete(
        &self,
        workspace_id: String,
        environment_id: String,
    ) -> AppResult<Vec<ApiEnvironment>> {
        let environments = self
            .api_client
            .delete_environment(workspace_id.clone(), environment_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.environment.delete",
                Some(&environment_id),
                serde_json::json!({ "softDelete": true }),
            )
            .await?;
        Ok(environments)
    }

    pub async fn api_environment_activate(
        &self,
        workspace_id: String,
        environment_id: Option<String>,
    ) -> AppResult<Vec<ApiEnvironment>> {
        self.api_client
            .activate_environment(workspace_id, environment_id)
            .await
    }

    pub async fn api_collection_list(&self, workspace_id: String) -> AppResult<Vec<ApiCollection>> {
        self.api_client.list_collections(workspace_id).await
    }

    pub async fn api_collection_create(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        let collection = self
            .api_client
            .create_collection(workspace_id.clone(), name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.create",
                Some(&collection.id),
                serde_json::json!({ "name": collection.name }),
            )
            .await?;
        Ok(collection)
    }

    pub async fn api_collection_rename(
        &self,
        workspace_id: String,
        collection_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        let collection = self
            .api_client
            .rename_collection(workspace_id.clone(), collection_id, name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.rename",
                Some(&collection.id),
                serde_json::json!({ "name": collection.name }),
            )
            .await?;
        Ok(collection)
    }

    pub async fn api_collection_delete(
        &self,
        workspace_id: String,
        collection_id: String,
    ) -> AppResult<Vec<ApiCollection>> {
        let collections = self
            .api_client
            .delete_collection(workspace_id.clone(), collection_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.delete",
                Some(&collection_id),
                serde_json::json!({ "softDelete": true, "cascade": true }),
            )
            .await?;
        Ok(collections)
    }

    pub async fn api_collection_folders_list(
        &self,
        workspace_id: String,
        collection_id: Option<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        self.api_client
            .list_collection_folders(workspace_id, collection_id)
            .await
    }

    pub async fn api_collection_folder_create(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        let folder = self
            .api_client
            .create_collection_folder(workspace_id.clone(), collection_id, parent_folder_id, name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.create",
                Some(&folder.id),
                serde_json::json!({ "collectionId": folder.collection_id, "parentFolderId": folder.parent_folder_id }),
            )
            .await?;
        Ok(folder)
    }

    pub async fn api_collection_folder_rename(
        &self,
        workspace_id: String,
        folder_id: String,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        let folder = self
            .api_client
            .rename_collection_folder(workspace_id.clone(), folder_id, name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.rename",
                Some(&folder.id),
                serde_json::json!({ "name": folder.name }),
            )
            .await?;
        Ok(folder)
    }

    pub async fn api_collection_folder_delete(
        &self,
        workspace_id: String,
        folder_id: String,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        let folders = self
            .api_client
            .delete_collection_folder(workspace_id.clone(), folder_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.delete",
                Some(&folder_id),
                serde_json::json!({ "softDelete": true, "recursive": true }),
            )
            .await?;
        Ok(folders)
    }

    pub async fn api_collection_folder_move(
        &self,
        workspace_id: String,
        folder_id: String,
        target_parent_folder_id: Option<String>,
    ) -> AppResult<ApiCollectionFolder> {
        let folder = self
            .api_client
            .move_collection_folder(workspace_id.clone(), folder_id, target_parent_folder_id)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.move",
                Some(&folder.id),
                serde_json::json!({ "parentFolderId": folder.parent_folder_id }),
            )
            .await?;
        Ok(folder)
    }

    pub async fn api_collection_folders_reorder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        folder_ids: Vec<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        let folders = self
            .api_client
            .reorder_collection_folders(
                workspace_id.clone(),
                collection_id.clone(),
                parent_folder_id,
                folder_ids,
            )
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.reorder",
                Some(&collection_id),
                serde_json::json!({ "folderCount": folders.len() }),
            )
            .await?;
        Ok(folders)
    }

    pub async fn api_request_move(
        &self,
        workspace_id: String,
        request_id: String,
        collection_id: Option<String>,
        parent_folder_id: Option<String>,
    ) -> AppResult<ApiSavedRequest> {
        let saved = self
            .api_client
            .move_request(
                workspace_id.clone(),
                request_id,
                collection_id,
                parent_folder_id,
            )
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.request.move",
                Some(&saved.id),
                serde_json::json!({ "collectionId": saved.collection_id, "parentFolderId": saved.parent_folder_id }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn api_requests_reorder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        request_ids: Vec<String>,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        let requests = self
            .api_client
            .reorder_requests(
                workspace_id.clone(),
                collection_id.clone(),
                parent_folder_id,
                request_ids,
            )
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.request.reorder",
                Some(&collection_id),
                serde_json::json!({ "requestCount": requests.len() }),
            )
            .await?;
        Ok(requests)
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
        let response = self.api_client.send(input.clone()).await?;
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

    pub async fn update_api_request(
        &self,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
    ) -> AppResult<ApiSavedRequest> {
        let saved = self
            .api_client
            .update_request(workspace_id.clone(), request_id, input)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.update_request",
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
        self.execute_saved_api_request_in_workspace(
            Some(state.active_workspace_id),
            request_id,
            timeout_ms_override,
        )
        .await
    }

    pub async fn execute_saved_api_request_in_workspace(
        &self,
        workspace_id: Option<String>,
        request_id: &str,
        timeout_ms_override: Option<u64>,
    ) -> AppResult<ApiResponse> {
        let saved = self.api_client.get_saved_request(request_id).await?;

        if workspace_id
            .as_deref()
            .is_some_and(|id| saved.workspace_id != id)
        {
            return Err(unfour_core::AppError::NotFound("api request".to_string()));
        }

        let headers: Vec<KeyValue> = serde_json::from_str(&saved.headers_json).unwrap_or_default();
        let query: Vec<KeyValue> = serde_json::from_str(&saved.query_json).unwrap_or_default();
        let timeout_ms = timeout_ms_override.map(|t| t.min(60_000));

        let input = ApiRequestInput {
            workspace_id: saved.workspace_id.clone(),
            name: Some(saved.name.clone()),
            parent_folder_id: saved.parent_folder_id.clone(),
            collection_id: Some(saved.collection_id.clone()),
            auth_json: Some(saved.auth_json.clone()),
            method: saved.method.clone(),
            url: saved.url.clone(),
            headers,
            query,
            body: saved.body.clone(),
            body_kind: saved.body_kind.clone(),
            timeout_ms,
        };

        self.api_client.send(input).await
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
        catalog: Option<String>,
    ) -> AppResult<DatabaseSchema> {
        self.database
            .schema(workspace_id, connection_id, catalog)
            .await
    }

    pub async fn database_catalogs(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<String>> {
        self.database
            .list_catalogs(workspace_id, connection_id)
            .await
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

    pub async fn database_table_structure(
        &self,
        input: DatabaseTableStructureInput,
    ) -> AppResult<DatabaseTableStructure> {
        self.database.table_structure(input).await
    }

    pub async fn mutate_database_row(
        &self,
        input: DatabaseRowMutationInput,
    ) -> AppResult<DatabaseRowMutationResult> {
        self.database.mutate_table_row(input).await
    }

    pub async fn record_database_query_history(
        &self,
        input: DbQueryHistoryRecordInput,
    ) -> AppResult<()> {
        self.database.record_query_history(input).await
    }

    pub async fn list_database_query_history(
        &self,
        workspace_id: String,
        limit: Option<i64>,
    ) -> AppResult<Vec<DbQueryHistoryEntry>> {
        self.database.list_query_history(workspace_id, limit).await
    }

    pub async fn clear_database_query_history(&self, workspace_id: String) -> AppResult<()> {
        self.database.clear_query_history(workspace_id).await
    }

    pub async fn list_saved_sql(&self, workspace_id: String) -> AppResult<Vec<SavedSql>> {
        self.database.list_saved_sql(workspace_id).await
    }

    pub async fn save_saved_sql(&self, input: SavedSqlInput) -> AppResult<SavedSql> {
        let saved = self.database.save_sql(input).await?;
        self.activity_log
            .record(
                Some(&saved.workspace_id),
                "database.saved_sql.save",
                Some(&saved.id),
                serde_json::json!({ "name": saved.name }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn delete_saved_sql(
        &self,
        workspace_id: String,
        id: String,
    ) -> AppResult<Vec<SavedSql>> {
        let remaining = self
            .database
            .delete_saved_sql(workspace_id.clone(), id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "database.saved_sql.delete",
                Some(&id),
                serde_json::json!({}),
            )
            .await?;
        Ok(remaining)
    }

    pub async fn list_ssh_connections(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<SshConnection>> {
        self.ssh.list_connections(workspace_id).await
    }

    /// Run a read-only SSH diagnostic command against a saved connection. The
    /// command is allowlist-validated and output line-redacted by the SSH
    /// engine; this records an activity event (command + exit status only, never
    /// the captured output).
    pub async fn run_ssh_diagnostic(
        &self,
        input: SshDiagnosticInput,
    ) -> AppResult<SshDiagnosticResult> {
        let workspace_id = input.workspace_id.clone();
        let result = self.ssh.run_diagnostic(input).await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "ssh.diagnostic",
                Some(&result.connection_id),
                serde_json::json!({
                    "command": result.command,
                    "exitStatus": result.exit_status,
                    "truncated": result.truncated,
                }),
            )
            .await?;
        Ok(result)
    }

    /// Run a one-shot SSH command through the SSH engine. Callers must perform
    /// environment policy and high-risk confirmation before invoking this
    /// method; this boundary records only a redacted/truncated command preview.
    pub async fn run_ssh_command(
        &self,
        input: SshDiagnosticInput,
    ) -> AppResult<SshDiagnosticResult> {
        let workspace_id = input.workspace_id.clone();
        let connection_id = input.connection_id.clone();
        let (command_kind, command_redacted) = command_activity_kind(&input.command);
        let result = self.ssh.run_command(input).await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "ssh.command",
                Some(&connection_id),
                serde_json::json!({
                    "commandKind": command_kind,
                    "commandRedacted": command_redacted,
                    "exitStatus": result.exit_status,
                    "truncated": result.truncated,
                }),
            )
            .await?;
        Ok(result)
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
        let workspace_id = input.workspace_id.clone();
        let host = input.host.clone();
        let port = input.port;
        let deleted = self.ssh.reset_host_fingerprint(input).await?;
        if deleted {
            self.activity_log
                .record(
                    Some(&workspace_id),
                    "ssh.host_key.reset",
                    Some(&format!("{}:{}", host, port)),
                    serde_json::json!({ "host": host, "port": port }),
                )
                .await?;
        }
        Ok(deleted)
    }

    pub async fn list_all_ssh_fingerprints(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<SshHostFingerprintInfo>> {
        self.ssh.list_all_host_fingerprints(workspace_id).await
    }

    pub async fn import_ssh_known_hosts(
        &self,
        input: SshKnownHostsImportInput,
    ) -> AppResult<SshKnownHostsImportResult> {
        let workspace_id = input.workspace_id.clone();
        let result = self.ssh.import_known_hosts(input).await?;
        self.activity_log
            .record(
                Some(&workspace_id),
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

    pub async fn export_ssh_known_hosts(
        &self,
        input: SshKnownHostsExportInput,
    ) -> AppResult<SshKnownHostsExportResult> {
        self.ssh.export_known_hosts(input).await
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
#[path = "lib_tests.rs"]
mod tests;
