use super::{get_workspace_on, WorkspaceService};
use sqlx::SqliteConnection;
use unfour_core::models::Workspace;
use unfour_core::{AppError, AppResult};

impl WorkspaceService {
    pub async fn assert_creation_source_on(
        &self,
        connection: &mut SqliteConnection,
        source_id: &str,
        environment_type: &str,
        mcp_policy: &str,
    ) -> AppResult<()> {
        let source = get_workspace_on(connection, source_id, false).await?;
        let active = super::read_setting_on(connection, "active_workspace_id").await?;
        if active.as_deref() != Some(source_id)
            || source.environment_type != environment_type
            || source.mcp_policy != mcp_policy
        {
            return Err(AppError::RowConflict(
                "workspace creation context changed".into(),
            ));
        }
        Ok(())
    }
    pub async fn assert_name_expected_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
        expected: &str,
    ) -> AppResult<()> {
        let current = get_workspace_on(connection, workspace_id, false).await?;
        if current.name != expected {
            return Err(AppError::RowConflict("workspace name changed".into()));
        }
        Ok(())
    }

    pub async fn assert_security_expected_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
        expected: &Workspace,
    ) -> AppResult<()> {
        let current = get_workspace_on(connection, workspace_id, false).await?;
        if current.id != expected.id
            || current.environment_type != expected.environment_type
            || current.mcp_policy != expected.mcp_policy
        {
            return Err(AppError::RowConflict(
                "workspace security settings changed".into(),
            ));
        }
        Ok(())
    }
}
