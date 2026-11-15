//! Legacy API environment compatibility and metadata-preserving updates.

use crate::CommandBus;
use unfour_core::models::{ApiEnvironment, KeyValue, WorkspaceEnvironment, WorkspaceVariableInput};
use unfour_core::AppResult;

impl CommandBus {
    pub async fn api_environments_list(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<ApiEnvironment>> {
        Ok(self
            .workspace_environments_list(workspace_id)
            .await?
            .into_iter()
            .map(crate::workspace_variable_commands::legacy_api_environment)
            .collect())
    }

    pub async fn api_environment_create(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiEnvironment> {
        Ok(crate::workspace_variable_commands::legacy_api_environment(
            self.workspace_environment_create(workspace_id, name)
                .await?,
        ))
    }

    pub async fn api_environment_update(
        &self,
        workspace_id: String,
        environment_id: String,
        name: String,
        variables: Vec<KeyValue>,
    ) -> AppResult<ApiEnvironment> {
        Ok(crate::workspace_variable_commands::legacy_api_environment(
            self.api_environment_update_with_metadata(
                workspace_id,
                environment_id,
                name,
                variables,
            )
            .await?,
        ))
    }

    /// Preserve variable classification until an adapter has applied its output policy.
    pub async fn api_environment_update_with_metadata(
        &self,
        workspace_id: String,
        environment_id: String,
        name: String,
        variables: Vec<KeyValue>,
    ) -> AppResult<WorkspaceEnvironment> {
        let existing = self
            .workspace
            .list_environments(workspace_id.clone())
            .await?
            .into_iter()
            .find(|environment| environment.id == environment_id)
            .ok_or_else(|| unfour_core::AppError::NotFound("workspace environment".to_string()))?;
        let variables = variables
            .into_iter()
            .enumerate()
            .map(|(index, variable)| {
                let metadata = existing
                    .variables
                    .iter()
                    .find(|current| current.key.eq_ignore_ascii_case(variable.key.trim()));
                WorkspaceVariableInput {
                    id: metadata.map(|current| current.id.clone()),
                    key: variable.key,
                    value: variable.value,
                    is_secret: metadata.is_some_and(|current| current.is_secret),
                    is_enabled: variable.enabled,
                    description: metadata.and_then(|current| current.description.clone()),
                    sort_order: i64::try_from(index).unwrap_or(i64::MAX),
                }
            })
            .collect();
        // Classification came from this snapshot; reject concurrent edits instead
        // of overwriting a newly marked secret with stale legacy metadata.
        self.workspace_environment_update_checked(
            workspace_id,
            environment_id,
            name,
            variables,
            Some(existing),
        )
        .await
    }

    pub async fn api_environment_delete(
        &self,
        workspace_id: String,
        environment_id: String,
    ) -> AppResult<Vec<ApiEnvironment>> {
        let environments = self
            .workspace_environment_delete(workspace_id, environment_id)
            .await?
            .into_iter()
            .map(crate::workspace_variable_commands::legacy_api_environment)
            .collect();
        Ok(environments)
    }

    pub async fn api_environment_activate(
        &self,
        workspace_id: String,
        environment_id: Option<String>,
    ) -> AppResult<Vec<ApiEnvironment>> {
        Ok(self
            .workspace_environment_set_active(workspace_id, environment_id)
            .await?
            .into_iter()
            .map(crate::workspace_variable_commands::legacy_api_environment)
            .collect())
    }
}
