use super::*;
use crate::transaction::CommandActivity;
use sqlx::SqliteConnection;
use unfour_core::domain::{
    connection_entity_key, validate_connection_domain_key, validate_external_connection_delete,
    CommandContext, DomainCommandResult, DomainEntityKey, DomainEntityType, DomainMutation,
    DomainSnapshot, ExternalApplyPage, ExternalApplyReport, ExternalConnectionApply,
    ExternalWorkspaceApply, ExternalWorkspaceEnvironmentApply,
    ExternalWorkspaceEnvironmentVariableApply, ExternalWorkspaceVariableApply,
    DATABASE_CONNECTION_TYPE, SSH_CONNECTION_TYPE,
};
use unfour_core::AppError;
use unfour_database_engine::{DatabaseConnectionCleanup, DatabaseService};
use unfour_http_engine::ApiClientService;
use unfour_ssh_engine::{SshConnectionCleanup, SshService};

pub(crate) struct FeatureCascadeOutcome {
    pub mutations: Vec<DomainMutation>,
    pub ssh_connection_cleanups: Vec<SshConnectionCleanup>,
    pub database_connection_cleanups: Vec<DatabaseConnectionCleanup>,
}

struct ExternalPageOutcome {
    report: ExternalApplyReport,
    ssh_connection_cleanups: Vec<SshConnectionCleanup>,
    database_connection_cleanups: Vec<DatabaseConnectionCleanup>,
}

/// Soft-delete leftover live feature entities owned by a workspace, children
/// first. Workspace-engine still tombstones environment variables, environments,
/// and workspace variables; this helper covers API, SSH Task, and Connection
/// aggregates so each feature crate does not orchestrate workspace delete on
/// its own.
pub(crate) async fn cascade_workspace_feature_entities_on(
    api_client: &ApiClientService,
    ssh: &SshService,
    database: &DatabaseService,
    connection: &mut SqliteConnection,
    context: &CommandContext,
    workspace_id: &str,
    deleted_at: Option<&str>,
) -> AppResult<FeatureCascadeOutcome> {
    let mut mutations = api_client
        .delete_workspace_api_entities_on(connection, context, workspace_id, deleted_at)
        .await?;
    mutations.extend(
        ssh.delete_workspace_ssh_task_entities_on(connection, context, workspace_id, deleted_at)
            .await?,
    );
    let resolved_deleted_at = deleted_at
        .map(str::to_string)
        .unwrap_or_else(unfour_workspace_engine::WorkspaceService::rfc3339_now);
    let (connection_mutations, ssh_connection_cleanups) = ssh
        .delete_workspace_connections_on(connection, context, workspace_id, &resolved_deleted_at)
        .await?;
    mutations.extend(connection_mutations);
    let (connection_mutations, database_connection_cleanups) = database
        .delete_workspace_connections_on(connection, context, workspace_id, &resolved_deleted_at)
        .await?;
    mutations.extend(connection_mutations);
    Ok(FeatureCascadeOutcome {
        mutations,
        ssh_connection_cleanups,
        database_connection_cleanups,
    })
}

impl CommandBus {
    pub async fn read_domain_snapshot(&self, key: &DomainEntityKey) -> AppResult<DomainSnapshot> {
        match key.entity_type {
            DomainEntityType::Connection => match self.connection_type_for_key(key).await?.as_str()
            {
                SSH_CONNECTION_TYPE => self.ssh.read_connection_domain_snapshot(key).await,
                DATABASE_CONNECTION_TYPE => {
                    self.database.read_connection_domain_snapshot(key).await
                }
                connection_type => Err(AppError::Config(format!(
                    "unsupported stored connection type: {connection_type}"
                ))),
            },
            DomainEntityType::ApiCollection
            | DomainEntityType::ApiFolder
            | DomainEntityType::ApiRequest => self.api_client.read_domain_snapshot(key).await,
            DomainEntityType::SshTask | DomainEntityType::SshTaskStep => {
                self.ssh.read_task_domain_snapshot(key).await
            }
            _ => self.workspace.read_snapshot(key).await,
        }
    }

