use crate::LocalDb;
use unfour_core::{models::LocalDataVersion, AppResult};

impl LocalDb {
    /// Read only compact clocks. Triggers update these in the writer's own
    /// transaction, including independent MCP processes and Cloud Sync apply.
    pub async fn local_data_versions(&self) -> AppResult<Vec<LocalDataVersion>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT workspace_id, domain, CAST(version AS TEXT) FROM local_data_versions",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(workspace_id, domain, version)| LocalDataVersion {
                workspace_id,
                domain,
                version,
            })
            .collect())
    }
}
