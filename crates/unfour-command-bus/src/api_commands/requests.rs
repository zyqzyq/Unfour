//! Request authoring and history queries; route writes share the business transaction.

use crate::{transaction::CommandActivity, CommandBus};
use unfour_core::domain::CommandContext;
use unfour_core::models::{ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiSavedRequest};
use unfour_core::network::{ApiRoute, ApiRouteEdit};
use unfour_core::AppResult;
use unfour_http_engine::local_routes;

impl CommandBus {
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
        self.save_api_request_routed(input, None).await
    }

    pub async fn save_api_request_routed(
        &self,
        input: ApiRequestInput,
        route: Option<ApiRoute>,
    ) -> AppResult<ApiSavedRequest> {
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
                    let saved = service
                        .save_request_on(connection, &executor_context, input)
                        .await?;
                    if let Some(route) = route {
                        local_routes::save_request_route(
                            connection,
                            &saved.value.workspace_id,
                            &saved.value.id,
                            &route,
                        )
                        .await?;
                    }
                    Ok(saved)
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
        self.update_api_request_if_current(workspace_id, request_id, input, None)
            .await
    }

    pub async fn update_api_request_if_current(
        &self,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
        expected_revision: Option<i64>,
    ) -> AppResult<ApiSavedRequest> {
        self.update_api_request_routed(workspace_id, request_id, input, expected_revision, None)
            .await
    }

    pub async fn update_api_request_routed(
        &self,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
        expected_revision: Option<i64>,
        route: Option<ApiRouteEdit>,
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
                    let saved = service
                        .update_request_if_current_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            request_id,
                            input,
                            expected_revision,
                        )
                        .await?;
                    if let Some(route) = route {
                        local_routes::update_request_route(
                            connection,
                            &saved.value.workspace_id,
                            &saved.value.id,
                            &route,
                        )
                        .await?;
                    }
                    Ok(saved)
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
                    let route =
                        local_routes::request_route(connection, &workspace_id, &request_id).await?;
                    let duplicate = service
                        .duplicate_request_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            request_id,
                        )
                        .await?;
                    local_routes::save_request_route(
                        connection,
                        &duplicate.value.workspace_id,
                        &duplicate.value.id,
                        &route,
                    )
                    .await?;
                    Ok(duplicate)
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
}
