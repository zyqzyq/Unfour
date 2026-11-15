//! Direct send and saved replay orchestration over the existing execution paths.

use crate::{api_execution::mcp_timeout, transaction::CommandActivity, CommandBus};
use unfour_core::models::{ApiRequestInput, ApiResponse, ApiSavedRequest, RequestExecutionResult};
use unfour_core::AppResult;

impl CommandBus {
    pub async fn send_api_request(&self, input: ApiRequestInput) -> AppResult<ApiResponse> {
        self.send_api_request_in_environment(input, None).await
    }

    pub async fn send_api_request_in_environment(
        &self,
        input: ApiRequestInput,
        environment_id_override: Option<String>,
    ) -> AppResult<ApiResponse> {
        let environment_id = self
            .resolve_api_environment_id(&input.workspace_id, environment_id_override.as_deref())
            .await?;
        let (resolved_input, secrets) = self
            .resolve_api_request_input_for_environment(input.clone(), environment_id.as_deref())
            .await?;
        let resolved_input = self.resolve_desktop_api_timeout(resolved_input).await?;
        let routes = self
            .api_routes_for_environment(&input.workspace_id, None, environment_id)
            .await?;
        let mut response = self
            .send_routed_http(
                resolved_input,
                tokio_util::sync::CancellationToken::new(),
                &secrets,
                &routes.effective,
                None,
            )
            .await?;
        let activity_warnings = self
            .record_completion_activity(
                CommandActivity {
                    workspace_id: Some(input.workspace_id.clone()),
                    action: "api.send_request",
                    target: (!response.history_id.is_empty()).then(|| response.history_id.clone()),
                    details: serde_json::json!({
                        "method": response.request_snapshot.as_ref().map(|snapshot| &snapshot.method),
                        "status": response.status
                    }),
                },
                "api",
                "send_request",
            )
            .await;
        response.warnings.extend(activity_warnings);
        Ok(response)
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
        let saved = self
            .saved_request_in_workspace(workspace_id.as_deref(), request_id)
            .await?;
        let input = saved_replay_input(&saved, timeout_ms_override)?;
        let environment_id = self
            .resolve_api_environment_id(&input.workspace_id, None)
            .await?;
        let (input, secrets) = self
            .resolve_api_request_input_for_environment(input, environment_id.as_deref())
            .await?;
        let routes = self
            .api_routes_for_environment(&input.workspace_id, Some(&saved.id), environment_id)
            .await?;
        self.send_routed_http(
            input,
            tokio_util::sync::CancellationToken::new(),
            &secrets,
            &routes.effective,
            None,
        )
        .await
    }

    pub async fn execute_saved_api_request_with_scripts_in_workspace(
        &self,
        workspace_id: Option<String>,
        request_id: &str,
        timeout_ms_override: Option<u64>,
        environment_id_override: Option<String>,
    ) -> AppResult<RequestExecutionResult> {
        let execution_id = unfour_core::id::new_id();
        self.execute_saved_api_request_with_scripts_controlled_in_workspace(
            &execution_id,
            workspace_id,
            request_id,
            timeout_ms_override,
            environment_id_override,
        )
        .await
    }

    /// Same saved replay path, with a caller-owned ID for cooperative cancellation.
    pub async fn execute_saved_api_request_with_scripts_controlled_in_workspace(
        &self,
        execution_id: &str,
        workspace_id: Option<String>,
        request_id: &str,
        timeout_ms_override: Option<u64>,
        environment_id_override: Option<String>,
    ) -> AppResult<RequestExecutionResult> {
        let saved = self
            .saved_request_in_workspace(workspace_id.as_deref(), request_id)
            .await?;
        self.execute_saved_api_request_snapshot_controlled(
            execution_id,
            &saved,
            timeout_ms_override,
            environment_id_override,
        )
        .await
    }

    async fn saved_request_in_workspace(
        &self,
        workspace_id: Option<&str>,
        request_id: &str,
    ) -> AppResult<ApiSavedRequest> {
        let saved = self.api_client.get_saved_request(request_id).await?;
        if workspace_id.is_some_and(|id| saved.workspace_id != id) {
            return Err(unfour_core::AppError::NotFound("api request".to_string()));
        }
        Ok(saved)
    }

    /// Replay the saved definition the adapter already authorized. The request
    /// may be edited by another client while the adapter evaluates its policy.
    pub async fn execute_saved_api_request_snapshot_controlled(
        &self,
        execution_id: &str,
        saved: &ApiSavedRequest,
        timeout_ms_override: Option<u64>,
        environment_id_override: Option<String>,
    ) -> AppResult<RequestExecutionResult> {
        self.send_api_request_routed(
            execution_id,
            saved_replay_input(saved, timeout_ms_override)?,
            environment_id_override,
            Some(&saved.id),
            None,
        )
        .await
    }
}

fn saved_replay_input(
    saved: &ApiSavedRequest,
    timeout_ms_override: Option<u64>,
) -> AppResult<ApiRequestInput> {
    Ok(ApiRequestInput {
        workspace_id: saved.workspace_id.clone(),
        name: Some(saved.name.clone()),
        parent_folder_id: saved.parent_folder_id.clone(),
        collection_id: Some(saved.collection_id.clone()),
        auth_json: Some(saved.auth_json.clone()),
        method: saved.method.clone(),
        url: saved.url.clone(),
        headers: serde_json::from_str(&saved.headers_json).unwrap_or_default(),
        query: serde_json::from_str(&saved.query_json).unwrap_or_default(),
        body: saved.body.clone(),
        body_kind: saved.body_kind.clone(),
        timeout_ms: Some(mcp_timeout(timeout_ms_override)?),
        pre_request_script: saved.pre_request_script.clone(),
        post_response_script: saved.post_response_script.clone(),
        script_schema_version: saved.script_schema_version,
        multipart_parts: vec![],
        temporary_variables: vec![],
    })
}
