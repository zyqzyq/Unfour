use sqlx::SqliteConnection;
use unfour_core::models::{WorkspaceEnvironment, WorkspaceVariable};
use unfour_core::{AppError, AppResult};

use super::variables::{get_environment_on, list_variables_on};
use super::WorkspaceService;

impl WorkspaceService {
    // The CommandBus calls these on the same transaction connection as the write.
    pub async fn assert_variables_expected_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
        expected: &[WorkspaceVariable],
    ) -> AppResult<()> {
        let current = list_variables_on(connection, workspace_id, false).await?;
        if current != expected {
            return Err(AppError::RowConflict("workspace variables changed".into()));
        }
        Ok(())
    }

    pub async fn assert_environment_expected_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
        environment_id: &str,
        expected: &WorkspaceEnvironment,
    ) -> AppResult<()> {
        let mut current =
            get_environment_on(connection, workspace_id, environment_id, false).await?;
        // Selection changes do not modify editable content.
        current.is_active = expected.is_active;
        if &current != expected {
            return Err(AppError::RowConflict(
                "workspace environment changed".into(),
            ));
        }
        Ok(())
    }
}
