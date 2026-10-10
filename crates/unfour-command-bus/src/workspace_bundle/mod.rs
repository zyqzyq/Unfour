mod backup;
mod filename;
mod local;
mod remap;
mod safety;
mod schema;
mod templates;
pub use local::WorkspaceBundleOptions;
#[cfg(test)]
mod backup_tests;
#[cfg(test)]
mod connection_credential_tests;
#[cfg(test)]
mod credential_tests;
#[cfg(test)]
mod mcp_policy_tests;
#[cfg(test)]
mod tests;
use crate::CommandBus;
use schema::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use unfour_core::{domain::*, models::*, AppError, AppResult};
use unfour_flow_engine::FlowService;

pub const MAX_BUNDLE_BYTES: usize = 32 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct WorkspaceBundleExportArtifact {
    pub content: String,
    pub suggested_file_name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBundleIssue {
    pub entity_id: String,
    pub name: String,
    pub code: String,
    pub field: String,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBundlePreview {
    pub name: String,
    pub counts: BTreeMap<String, usize>,
    pub reconfigure: Vec<WorkspaceBundleIssue>,
    pub paths: Vec<LocalPath>,
}
fn invalid() -> AppError {
    AppError::Validation("WORKSPACE_BUNDLE_INVALID".into())
}
// Only business fields cross the versioned boundary. These omissions are not part of V1.
fn portable<T: serde::de::DeserializeOwned>(source: impl Serialize) -> AppResult<T> {
    let mut value = serde_json::to_value(source)?;
    let map = value.as_object_mut().ok_or_else(invalid)?;
    for field in [
        "workspaceId",
        "createdAt",
        "updatedAt",
        "revision",
        "mcpPolicy",
        "deletedAt",
        "syncStatus",
        "remoteId",
    ] {
        map.remove(field);
    }
    Ok(serde_json::from_value(value)?)
}
fn parse(content: &str) -> AppResult<WorkspaceBundle> {
    if content.len() > MAX_BUNDLE_BYTES {
        return Err(AppError::Validation("WORKSPACE_BUNDLE_TOO_LARGE".into()));
    }
    let mut bundle: WorkspaceBundle = serde_json::from_str(content).map_err(|_| invalid())?;
    if bundle.format != "unfour-workspace" || !matches!(bundle.version, 1 | 2) {
        return Err(AppError::Validation(
            "WORKSPACE_BUNDLE_UNSUPPORTED_VERSION".into(),
        ));
    }
    safety::sanitize(&mut bundle)?;
    local::validate_paths(&bundle)?;
    Ok(bundle)
}
impl CommandBus {
    pub async fn workspace_bundle_export(&self, workspace: String) -> AppResult<String> {
        Ok(self
            .workspace_bundle_export_artifact(workspace)
            .await?
            .content)
    }
    pub async fn workspace_bundle_export_artifact(
        &self,
        workspace: String,
    ) -> AppResult<WorkspaceBundleExportArtifact> {
        let mut tx = self.db.pool().begin().await?;
        let bundle = self
            .workspace_bundle_snapshot_on(&mut tx, &workspace)
            .await?;
        tx.rollback().await?;
        let content = serde_json::to_string_pretty(&bundle)?;
        if content.len() > MAX_BUNDLE_BYTES {
            return Err(AppError::Validation("WORKSPACE_BUNDLE_TOO_LARGE".into()));
        }
        Ok(WorkspaceBundleExportArtifact {
            content,
            suggested_file_name: filename::suggested_file_name(&bundle.workspace.name),
        })
    }
    /// All business definitions and supplements must use the caller's snapshot.
    async fn workspace_bundle_snapshot_on(
        &self,
        db: &mut sqlx::SqliteConnection,
        workspace: &str,
    ) -> AppResult<WorkspaceBundle> {
        let mut snapshots = self
            .workspace
            .export_workspace_snapshots_on(&mut *db, &workspace)
            .await?;
        snapshots.extend(
            self.api_client
                .export_workspace_snapshots_on(&mut *db, &workspace)
                .await?,
        );
        snapshots.extend(
            self.ssh
                .export_connection_snapshots_on(&mut *db, &workspace)
                .await?,
        );
        snapshots.extend(
            self.database
                .export_connection_snapshots_on(&mut *db, &workspace)
                .await?,
        );
        for key in self
            .ssh
            .list_task_domain_entities_on(&mut *db, &workspace)
            .await?
        {
            snapshots.push(
                self.ssh
                    .read_task_domain_snapshot_on(&mut *db, &key)
                    .await?,
            );
        }
        let mut bundle = WorkspaceBundle {
            format: "unfour-workspace".into(),
            version: 1,
            workspace: WorkspaceRecord {
                id: workspace.into(),
                name: String::new(),
                environment_type: "dev".into(),
            },
            variables: vec![],
            environments: vec![],
            environment_variables: vec![],
            collections: vec![],
            folders: vec![],
            requests: vec![],
            connections: vec![],
            ssh_tasks: vec![],
            ssh_steps: vec![],
            saved_sql: vec![],
            flows: vec![],
            local_paths: vec![],
            api_templates: vec![],
            credential_requirements: vec![],
        };
        for snapshot in snapshots {
            match snapshot {
                DomainSnapshot::Workspace(record) => bundle.workspace = portable(record)?,
                DomainSnapshot::WorkspaceVariable(record) => {
                    bundle.variables.push(portable(record)?)
                }
                DomainSnapshot::WorkspaceEnvironment(record) => {
                    bundle.environments.push(portable(record)?)
                }
                DomainSnapshot::WorkspaceEnvironmentVariable(record) => {
                    bundle.environment_variables.push(portable(record)?)
                }
                DomainSnapshot::ApiCollection(record) => bundle.collections.push(portable(record)?),
                DomainSnapshot::ApiFolder(record) => bundle.folders.push(portable(record)?),
                DomainSnapshot::ApiRequest(record) => bundle.requests.push(portable(record)?),
                DomainSnapshot::Connection(record) => bundle.connections.push(portable(record)?),
                DomainSnapshot::SshTask(record) => bundle.ssh_tasks.push(portable(record)?),
                DomainSnapshot::SshTaskStep(record) => bundle.ssh_steps.push(portable(record)?),

                DomainSnapshot::Tombstone(_) => return Err(invalid()),
            }
        }
        for record in self
            .database
            .export_saved_sql_on(&mut *db, &bundle.workspace.id)
            .await?
        {
            bundle.saved_sql.push(portable(record)?);
        }
        for record in FlowService::new(self.db.clone())
            .export_definitions_on(&mut *db, &bundle.workspace.id)
            .await?
        {
            bundle.flows.push(portable(record)?);
        }
        safety::sanitize(&mut bundle)?;
        Ok(bundle)
    }
    pub async fn workspace_bundle_preview(
        &self,
        content: &str,
    ) -> AppResult<WorkspaceBundlePreview> {
        let bundle = parse(content)?;
        let mut preview = safety::preview(&bundle);
        let plan = remap::prepare(bundle, None)?;
        // Exercise every domain validator, including cross-record constraints, without committing.
        let mut tx = self.db.pool().begin().await?;
        preview.name = self
            .workspace
            .bundle_name_on(&mut tx, &preview.name)
            .await?;
        self.import_bundle_on(&mut tx, plan).await?;
        tx.rollback().await?;
        Ok(preview)
    }
    pub async fn workspace_bundle_import(
        &self,
        content: String,
        name: String,
    ) -> AppResult<Workspace> {
        let mut bundle = parse(&content)?;
        bundle.workspace.name = name;
        let plan = remap::prepare(bundle, None)?;
        let workspace_id = plan.workspace_id.clone();
        let bus = self.clone();
        self.execute_domain_command(
            CommandContext::local("workspace.bundle.import"),
            None,
            move |connection| {
                Box::pin(async move {
                    let mutations = bus.import_bundle_on(connection, plan).await?;
                    let workspace = bus
                        .workspace
                        .read_workspace_on(connection, &workspace_id)
                        .await?;
                    Ok(DomainCommandResult::new(workspace, mutations))
                })
            },
        )
        .await
    }
    async fn import_bundle_on(
        &self,
        connection: &mut sqlx::SqliteConnection,
        mut plan: remap::ImportPlan,
    ) -> AppResult<Vec<DomainMutation>> {
        // Reuse external materializers solely for safe fresh inserts (all IDs are new).
        // The enclosing operation is local; hooks receive local mutations atomically.
        let ExternalWorkspaceApply::Upsert(workspace) = &mut plan.page.workspaces[0] else {
            return Err(invalid());
        };
        workspace.name = self
            .workspace
            .bundle_name_on(connection, &workspace.name)
            .await?;
        let context = CommandContext::external("workspace.bundle.materialize");
        let mut mutations = self
            .workspace
            .apply_external_page_on(connection, &context, plan.page.clone())
            .await?
            .mutations;
        for change in &plan.page.connections {
            let ExternalConnectionApply::Upsert(record) = change else {
                return Err(invalid());
            };
            let result = if record.connection_type == "ssh" {
                self.ssh
                    .apply_external_connection_on(connection, &context, change.clone())
                    .await?
                    .mutations
            } else {
                self.database
                    .apply_external_connection_on(connection, &context, change.clone())
                    .await?
                    .mutations
            };
            mutations.extend(result);
        }
        mutations.extend(
            self.api_client
                .apply_external_page_on(connection, &context, plan.page.clone())
                .await?
                .mutations,
        );
        mutations.extend(
            self.ssh
                .apply_external_task_page_on(connection, &context, plan.page.clone())
                .await?
                .mutations,
        );
        self.restore_local_fields_on(connection, &plan).await?;
        for record in plan.sql {
            self.database
                .import_saved_sql_on(connection, record.1, &record.0)
                .await?;
        }
        let flow = FlowService::new(self.db.clone());
        for definition in plan.flows {
            flow.import_definition_on(connection, definition).await?;
        }
        for mutation in &mut mutations {
            mutation.origin = MutationOrigin::Local;
        }
        Ok(mutations)
    }
}
