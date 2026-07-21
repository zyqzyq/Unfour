use super::*;

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
            ssh: SshService::new(db, secret_store).with_task_log_dir(task_log_dir()?),
            workspace,
        })
    }

    /// Construct a `CommandBus` with an in-memory secret store for tests and
    /// local adapters that do not access credentials.
    pub async fn from_db(db: LocalDb) -> AppResult<Self> {
        let secret_store = SecretStore::in_memory("unfour-test");
        Self::from_db_with_secret_store(db, secret_store).await
    }

    /// Construct a `CommandBus` over an existing DB without seeding the
    /// default workspace, backed by the real OS keychain.
    ///
    /// Unlike [`from_db_with_secret_store`](Self::from_db_with_secret_store),
    /// this skips `ensure_default_workspace()` so satellite processes (e.g.
    /// the MCP server) can attach to an already-initialized storage without
    /// re-seeding. The returned bus retains **full write capabilities**; the
    /// `without_seeding` suffix refers only to construction-time seeding,
    /// not to runtime capability. Current callers only exercise reads and
    /// do not create/rotate/delete credentials by convention, not by type.
    pub async fn from_existing_db_without_seeding(db: LocalDb) -> AppResult<Self> {
        Self::from_db_without_workspace_seed(db, SecretStore::new(DEFAULT_SECRET_SERVICE)).await
    }

    pub(super) async fn from_db_without_workspace_seed(
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
            ssh: SshService::new(db, secret_store).with_task_log_dir(task_log_dir()?),
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

    #[cfg(feature = "ssh-native")]
    pub fn set_sftp_transfer_callback(&self, callback: unfour_ssh_engine::SftpTransferCallback) {
        self.ssh.set_sftp_transfer_callback(callback);
    }

    #[cfg(feature = "ssh-native")]
    pub fn set_task_run_callback(&self, callback: unfour_ssh_engine::TaskRunCallback) {
        self.ssh.set_task_run_callback(callback);
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

    pub(super) async fn read_workspace_state(&self) -> AppResult<WorkspaceState> {
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
}

fn task_log_dir() -> AppResult<std::path::PathBuf> {
    Ok(unfour_paths::resolve_unfour_paths()
        .map_err(|error| {
            unfour_core::AppError::Config(format!("failed to resolve SSH task log path: {error}"))
        })?
        .logs_dir
        .join("tasks"))
}
