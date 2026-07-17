use super::*;

impl CommandBus {
    pub async fn api_environments_list(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<ApiEnvironment>> {
        self.api_client.list_environments(workspace_id).await
    }

    pub async fn api_environment_create(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiEnvironment> {
        let environment = self
            .api_client
            .create_environment(workspace_id.clone(), name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.environment.create",
                Some(&environment.id),
                serde_json::json!({ "name": environment.name }),
            )
            .await?;
        Ok(environment)
    }

    pub async fn api_environment_update(
        &self,
        workspace_id: String,
        environment_id: String,
        name: String,
        variables: Vec<KeyValue>,
    ) -> AppResult<ApiEnvironment> {
        let environment = self
            .api_client
            .update_environment(workspace_id.clone(), environment_id, name, variables)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.environment.update",
                Some(&environment.id),
                serde_json::json!({ "variableCount": environment.variables.len() }),
            )
            .await?;
        Ok(environment)
    }

    pub async fn api_environment_delete(
        &self,
        workspace_id: String,
        environment_id: String,
    ) -> AppResult<Vec<ApiEnvironment>> {
        let environments = self
            .api_client
            .delete_environment(workspace_id.clone(), environment_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.environment.delete",
                Some(&environment_id),
                serde_json::json!({ "softDelete": true }),
            )
            .await?;
        Ok(environments)
    }

    pub async fn api_environment_activate(
        &self,
        workspace_id: String,
        environment_id: Option<String>,
    ) -> AppResult<Vec<ApiEnvironment>> {
        self.api_client
            .activate_environment(workspace_id, environment_id)
            .await
    }

    pub async fn api_collection_list(&self, workspace_id: String) -> AppResult<Vec<ApiCollection>> {
        self.api_client.list_collections(workspace_id).await
    }

    pub async fn api_collection_export(
        &self,
        workspace_id: String,
        collection_id: String,
        format: ApiCollectionExportFormat,
    ) -> AppResult<ApiCollectionExportArtifact> {
        self.api_client
            .export_collection_openapi(workspace_id, collection_id, format)
            .await
    }

    pub async fn api_collection_import(
        &self,
        workspace_id: String,
        content: String,
    ) -> AppResult<ApiCollectionImportResult> {
        let result = self
            .api_client
            .import_collection_openapi(workspace_id.clone(), content)
            .await?;
        if let Some(collection) = &result.collection {
            self.activity_log
                .record(
                    Some(&workspace_id),
                    "api.collection.import",
                    Some(&collection.id),
                    serde_json::json!({
                        "folderCount": result.folder_count,
                        "requestCount": result.request_count,
                    }),
                )
                .await?;
        }
        Ok(result)
    }

    pub async fn api_collection_create(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        let collection = self
            .api_client
            .create_collection(workspace_id.clone(), name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.create",
                Some(&collection.id),
                serde_json::json!({ "name": collection.name }),
            )
            .await?;
        Ok(collection)
    }

    pub async fn api_collection_rename(
        &self,
        workspace_id: String,
        collection_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        let collection = self
            .api_client
            .rename_collection(workspace_id.clone(), collection_id, name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.rename",
                Some(&collection.id),
                serde_json::json!({ "name": collection.name }),
            )
            .await?;
        Ok(collection)
    }

    pub async fn api_collection_delete(
        &self,
        workspace_id: String,
        collection_id: String,
    ) -> AppResult<Vec<ApiCollection>> {
        let collections = self
            .api_client
            .delete_collection(workspace_id.clone(), collection_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.delete",
                Some(&collection_id),
                serde_json::json!({ "softDelete": true, "cascade": true }),
            )
            .await?;
        Ok(collections)
    }

    pub async fn api_collection_folders_list(
        &self,
        workspace_id: String,
        collection_id: Option<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        self.api_client
            .list_collection_folders(workspace_id, collection_id)
            .await
    }

    pub async fn api_collection_folder_create(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        let folder = self
            .api_client
            .create_collection_folder(workspace_id.clone(), collection_id, parent_folder_id, name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.create",
                Some(&folder.id),
                serde_json::json!({ "collectionId": folder.collection_id, "parentFolderId": folder.parent_folder_id }),
            )
            .await?;
        Ok(folder)
    }

    pub async fn api_collection_folder_rename(
        &self,
        workspace_id: String,
        folder_id: String,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        let folder = self
            .api_client
            .rename_collection_folder(workspace_id.clone(), folder_id, name)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.rename",
                Some(&folder.id),
                serde_json::json!({ "name": folder.name }),
            )
            .await?;
        Ok(folder)
    }

    pub async fn api_collection_folder_delete(
        &self,
        workspace_id: String,
        folder_id: String,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        let folders = self
            .api_client
            .delete_collection_folder(workspace_id.clone(), folder_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.delete",
                Some(&folder_id),
                serde_json::json!({ "softDelete": true, "recursive": true }),
            )
            .await?;
        Ok(folders)
    }