    pub async fn list_connection_domain_entities(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<DomainEntityKey>> {
        let workspace_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?1 AND deleted_at IS NULL)",
        )
        .bind(&workspace_id)
        .fetch_one(self.db.pool())
        .await?;
        if !workspace_exists {
            return Err(AppError::NotFound("workspace".to_string()));
        }
        let ids: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT id FROM connections
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY id
            "#,
        )
        .bind(&workspace_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(ids
            .into_iter()
            .map(|id| connection_entity_key(&workspace_id, id))
            .collect())
    }

    pub async fn list_ssh_task_domain_entities(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<DomainEntityKey>> {
        self.ssh.list_task_domain_entities(workspace_id).await
    }

    pub async fn apply_external_page(
        &self,
        page: ExternalApplyPage,
    ) -> AppResult<ExternalApplyReport> {
        let counts = serde_json::json!({
            "workspaceCount": page.workspaces.len(),
            "connectionCount": page.connections.len(),
            "workspaceVariableCount": page.workspace_variables.len(),
            "workspaceEnvironmentCount": page.workspace_environments.len(),
            "workspaceEnvironmentVariableCount": page.workspace_environment_variables.len(),
            "apiCollectionCount": page.api_collections.len(),
            "apiFolderCount": page.api_folders.len(),
            "apiRequestCount": page.api_requests.len(),
            "sshTaskCount": page.ssh_tasks.len(),
            "sshTaskStepCount": page.ssh_task_steps.len(),
        });
        let context = CommandContext::external("workspace.external.apply_page");
        let executor_context = context.clone();
        let workspace = self.workspace.clone();
        let api_client = self.api_client.clone();
        let ssh = self.ssh.clone();
        let database = self.database.clone();
        let cleanup_ssh = ssh.clone();
        let cleanup_database = database.clone();
        let outcome = self
            .execute_domain_command(
                context,
                Some(CommandActivity {
                    workspace_id: None,
                    action: "workspace.external.apply_page",
                    target: None,
                    details: counts,
                }),
                move |connection| {
                    Box::pin(async move {
                        let workspace_deletes: Vec<(String, String)> = page
                            .workspaces
                            .iter()
                            .filter_map(|change| match change {
                                ExternalWorkspaceApply::Delete(delete) => Some((
                                    delete.entity.workspace_id.clone(),
                                    delete.deleted_at.clone(),
                                )),
                                _ => None,
                            })
                            .collect();
                        let api_page = page.clone();
                        let ssh_page = page.clone();
                        let connection_changes = page.connections.clone();
                        let mut mutations = Vec::new();
                        let mut ssh_connection_cleanups = Vec::new();
                        let mut database_connection_cleanups = Vec::new();
                        for (workspace_id, deleted_at) in &workspace_deletes {
                            let cascade = cascade_workspace_feature_entities_on(
                                &api_client,
                                &ssh,
                                &database,
                                connection,
                                &executor_context,
                                workspace_id,
                                Some(deleted_at),
                            )
                            .await?;
                            mutations.extend(cascade.mutations);
                            ssh_connection_cleanups.extend(cascade.ssh_connection_cleanups);
                            database_connection_cleanups
                                .extend(cascade.database_connection_cleanups);
                        }
                        let workspace_outcome = workspace
                            .apply_external_page_on(connection, &executor_context, page)
                            .await?;
                        mutations.extend(workspace_outcome.mutations);
                        let connection_outcome = apply_external_connection_changes_on(
                            &ssh,
                            &database,
                            connection,
                            &executor_context,
                            connection_changes,
                        )
                        .await?;
                        mutations.extend(connection_outcome.mutations);
                        ssh_connection_cleanups.extend(connection_outcome.ssh_connection_cleanups);
                        database_connection_cleanups
                            .extend(connection_outcome.database_connection_cleanups);
                        let api_outcome = api_client
                            .apply_external_page_on(connection, &executor_context, api_page)
                            .await?;
                        let ssh_outcome = ssh
                            .apply_external_task_page_on(connection, &executor_context, ssh_page)
                            .await?;
                        mutations.extend(api_outcome.mutations);
                        mutations.extend(ssh_outcome.mutations);
                        let mut secret_material_outcomes =
                            workspace_outcome.value.secret_material_outcomes;
                        secret_material_outcomes.extend(api_outcome.value.secret_material_outcomes);
                        secret_material_outcomes.extend(ssh_outcome.value.secret_material_outcomes);
                        let report = ExternalApplyReport {
                            applied_count: mutations.len(),
                            mutations: mutations.clone(),
                            secret_material_outcomes,
                        };
                        Ok(DomainCommandResult::new(
                            ExternalPageOutcome {
                                report,
                                ssh_connection_cleanups,
                                database_connection_cleanups,
                            },
                            mutations,
                        ))
                    })
                },
            )
            .await?;
        cleanup_ssh
            .cleanup_connection_changes(outcome.ssh_connection_cleanups)
            .await;
        cleanup_database
            .cleanup_connection_changes(outcome.database_connection_cleanups)
            .await;
        Ok(outcome.report)
    }

    pub async fn apply_external_workspaces(
        &self,
        changes: Vec<ExternalWorkspaceApply>,
    ) -> AppResult<ExternalApplyReport> {
        self.apply_external_page(ExternalApplyPage {
            workspaces: changes,
            ..ExternalApplyPage::default()
        })
        .await
    }

    pub async fn apply_external_connections(
        &self,
        changes: Vec<ExternalConnectionApply>,
    ) -> AppResult<ExternalApplyReport> {
        self.apply_external_page(ExternalApplyPage {
            connections: changes,
            ..ExternalApplyPage::default()
        })
        .await
    }

    pub async fn apply_external_workspace_variables(
        &self,
        changes: Vec<ExternalWorkspaceVariableApply>,
    ) -> AppResult<ExternalApplyReport> {
        self.apply_external_page(ExternalApplyPage {
            workspace_variables: changes,
            ..ExternalApplyPage::default()
        })
        .await
    }

    pub async fn apply_external_workspace_environments(
        &self,
        changes: Vec<ExternalWorkspaceEnvironmentApply>,
    ) -> AppResult<ExternalApplyReport> {
        self.apply_external_page(ExternalApplyPage {
            workspace_environments: changes,
            ..ExternalApplyPage::default()
        })
        .await
    }

    pub async fn apply_external_workspace_environment_variables(
        &self,
        changes: Vec<ExternalWorkspaceEnvironmentVariableApply>,
    ) -> AppResult<ExternalApplyReport> {
        self.apply_external_page(ExternalApplyPage {
            workspace_environment_variables: changes,
            ..ExternalApplyPage::default()
        })
        .await
    }

    async fn connection_type_for_key(&self, key: &DomainEntityKey) -> AppResult<String> {
        validate_connection_domain_key(key)?;
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT workspace_id, connection_type FROM connections WHERE id = ?1")
                .bind(&key.entity_id)
                .fetch_optional(self.db.pool())
                .await?;
        let (workspace_id, connection_type) =
            row.ok_or_else(|| AppError::NotFound("connection".to_string()))?;
        if workspace_id != key.workspace_id {
            return Err(AppError::Validation(
                "connection domain key workspace ownership mismatch".to_string(),
            ));
        }
        Ok(connection_type)
    }
}

