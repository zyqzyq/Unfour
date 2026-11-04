use sqlx::SqliteConnection;
use unfour_core::domain::{CommandContext, DomainCommandResult};
use unfour_core::models::{ApiCollection, ApiCollectionFolder, ApiRequestInput, ApiSavedRequest};
use unfour_core::{AppError, AppResult};

use super::{collection_on, folder_on, request_on, ApiClientService};

fn stale() -> AppError {
    AppError::RowConflict("This item changed outside this editor. Reload it before saving.".into())
}

impl ApiClientService {
    /// The expected version is checked inside the caller's write transaction.
    /// SQLite serializes writers; a competing commit cannot pass this check and
    /// then be overwritten by the update in the same transaction.
    pub async fn update_request_if_current_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
        expected_revision: Option<i64>,
    ) -> AppResult<DomainCommandResult<ApiSavedRequest>> {
        if let Some(expected) = expected_revision {
            let current = request_on(connection, &workspace_id, &request_id, false).await?;
            if current.revision != expected {
                return Err(stale());
            }
        }
        self.update_request_on(connection, context, workspace_id, request_id, input)
            .await
    }

    pub async fn rename_collection_if_current_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        collection_id: String,
        name: String,
        expected_updated_at: Option<String>,
    ) -> AppResult<DomainCommandResult<ApiCollection>> {
        if let Some(expected) = expected_updated_at {
            let current = collection_on(connection, &workspace_id, &collection_id, false).await?;
            if current.updated_at != expected {
                return Err(stale());
            }
        }
        self.rename_collection_on(connection, context, workspace_id, collection_id, name)
            .await
    }

    pub async fn rename_collection_folder_if_current_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        folder_id: String,
        name: String,
        expected_updated_at: Option<String>,
    ) -> AppResult<DomainCommandResult<ApiCollectionFolder>> {
        if let Some(expected) = expected_updated_at {
            let current = folder_on(connection, &workspace_id, &folder_id, false).await?;
            if current.updated_at != expected {
                return Err(stale());
            }
        }
        self.rename_collection_folder_on(connection, context, workspace_id, folder_id, name)
            .await
    }
}
