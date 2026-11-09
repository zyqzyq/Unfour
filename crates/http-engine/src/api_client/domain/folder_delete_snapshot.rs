use super::{folder_on, list_folders_on, ApiClientService};
use sqlx::SqliteConnection;
use unfour_core::{models::ApiFolderDeleteSnapshot, AppResult};

impl ApiClientService {
    pub async fn folder_delete_snapshot(
        &self,
        workspace_id: &str,
        folder_id: &str,
    ) -> AppResult<ApiFolderDeleteSnapshot> {
        let mut tx = self.db.pool().begin().await?;
        let snapshot = self
            .folder_delete_snapshot_on(&mut tx, workspace_id, folder_id)
            .await?;
        tx.commit().await?;
        Ok(snapshot)
    }
    pub async fn folder_delete_snapshot_on(
        &self,
        connection: &mut SqliteConnection,
        workspace_id: &str,
        folder_id: &str,
    ) -> AppResult<ApiFolderDeleteSnapshot> {
        folder_on(connection, workspace_id, folder_id, false).await?;
        let folders = list_folders_on(connection, workspace_id, None).await?;
        let mut ids = std::collections::BTreeSet::from([folder_id.to_owned()]);
        loop {
            let before = ids.len();
            for f in &folders {
                if f.parent_folder_id.as_ref().is_some_and(|p| ids.contains(p)) {
                    ids.insert(f.id.clone());
                }
            }
            if ids.len() == before {
                break;
            }
        }
        let mut revisions = folders
            .iter()
            .filter(|f| ids.contains(&f.id))
            .map(|f| (f.id.clone(), f.revision))
            .collect::<Vec<_>>();
        // Do not load bodies, credentials, or scripts just to prepare confirmation.
        // Request deletion traverses tombstoned intermediate folders too. Match
        // that reachability rather than reusing the visible-folder revision set.
        let mut requests: Vec<(String, i64)> = sqlx::query_as(
            r#"
            WITH RECURSIVE folder_tree(id) AS (
              SELECT id FROM api_collection_folders WHERE workspace_id = ?1 AND id = ?2
              UNION
              SELECT child.id FROM api_collection_folders child
              JOIN folder_tree parent ON child.parent_folder_id = parent.id
              WHERE child.workspace_id = ?1
            )
            SELECT id, revision FROM api_requests
            WHERE workspace_id = ?1 AND parent_folder_id IN (SELECT id FROM folder_tree)
              AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(folder_id)
        .fetch_all(&mut *connection)
        .await?;
        revisions.sort();
        requests.sort();
        Ok(ApiFolderDeleteSnapshot {
            folders: revisions,
            requests,
        })
    }
}
