use super::*;
#[cfg(test)]
use unfour_core::domain::CommandContext;

impl ApiClientService {
    #[cfg(test)]
    pub(crate) async fn save_request(&self, input: ApiRequestInput) -> AppResult<ApiSavedRequest> {
        let context = CommandContext::local("api.save_request");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .save_request_on(&mut transaction, &context, input)
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    #[cfg(test)]
    pub(crate) async fn update_request(
        &self,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
    ) -> AppResult<ApiSavedRequest> {
        let context = CommandContext::local("api.update_request");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .update_request_on(&mut transaction, &context, workspace_id, request_id, input)
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    pub async fn list_saved_requests(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        validate_workspace_id(&workspace_id)?;

        let items = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              settings_json, pre_request_script, post_response_script, script_schema_version,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
            FROM api_requests
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY collection_id, COALESCE(parent_folder_id, ''), sort_order, updated_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(items)
    }

    #[cfg(test)]
    pub(crate) async fn duplicate_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<ApiSavedRequest> {
        let context = CommandContext::local("api.duplicate_request");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .duplicate_request_on(&mut transaction, &context, workspace_id, request_id)
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    #[cfg(test)]
    pub(crate) async fn delete_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        let context = CommandContext::local("api.delete_request");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .delete_request_on(&mut transaction, &context, workspace_id, request_id)
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    #[cfg(test)]
    pub(crate) async fn move_request(
        &self,
        workspace_id: String,
        request_id: String,
        collection_id: Option<String>,
        parent_folder_id: Option<String>,
    ) -> AppResult<ApiSavedRequest> {
        let context = CommandContext::local("api.request.move");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .move_request_on(
                &mut transaction,
                &context,
                workspace_id,
                request_id,
                collection_id,
                parent_folder_id,
            )
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    #[cfg(test)]
    pub(crate) async fn reorder_requests(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        request_ids: Vec<String>,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        let context = CommandContext::local("api.request.reorder");
        let mut transaction = self.db.pool().begin().await?;
        let outcome = self
            .reorder_requests_on(
                &mut transaction,
                &context,
                workspace_id,
                collection_id,
                parent_folder_id,
                request_ids,
            )
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    pub async fn get_saved_request(&self, id: &str) -> AppResult<ApiSavedRequest> {
        let saved = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              settings_json, pre_request_script, post_response_script, script_schema_version,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
            FROM api_requests
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        saved.ok_or_else(|| AppError::NotFound("api request".to_string()))
    }
}
