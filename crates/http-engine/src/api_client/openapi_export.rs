use super::*;
use unfour_core::models::{
    ApiCollectionExportArtifact, ApiCollectionExportFormat, ApiHistoryDetail,
};

mod content;
mod convert;
mod model;
mod source;

use convert::{build_document, sanitize_file_name, serialize_document};
use model::OpenApiExportSource;

impl ApiClientService {
    pub async fn export_collection_openapi(
        &self,
        workspace_id: String,
        collection_id: String,
        format: ApiCollectionExportFormat,
    ) -> AppResult<ApiCollectionExportArtifact> {
        validate_workspace_id(&workspace_id)?;
        if collection_id.trim().is_empty() {
            return Err(AppError::Validation(
                "api collection id cannot be empty".to_string(),
            ));
        }

        let collection = self.get_collection(&workspace_id, &collection_id).await?;
        let folders = self
            .list_collection_folders(workspace_id.clone(), Some(collection_id.clone()))
            .await?;
        let requests = self
            .list_saved_requests(workspace_id.clone())
            .await?
            .into_iter()
            .filter(|request| request.collection_id == collection_id)
            .collect();
        let environments = self.list_environments(workspace_id.clone()).await?;
        let histories = self
            .list_collection_export_histories(&workspace_id, &collection_id)
            .await?;
        let source = OpenApiExportSource {
            collection,
            collection_auth_json: None,
            collection_base_url: None,
            collection_version: None,
            environments,
            folders,
            histories,
            requests,
        };
        let document = build_document(&source)?;
        let content = serialize_document(&document, format)?;
        let base_name = sanitize_file_name(&source.collection.name);
        let (extension, media_type) = match format {
            ApiCollectionExportFormat::Json => ("json", "application/json"),
            ApiCollectionExportFormat::Yaml => ("yaml", "application/yaml"),
        };

        Ok(ApiCollectionExportArtifact {
            content,
            media_type: media_type.to_string(),
            suggested_file_name: format!("{base_name}.openapi.{extension}"),
        })
    }

    async fn list_collection_export_histories(
        &self,
        workspace_id: &str,
        collection_id: &str,
    ) -> AppResult<Vec<ApiHistoryDetail>> {
        let histories = sqlx::query_as::<_, ApiHistoryDetail>(
            r#"
            SELECT DISTINCT
              history.id, history.workspace_id, history.name, history.method, history.url,
              history.request_headers_json, history.request_query_json, history.request_body,
              history.status, history.duration_ms, history.response_headers_json,
              history.response_body_preview, history.created_at, history.updated_at
            FROM api_history history
            JOIN api_requests request
              ON request.workspace_id = history.workspace_id
             AND request.collection_id = ?2
             AND request.deleted_at IS NULL
             AND request.name = history.name
             AND UPPER(request.method) = UPPER(history.method)
             AND request.url = history.url
             AND request.headers_json = history.request_headers_json
             AND request.query_json = history.request_query_json
             AND COALESCE(request.body, '') = COALESCE(history.request_body, '')
            WHERE history.workspace_id = ?1
            ORDER BY history.created_at DESC
            "#,
        )
        .bind(workspace_id)
        .bind(collection_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(histories)
    }
}

#[cfg(test)]
mod tests;
