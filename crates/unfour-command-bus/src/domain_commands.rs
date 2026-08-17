use super::*;
use crate::transaction::CommandActivity;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityKey, DomainEntityType, DomainSnapshot,
    ExternalApplyPage, ExternalApplyReport, ExternalWorkspaceApply,
    ExternalWorkspaceEnvironmentApply, ExternalWorkspaceEnvironmentVariableApply,
    ExternalWorkspaceVariableApply,
};

impl CommandBus {
    pub async fn read_domain_snapshot(&self, key: &DomainEntityKey) -> AppResult<DomainSnapshot> {
        match key.entity_type {
            DomainEntityType::ApiCollection
            | DomainEntityType::ApiFolder
            | DomainEntityType::ApiRequest => self.api_client.read_domain_snapshot(key).await,
            DomainEntityType::SshTask | DomainEntityType::SshTaskStep => {
                self.ssh.read_task_domain_snapshot(key).await
            }
            _ => self.workspace.read_snapshot(key).await,
        }
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
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: None,
                action: "workspace.external.apply_page",
                target: None,
                details: counts,
            }),
            move |connection| {
                Box::pin(async move {
                    let api_page = page.clone();
                    let ssh_page = page.clone();
                    let workspace_outcome = workspace
                        .apply_external_page_on(connection, &executor_context, page)
                        .await?;
                    let api_outcome = api_client
                        .apply_external_page_on(connection, &executor_context, api_page)
                        .await?;
                    let ssh_outcome = ssh
                        .apply_external_task_page_on(connection, &executor_context, ssh_page)
                        .await?;
                    let mut mutations = workspace_outcome.mutations;
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
                    Ok(DomainCommandResult::new(report, mutations))
                })
            },
        )
        .await
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
}
