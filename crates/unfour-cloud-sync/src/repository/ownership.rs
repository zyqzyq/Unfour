//! Durable local-workspace ownership resolution and binding creation fences.
//! Ownership is separate from runtime account activity and never transfers
//! implicitly during sign-in, sign-out, pause, or worker scheduling.

use sqlx::SqliteConnection;

use super::SyncRepository;
use crate::{SyncBinding, SyncError, SyncWorkspaceOwner};

impl SyncRepository {
    /// Resolve the one Cloud Sync owner for a local workspace. The explicit
    /// ownership row is the authoritative runtime source; bindings without
    /// that metadata are invariant violations rather than an implicit owner.
    pub async fn resolve_cloud_sync_owner(
        &self,
        workspace_id: &str,
    ) -> Result<Option<SyncWorkspaceOwner>, SyncError> {
        let mut connection = self.pool.acquire().await?;
        Self::resolve_cloud_sync_owner_on(&mut connection, workspace_id).await
    }

    pub(crate) async fn resolve_cloud_sync_owner_on(
        connection: &mut SqliteConnection,
        workspace_id: &str,
    ) -> Result<Option<SyncWorkspaceOwner>, SyncError> {
        if let Some((account_id, cloud_workspace_id)) = sqlx::query_as::<_, (String, String)>(
            "SELECT account_id, cloud_workspace_id FROM cloud_sync_workspace_ownership WHERE local_workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *connection)
        .await?
        {
            let binding_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND local_workspace_id = ?2 AND cloud_workspace_id = ?3)",
            )
            .bind(&account_id)
            .bind(workspace_id)
            .bind(&cloud_workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            if !binding_exists {
                return Err(SyncError::WorkspaceOwnershipInvariant);
            }
            return Ok(Some(SyncWorkspaceOwner {
                account_id,
                cloud_workspace_id,
            }));
        }

        let bindings: Vec<(String, String)> = sqlx::query_as(
            "SELECT account_id, cloud_workspace_id FROM cloud_sync_workspace_bindings WHERE local_workspace_id = ?1 ORDER BY account_id, cloud_workspace_id",
        )
        .bind(workspace_id)
        .fetch_all(&mut *connection)
        .await?;
        match bindings.as_slice() {
            [] => Ok(None),
            [_] => Err(SyncError::WorkspaceOwnershipInvariant),
            _ => Err(SyncError::WorkspaceOwnershipAmbiguous),
        }
    }

    pub(crate) async fn ensure_new_binding_owner_available_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        workspace_id: &str,
    ) -> Result<(), SyncError> {
        match Self::resolve_cloud_sync_owner_on(connection, workspace_id).await? {
            None => Ok(()),
            Some(owner) if owner.account_id == account_id => {
                Err(SyncError::WorkspaceOwnershipInvariant)
            }
            Some(_) => Err(SyncError::WorkspaceOwnedByAnotherAccount),
        }
    }

    pub(crate) async fn insert_workspace_owner_on(
        connection: &mut SqliteConnection,
        account_id: &str,
        workspace_id: &str,
        cloud_workspace_id: &str,
        now: &str,
    ) -> Result<(), SyncError> {
        let changed = sqlx::query(
            r#"INSERT INTO cloud_sync_workspace_ownership (
                 local_workspace_id, account_id, cloud_workspace_id, created_at, updated_at
               ) VALUES (?1, ?2, ?3, ?4, ?4)
               ON CONFLICT(local_workspace_id) DO NOTHING"#,
        )
        .bind(workspace_id)
        .bind(account_id)
        .bind(cloud_workspace_id)
        .bind(now)
        .execute(&mut *connection)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(SyncError::WorkspaceOwnedByAnotherAccount);
        }
        Ok(())
    }

    pub(crate) async fn assert_workspace_owner_on(
        connection: &mut SqliteConnection,
        binding: &SyncBinding,
    ) -> Result<(), SyncError> {
        match Self::resolve_cloud_sync_owner_on(connection, &binding.local_workspace_id).await? {
            Some(owner)
                if owner.account_id == binding.account_id
                    && owner.cloud_workspace_id == binding.cloud_workspace_id =>
            {
                Ok(())
            }
            Some(owner) if owner.account_id != binding.account_id => {
                Err(SyncError::WorkspaceOwnedByAnotherAccount)
            }
            Some(_) => Err(SyncError::WorkspaceOwnershipInvariant),
            None => Err(SyncError::NotFound),
        }
    }

    pub(crate) async fn assert_workspace_owner(
        &self,
        binding: &SyncBinding,
    ) -> Result<(), SyncError> {
        let mut connection = self.pool.acquire().await?;
        Self::assert_workspace_owner_on(&mut connection, binding).await
    }
}
