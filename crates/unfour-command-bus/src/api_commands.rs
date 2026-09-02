use super::*;
use crate::transaction::CommandActivity;
use unfour_core::domain::CommandContext;

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
                    .find(|current| current.key == variable.key);
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
        Ok(crate::workspace_variable_commands::legacy_api_environment(
            self.workspace_environment_update(workspace_id, environment_id, name, variables)
                .await?,
        ))
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

    pub async fn api_collection_list(&self, workspace_id: String) -> AppResult<Vec<ApiCollection>> {
        self.api_client.list_collections(workspace_id).await
    }

    pub async fn api_collection_export(
        &self,
        workspace_id: String,
        collection_id: String,
        format: ApiCollectionExportFormat,
    ) -> AppResult<ApiCollectionExportArtifact> {
        let environments = self.api_environments_list(workspace_id.clone()).await?;
        self.api_client
            .export_collection_openapi(workspace_id, collection_id, format, environments)
            .await
    }

    pub async fn api_collection_import(
        &self,
        workspace_id: String,
        content: String,
    ) -> AppResult<ApiCollectionImportResult> {
        let context = CommandContext::local("api.collection.import");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let content_bytes = content.len();
        self.execute_domain_command_with_activity(
            context,
            move |result: &ApiCollectionImportResult| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.import",
                target: result
                    .collection
                    .as_ref()
                    .map(|collection| collection.id.clone()),
                details: serde_json::json!({
                    "contentBytes": content_bytes,
                    "folderCount": result.folder_count,
                    "requestCount": result.request_count,
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .import_collection_openapi_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            content,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_create(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        let context = CommandContext::local("api.collection.create");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |collection: &ApiCollection| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.create",
                target: Some(collection.id.clone()),
                details: serde_json::json!({ "name": collection.name }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .create_collection_on(connection, &executor_context, workspace_id, name)
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_rename(
        &self,
        workspace_id: String,
        collection_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        let context = CommandContext::local("api.collection.rename");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |collection: &ApiCollection| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.rename",
                target: Some(collection.id.clone()),
                details: serde_json::json!({ "name": collection.name }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .rename_collection_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                            name,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_delete(
        &self,
        workspace_id: String,
        collection_id: String,
    ) -> AppResult<Vec<ApiCollection>> {
        let context = CommandContext::local("api.collection.delete");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_collection_id = collection_id.clone();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.delete",
                target: Some(activity_collection_id),
                details: serde_json::json!({ "softDelete": true, "cascade": true }),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .delete_collection_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                        )
                        .await
                })
            },
        )
        .await
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
        let context = CommandContext::local("api.collection.folder.create");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |folder: &ApiCollectionFolder| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.create",
                target: Some(folder.id.clone()),
                details: serde_json::json!({
                    "collectionId": folder.collection_id,
                    "parentFolderId": folder.parent_folder_id,
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .create_collection_folder_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                            parent_folder_id,
                            name,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folder_rename(
        &self,
        workspace_id: String,
        folder_id: String,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        let context = CommandContext::local("api.collection.folder.rename");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |folder: &ApiCollectionFolder| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.rename",
                target: Some(folder.id.clone()),
                details: serde_json::json!({ "name": folder.name }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .rename_collection_folder_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            folder_id,
                            name,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folder_delete(
        &self,
        workspace_id: String,
        folder_id: String,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        let context = CommandContext::local("api.collection.folder.delete");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_folder_id = folder_id.clone();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.delete",
                target: Some(activity_folder_id),
                details: serde_json::json!({ "softDelete": true, "recursive": true }),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .delete_collection_folder_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            folder_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folder_move(
        &self,
        workspace_id: String,
        folder_id: String,
        target_parent_folder_id: Option<String>,
    ) -> AppResult<ApiCollectionFolder> {
        let context = CommandContext::local("api.collection.folder.move");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |folder: &ApiCollectionFolder| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.move",
                target: Some(folder.id.clone()),
                details: serde_json::json!({ "parentFolderId": folder.parent_folder_id }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .move_collection_folder_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            folder_id,
                            target_parent_folder_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folders_reorder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        folder_ids: Vec<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        let context = CommandContext::local("api.collection.folder.reorder");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_collection_id = collection_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |folders: &Vec<ApiCollectionFolder>| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.reorder",
                target: Some(activity_collection_id),
                details: serde_json::json!({ "folderCount": folders.len() }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .reorder_collection_folders_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                            parent_folder_id,
                            folder_ids,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_request_move(
        &self,
        workspace_id: String,
        request_id: String,
        collection_id: Option<String>,
        parent_folder_id: Option<String>,
    ) -> AppResult<ApiSavedRequest> {
        let context = CommandContext::local("api.request.move");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |request: &ApiSavedRequest| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.request.move",
                target: Some(request.id.clone()),
                details: serde_json::json!({
                    "collectionId": request.collection_id,
                    "parentFolderId": request.parent_folder_id,
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .move_request_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            request_id,
                            collection_id,
                            parent_folder_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_requests_reorder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        request_ids: Vec<String>,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        let context = CommandContext::local("api.request.reorder");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_collection_id = collection_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |requests: &Vec<ApiSavedRequest>| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.request.reorder",
                target: Some(activity_collection_id),
                details: serde_json::json!({ "requestCount": requests.len() }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .reorder_requests_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                            parent_folder_id,
                            request_ids,
                        )
                        .await
                })
            },
        )
        .await
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
        self.send_api_request_in_environment(input, None).await
    }

    pub async fn send_api_request_in_environment(
        &self,
        input: ApiRequestInput,
        environment_id_override: Option<String>,
    ) -> AppResult<ApiResponse> {
        let resolved_input = self
            .resolve_api_request_input_in_environment(
                input.clone(),
                environment_id_override.as_deref(),
            )
            .await?;
        let response = self.api_client.send(resolved_input).await?;
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
        let context = CommandContext::local("api.save_request");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = input.workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |request: &ApiSavedRequest| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.save_request",
                target: Some(request.id.clone()),
                details: serde_json::json!({
                    "name": request.name,
                    "method": request.method,
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .save_request_on(connection, &executor_context, input)
                        .await
                })
            },
        )
        .await
    }

    pub async fn update_api_request(
        &self,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
    ) -> AppResult<ApiSavedRequest> {
        let context = CommandContext::local("api.update_request");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |request: &ApiSavedRequest| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.update_request",
                target: Some(request.id.clone()),
                details: serde_json::json!({
                    "name": request.name,
                    "method": request.method,
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .update_request_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            request_id,
                            input,
                        )
                        .await
                })
            },
        )
        .await
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
        let context = CommandContext::local("api.duplicate_request");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let source_id = request_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |request: &ApiSavedRequest| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.duplicate_request",
                target: Some(request.id.clone()),
                details: serde_json::json!({
                    "sourceId": source_id,
                    "name": request.name,
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .duplicate_request_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            request_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn delete_api_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        let context = CommandContext::local("api.delete_request");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_request_id = request_id.clone();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.delete_request",
                target: Some(activity_request_id),
                details: serde_json::json!({ "softDelete": true }),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .delete_request_on(connection, &executor_context, workspace_id, request_id)
                        .await
                })
            },
        )
        .await
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
            pre_request_script: saved.pre_request_script.clone(),
            post_response_script: saved.post_response_script.clone(),
            script_schema_version: saved.script_schema_version,
            temporary_variables: vec![],
        };

        let input = self.resolve_api_request_input(input).await?;
        self.api_client.send(input).await
    }

    pub async fn execute_saved_api_request_with_scripts_in_workspace(
        &self,
        workspace_id: Option<String>,
        request_id: &str,
        timeout_ms_override: Option<u64>,
        environment_id_override: Option<String>,
    ) -> AppResult<RequestExecutionResult> {
        let saved = self.api_client.get_saved_request(request_id).await?;

        if workspace_id
            .as_deref()
            .is_some_and(|id| saved.workspace_id != id)
        {
            return Err(unfour_core::AppError::NotFound("api request".to_string()));
        }

        let headers: Vec<KeyValue> = serde_json::from_str(&saved.headers_json).unwrap_or_default();
        let query: Vec<KeyValue> = serde_json::from_str(&saved.query_json).unwrap_or_default();
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
            timeout_ms: timeout_ms_override.map(|timeout| timeout.min(60_000)),
            pre_request_script: saved.pre_request_script.clone(),
            post_response_script: saved.post_response_script.clone(),
            script_schema_version: saved.script_schema_version,
            temporary_variables: vec![],
        };

        self.send_api_request_with_scripts_in_environment(input, environment_id_override)
            .await
    }
}