    pub async fn api_collection_folder_move(
        &self,
        workspace_id: String,
        folder_id: String,
        target_parent_folder_id: Option<String>,
    ) -> AppResult<ApiCollectionFolder> {
        let folder = self
            .api_client
            .move_collection_folder(workspace_id.clone(), folder_id, target_parent_folder_id)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.move",
                Some(&folder.id),
                serde_json::json!({ "parentFolderId": folder.parent_folder_id }),
            )
            .await?;
        Ok(folder)
    }

    pub async fn api_collection_folders_reorder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        folder_ids: Vec<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        let folders = self
            .api_client
            .reorder_collection_folders(
                workspace_id.clone(),
                collection_id.clone(),
                parent_folder_id,
                folder_ids,
            )
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.collection.folder.reorder",
                Some(&collection_id),
                serde_json::json!({ "folderCount": folders.len() }),
            )
            .await?;
        Ok(folders)
    }

    pub async fn api_request_move(
        &self,
        workspace_id: String,
        request_id: String,
        collection_id: Option<String>,
        parent_folder_id: Option<String>,
    ) -> AppResult<ApiSavedRequest> {
        let saved = self
            .api_client
            .move_request(
                workspace_id.clone(),
                request_id,
                collection_id,
                parent_folder_id,
            )
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.request.move",
                Some(&saved.id),
                serde_json::json!({ "collectionId": saved.collection_id, "parentFolderId": saved.parent_folder_id }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn api_requests_reorder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        request_ids: Vec<String>,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        let requests = self
            .api_client
            .reorder_requests(
                workspace_id.clone(),
                collection_id.clone(),
                parent_folder_id,
                request_ids,
            )
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.request.reorder",
                Some(&collection_id),
                serde_json::json!({ "requestCount": requests.len() }),
            )
            .await?;
        Ok(requests)
    }

    pub async fn workspace_layout(&self, workspace_id: String) -> AppResult<WorkspaceLayout> {
        self.workspace.layout(workspace_id).await
    }

    pub async fn workspace_layout_update(
        &self,
        workspace_id: String,
        layout: WorkspaceLayout,
    ) -> AppResult<WorkspaceLayout> {
        self.workspace.update_layout(workspace_id, layout).await
    }

    pub async fn send_api_request(&self, input: ApiRequestInput) -> AppResult<ApiResponse> {
        let response = self.api_client.send(input.clone()).await?;
        self.activity_log
            .record(
                Some(&input.workspace_id),
                "api.send_request",
                Some(&response.history_id),
                serde_json::json!({
                    "method": input.method,
                    "url": input.url,
                    "status": response.status
                }),
            )
            .await?;
        Ok(response)
    }

    pub async fn list_api_history(
        &self,
        workspace_id: String,
        limit: Option<i64>,
    ) -> AppResult<Vec<ApiHistoryItem>> {
        self.api_client.list_history(workspace_id, limit).await
    }

    pub async fn api_history_detail(
        &self,
        workspace_id: String,
        history_id: String,
    ) -> AppResult<ApiHistoryDetail> {
        self.api_client
            .history_detail(workspace_id, history_id)
            .await
    }

    pub async fn save_api_request(&self, input: ApiRequestInput) -> AppResult<ApiSavedRequest> {
        let saved = self.api_client.save_request(input).await?;
        self.activity_log
            .record(
                Some(&saved.workspace_id),
                "api.save_request",
                Some(&saved.id),
                serde_json::json!({ "name": saved.name, "method": saved.method }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn update_api_request(
        &self,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
    ) -> AppResult<ApiSavedRequest> {
        let saved = self
            .api_client
            .update_request(workspace_id.clone(), request_id, input)
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.update_request",
                Some(&saved.id),
                serde_json::json!({ "name": saved.name, "method": saved.method }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn list_saved_api_requests(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        self.api_client.list_saved_requests(workspace_id).await
    }

    pub async fn duplicate_api_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<ApiSavedRequest> {
        let saved = self
            .api_client
            .duplicate_request(workspace_id.clone(), request_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.duplicate_request",
                Some(&saved.id),
                serde_json::json!({ "sourceId": request_id, "name": saved.name }),
            )
            .await?;
        Ok(saved)
    }

    pub async fn delete_api_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        let requests = self
            .api_client
            .delete_request(workspace_id.clone(), request_id.clone())
            .await?;
        self.activity_log
            .record(
                Some(&workspace_id),
                "api.delete_request",
                Some(&request_id),
                serde_json::json!({ "softDelete": true }),
            )
            .await?;
        Ok(requests)
    }

    pub async fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms_override: Option<u64>,
    ) -> AppResult<ApiResponse> {
        let state = self.read_workspace_state().await?;
        self.execute_saved_api_request_in_workspace(
            Some(state.active_workspace_id),
            request_id,
            timeout_ms_override,
        )
        .await
    }

    pub async fn execute_saved_api_request_in_workspace(
        &self,
        workspace_id: Option<String>,
        request_id: &str,
        timeout_ms_override: Option<u64>,
    ) -> AppResult<ApiResponse> {
        let saved = self.api_client.get_saved_request(request_id).await?;

        if workspace_id
            .as_deref()
            .is_some_and(|id| saved.workspace_id != id)
        {
            return Err(unfour_core::AppError::NotFound("api request".to_string()));
        }

        let headers: Vec<KeyValue> = serde_json::from_str(&saved.headers_json).unwrap_or_default();
        let query: Vec<KeyValue> = serde_json::from_str(&saved.query_json).unwrap_or_default();
        let timeout_ms = timeout_ms_override.map(|t| t.min(60_000));

        let input = ApiRequestInput {
            workspace_id: saved.workspace_id.clone(),
            name: Some(saved.name.clone()),
            parent_folder_id: saved.parent_folder_id.clone(),
            collection_id: Some(saved.collection_id.clone()),
            auth_json: Some(saved.auth_json.clone()),
            method: saved.method.clone(),
            url: saved.url.clone(),
            headers,
            query,
            body: saved.body.clone(),
            body_kind: saved.body_kind.clone(),
            timeout_ms,
        };

        self.api_client.send(input).await
    }
}