async fn apply_external_connection_changes_on(
    ssh: &SshService,
    database: &DatabaseService,
    connection: &mut SqliteConnection,
    context: &CommandContext,
    changes: Vec<ExternalConnectionApply>,
) -> AppResult<FeatureCascadeOutcome> {
    let mut mutations = Vec::new();
    let mut ssh_connection_cleanups = Vec::new();
    let mut database_connection_cleanups = Vec::new();
    for change in changes {
        let connection_type = match &change {
            ExternalConnectionApply::Upsert(record) => Some(record.connection_type.as_str()),
            ExternalConnectionApply::Delete(delete) => {
                validate_external_connection_delete(delete)?;
                let row: Option<(String, String)> = sqlx::query_as(
                    "SELECT workspace_id, connection_type FROM connections WHERE id = ?1",
                )
                .bind(&delete.entity.entity_id)
                .fetch_optional(&mut *connection)
                .await?;
                match row {
                    Some((workspace_id, connection_type)) => {
                        if workspace_id != delete.entity.workspace_id {
                            return Err(AppError::Validation(
                                "external connection workspace ownership mismatch".to_string(),
                            ));
                        }
                        Some(if connection_type == SSH_CONNECTION_TYPE {
                            SSH_CONNECTION_TYPE
                        } else if connection_type == DATABASE_CONNECTION_TYPE {
                            DATABASE_CONNECTION_TYPE
                        } else {
                            return Err(AppError::Config(format!(
                                "unsupported stored connection type: {connection_type}"
                            )));
                        })
                    }
                    None => None,
                }
            }
        };
        let Some(connection_type) = connection_type else {
            continue;
        };
        match connection_type {
            SSH_CONNECTION_TYPE => {
                let outcome = ssh
                    .apply_external_connection_on(connection, context, change)
                    .await?;
                mutations.extend(outcome.mutations);
                if let Some(cleanup) = outcome.value {
                    ssh_connection_cleanups.push(cleanup);
                }
            }
            DATABASE_CONNECTION_TYPE => {
                let outcome = database
                    .apply_external_connection_on(connection, context, change)
                    .await?;
                mutations.extend(outcome.mutations);
                if let Some(cleanup) = outcome.value {
                    database_connection_cleanups.push(cleanup);
                }
            }
            unsupported => {
                return Err(AppError::Validation(format!(
                    "unsupported external connection type: {unsupported}"
                )));
            }
        }
    }
    Ok(FeatureCascadeOutcome {
        mutations,
        ssh_connection_cleanups,
        database_connection_cleanups,
    })
}
