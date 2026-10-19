use crate::flow_authoring::{
    apply_api_arguments, ssh_inputs, validate_api_arguments, validate_sql_argument,
};
use crate::CommandBus;
use serde_json::{json, Value};
use std::{future::Future, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
use unfour_core::{models::*, AppResult};
use unfour_flow_engine::{expression::invalid, FlowExecutor, FlowFuture, FlowService};

impl CommandBus {
    fn flow_service(&self) -> FlowService {
        FlowService::new(self.db.clone())
    }
    pub async fn list_flows(&self, workspace_id: String) -> AppResult<Vec<FlowDefinition>> {
        self.flow_service().list(&workspace_id).await
    }
    pub async fn list_flow_summaries(&self, workspace_id: String) -> AppResult<Vec<FlowSummary>> {
        self.flow_service().list_summaries(&workspace_id).await
    }
    pub async fn get_flow(
        &self,
        workspace_id: String,
        flow_id: String,
    ) -> AppResult<FlowDefinition> {
        self.flow_service().get(&workspace_id, &flow_id).await
    }
    pub async fn save_flow(&self, input: FlowDefinition) -> AppResult<FlowDefinition> {
        unfour_flow_engine::validate_definition(&input)?;
        for step in &input.steps {
            let action = match &step.node {
                FlowNode::Action { action } => action,
                FlowNode::Poll { probe, .. } | FlowNode::WaitUntil { probe, .. } => probe,
                _ => continue,
            };
            if action.capability == FlowCapability::Api {
                validate_api_arguments(&action.arguments, true)?;
            }
            if action.capability == FlowCapability::Database {
                let connection = self
                    .list_database_connections(input.workspace_id.clone())
                    .await?
                    .into_iter()
                    .find(|c| c.id == action.resource_id)
                    .ok_or_else(|| invalid("FLOW_RESOURCE_MISSING"))?;
                validate_sql_argument(&action.arguments, &connection.driver)?;
            }
            if action.capability == FlowCapability::Ssh {
                if action
                    .connection_id
                    .as_deref()
                    .is_none_or(|id| id.trim().is_empty())
                {
                    return Err(invalid("FLOW_RESOURCE_MISSING"));
                }
                let detail = self
                    .ssh
                    .get_task(&input.workspace_id, &action.resource_id)
                    .await?;
                let names = unfour_ssh_engine::SshService::detected_task_inputs(&detail.steps)?;
                // Environment is selected at run time; explicit legacy inputs can
                // already be checked without inventing an environment at save time.
                if action.arguments.get("workspaceDefaults") != Some(&json!(true)) {
                    ssh_inputs(&action.arguments, &names, &Default::default(), true)?;
                }
            }
        }
        self.flow_service().save(input).await
    }
    pub async fn delete_flow(&self, workspace_id: String, flow_id: String) -> AppResult<()> {
        self.flow_service().delete(&workspace_id, &flow_id).await
    }
    pub async fn delete_flow_at_revision(
        &self,
        workspace_id: String,
        flow_id: String,
        revision: i64,
    ) -> AppResult<()> {
        self.flow_service()
            .delete_at_revision(&workspace_id, &flow_id, revision)
            .await
    }
    pub async fn run_flow(&self, input: FlowRunInput) -> AppResult<FlowRun> {
        self.run_flow_at_revision(input, None).await
    }
    pub async fn run_flow_at_revision(
        &self,
        input: FlowRunInput,
        expected_revision: Option<i64>,
    ) -> AppResult<FlowRun> {
        self.workspace
            .resolve_variables(&input.workspace_id, input.environment_id.as_deref(), "")
            .await?;
        self.flow_service()
            .run_at_revision(input, Arc::new(self.clone()), expected_revision)
            .await
    }
    pub async fn get_flow_run(&self, workspace_id: String, run_id: String) -> AppResult<FlowRun> {
        self.flow_service().get_run(&workspace_id, &run_id).await
    }
    pub async fn list_flow_runs(
        &self,
        workspace_id: String,
        flow_id: String,
    ) -> AppResult<Vec<FlowRunSummary>> {
        self.flow_service().list_runs(&workspace_id, &flow_id).await
    }
    pub async fn list_flow_runs_page(
        &self,
        workspace_id: String,
        flow_id: String,
        limit: u32,
        cursor: Option<String>,
    ) -> AppResult<FlowRunPage> {
        self.flow_service()
            .list_runs_page(&workspace_id, &flow_id, limit, cursor.as_deref())
            .await
    }
    pub async fn cancel_flow_run(
        &self,
        workspace_id: String,
        run_id: String,
    ) -> AppResult<FlowRun> {
        self.flow_service().cancel(&workspace_id, &run_id).await
    }

    async fn flow_resource(&self, action: &FlowAction, input: &FlowRunInput) -> AppResult<Value> {
        let workspace = &input.workspace_id;
        match action.capability {
            FlowCapability::Api => {
                let saved = self
                    .api_client
                    .get_saved_request(&action.resource_id)
                    .await
                    .map_err(|_| invalid("FLOW_RESOURCE_MISSING"))?;
                if saved.workspace_id != *workspace || saved.deleted_at.is_some() {
                    return Err(invalid("FLOW_RESOURCE_MISSING"));
                }
                Ok(serde_json::to_value(saved)?)
            }
            FlowCapability::Ssh => {
                let detail = self
                    .ssh
                    .get_task(workspace, &action.resource_id)
                    .await
                    .map_err(|_| invalid("FLOW_RESOURCE_MISSING"))?;
                let connection = self
                    .list_ssh_connections(workspace.clone())
                    .await?
                    .into_iter()
                    .find(|c| Some(&c.id) == action.connection_id.as_ref())
                    .ok_or_else(|| invalid("FLOW_RESOURCE_MISSING"))?;
                // Last-used UI binding is neither execution context nor part of the snapshot identity.
                Ok(json!({"task": detail.task, "steps": detail.steps, "connection": connection}))
            }
            FlowCapability::Database => {
                let connection = self
                    .list_database_connections(workspace.clone())
                    .await?
                    .into_iter()
                    .find(|c| c.id == action.resource_id)
                    .ok_or_else(|| invalid("FLOW_RESOURCE_MISSING"))?;
                Ok(serde_json::to_value(connection)?)
            }
        }
    }
}

impl FlowExecutor for CommandBus {
    fn snapshot_secret_values(&self, snapshot: &Value) -> AppResult<Vec<String>> {
        match snapshot.get("request") {
            Some(request) => {
                let request: ApiRequestInput = serde_json::from_value(request.clone())?;
                unfour_http_engine::runtime_request_secret_values(&request)
            }
            None => Ok(Vec::new()),
        }
    }
    fn prepare<'a>(
        &'a self,
        action: &'a FlowAction,
        input: &'a FlowRunInput,
        probe: bool,
    ) -> FlowFuture<'a, Value> {
        Box::pin(async move {
            // Validate even flows without API actions against an explicitly supplied environment.
            self.workspace
                .resolve_variables(&input.workspace_id, input.environment_id.as_deref(), "")
                .await?;
            let resource = self.flow_resource(action, input).await?;
            let arguments = action
                .arguments
                .as_object()
                .ok_or_else(|| invalid("FLOW_ARGUMENTS_OBJECT_REQUIRED"))?;
            let allowed: &[&str] = match action.capability {
                FlowCapability::Api => &[
                    "url",
                    "body",
                    "headers",
                    "query",
                    "headersPatch",
                    "queryPatch",
                ],
                FlowCapability::Ssh => &["inputs", "workspaceDefaults"],
                FlowCapability::Database => &["sql", "catalog", "schema", "limit"],
            };
            if arguments.keys().any(|key| !allowed.contains(&key.as_str())) {
                return Err(invalid("FLOW_UNKNOWN_ARGUMENT"));
            }
            if action.capability == FlowCapability::Database {
                validate_sql_argument(
                    &action.arguments,
                    resource["driver"].as_str().unwrap_or(""),
                )?;
            }
            if action.capability == FlowCapability::Api {
                validate_api_arguments(&action.arguments, true)?;
            }
            let mut snapshot = json!({"resource": resource, "probe": probe});
            if action.capability == FlowCapability::Api {
                let saved: ApiSavedRequest = serde_json::from_value(snapshot["resource"].clone())?;
                if saved
                    .pre_request_script
                    .as_deref()
                    .is_some_and(|s| !s.trim().is_empty())
                    || saved
                        .post_response_script
                        .as_deref()
                        .is_some_and(|s| !s.trim().is_empty())
                    || saved.body_kind == MULTIPART_BODY_KIND
                {
                    return Err(invalid("FLOW_API_SCRIPT_OR_MULTIPART_UNSUPPORTED"));
                }
                if probe && !matches!(saved.method.to_uppercase().as_str(), "GET" | "HEAD") {
                    return Err(invalid("FLOW_PROBE_REQUIRES_READ_OPERATION"));
                }
                let request = ApiRequestInput {
                    workspace_id: saved.workspace_id,
                    name: Some(saved.name),
                    parent_folder_id: saved.parent_folder_id,
                    collection_id: Some(saved.collection_id),
                    auth_json: Some(saved.auth_json),
                    method: saved.method,
                    url: saved.url,
                    headers: serde_json::from_str(&saved.headers_json)?,
                    query: serde_json::from_str(&saved.query_json)?,
                    body: saved.body,
                    body_kind: saved.body_kind,
                    timeout_ms: Some(60_000),
                    pre_request_script: None,
                    post_response_script: None,
                    script_schema_version: 1,
                    temporary_variables: vec![],
                    multipart_parts: vec![],
                };
                snapshot["request"] = serde_json::to_value(request)?;
            }
            if probe && action.capability == FlowCapability::Ssh {
                return Err(invalid("FLOW_SSH_PROBE_UNSUPPORTED"));
            }
            if action.capability == FlowCapability::Ssh {
                snapshot["unresolvedAction"] = serde_json::to_value(action)?;
                let steps: Vec<SshTaskStep> =
                    serde_json::from_value(snapshot["resource"]["steps"].clone())?;
                let names = unfour_ssh_engine::SshService::detected_task_inputs(&steps)?;
                let defaults = self.flow_ssh_defaults(action, input).await?;
                ssh_inputs(&action.arguments, &names, &defaults, true)?;
                for guard in self.extensions.ssh_task_execution_guards() {
                    guard
                        .validate(&input.workspace_id, &action.resource_id)
                        .await?;
                }
            }
            // DB probes require a read-only connection; the owning engine enforces SQL safety.
            if probe
                && action.capability == FlowCapability::Database
                && snapshot["resource"]["readOnly"] != true
            {
                return Err(invalid("FLOW_PROBE_REQUIRES_READ_ONLY_CONNECTION"));
            }
            Ok(snapshot)
        })
    }

    fn execute<'a>(
        &'a self,
        action: &'a FlowAction,
        snapshot: &'a Value,
        input: &'a FlowRunInput,
        cancel: CancellationToken,
        persistence: &'a unfour_flow_engine::FlowPersistenceContext,
    ) -> FlowFuture<'a, Value> {
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(invalid("FLOW_CANCELLED"));
            }
            // Fail explicitly if a referenced resource changed after validation.
            if self.flow_resource(action, input).await? != snapshot["resource"] {
                return Err(invalid("FLOW_RESOURCE_CHANGED"));
            }
            if cancel.is_cancelled() {
                return Err(invalid("FLOW_CANCELLED"));
            }
            match action.capability {
                FlowCapability::Api => {
                    let mut request = snapshot["request"].clone();
                    apply_api_arguments(&mut request, &action.arguments)?;
                    let request: ApiRequestInput = serde_json::from_value(request)?;
                    let (request, variable_secrets) = self
                        .resolve_api_request_input_for_environment(
                            request,
                            input.environment_id.as_deref(),
                        )
                        .await?;
                    let request_secrets =
                        unfour_http_engine::runtime_request_secret_values(&request)?;
                    persistence.extend(request_secrets.iter().cloned());
                    persistence.extend(variable_secrets.iter().cloned());
                    if cancel.is_cancelled() {
                        return Err(invalid("FLOW_CANCELLED"));
                    }
                    // The runner drops this future after the grace window. Keep
                    // the request alive until then so a response that arrives
                    // during the window is recorded. Dropping it aborts the
                    // connection and leaves the remote outcome unconfirmed.
                    let response = match finish_unless_dropped(&cancel, {
                        self.api_client.send_cancellable_with_secrets(
                            request,
                            CancellationToken::new(),
                            &variable_secrets,
                        )
                    })
                    .await
                    {
                        Ok(response) => response,
                        Err(unfour_core::AppError::ApiCancelled(_)) => {
                            return Err(invalid("FLOW_OUTCOME_UNCERTAIN"));
                        }
                        Err(error) => return Err(error),
                    };
                    let body = serde_json::from_str::<Value>(&response.body)
                        .unwrap_or(Value::String(response.body));
                    if response.status >= 400 {
                        let mut details = json!({"status": response.status, "headers": response.headers, "body": body, "durationMs": response.duration_ms, "historyId": response.history_id});
                        crate::flow_authoring::scrub_diagnostics(&mut details, &request_secrets);
                        return Err(unfour_core::AppError::FlowActionFailed {
                            source: Box::new(unfour_core::AppError::HttpStatus(response.status)),
                            details: self.flow_diagnostics(details, input).await?,
                        });
                    }
                    Ok(
                        json!({"status": response.status, "headers": response.headers, "body": body, "durationMs": response.duration_ms, "historyId": response.history_id}),
                    )
                }
                FlowCapability::Database => {
                    let args = &action.arguments;
                    let query = DatabaseQueryInput {
                        workspace_id: input.workspace_id.clone(),
                        connection_id: action.resource_id.clone(),
                        sql: args["sql"]
                            .as_str()
                            .ok_or_else(|| invalid("FLOW_SQL_REQUIRED"))?
                            .into(),
                        limit: Some(args["limit"].as_u64().unwrap_or(100).min(1000) as u32),
                        confirm_mutation: Some(input.confirm_effects && snapshot["probe"] != true),
                        catalog: args["catalog"].as_str().map(str::to_owned),
                        schema: args["schema"].as_str().map(str::to_owned),
                        timeout_ms: Some(60_000),
                        enforce_read_only: false,
                    };
                    if cancel.is_cancelled() {
                        return Err(invalid("FLOW_CANCELLED"));
                    }
                    // Dropping the query does not roll it back. Wait for a
                    // result that arrives during the runner's grace window.
                    let result =
                        finish_unless_dropped(&cancel, self.execute_database_query(query)).await?;
                    Ok(serde_json::to_value(result)?)
                }
                FlowCapability::Ssh => {
                    let steps: Vec<SshTaskStep> =
                        serde_json::from_value(snapshot["resource"]["steps"].clone())?;
                    let names = unfour_ssh_engine::SshService::detected_task_inputs(&steps)?;
                    let defaults = self.flow_ssh_defaults(action, input).await?;
                    let inputs = ssh_inputs(&action.arguments, &names, &defaults, false)?;
                    let secret_input_names = self
                        .flow_ssh_secret_names(
                            &serde_json::from_value(snapshot["unresolvedAction"].clone())?,
                            input,
                            &inputs,
                        )
                        .await?;
                    if cancel.is_cancelled() {
                        return Err(invalid("FLOW_CANCELLED"));
                    }
                    let run = self
                        .run_ssh_task(SshTaskRunInput {
                            workspace_id: input.workspace_id.clone(),
                            task_id: action.resource_id.clone(),
                            connection_id: action.connection_id.clone(),
                            inputs,
                            secret_input_names,
                        })
                        .await?;
                    let mut cancel_noted = false;
                    loop {
                        let current = self
                            .list_ssh_task_runs(
                                input.workspace_id.clone(),
                                action.resource_id.clone(),
                            )
                            .await?
                            .into_iter()
                            .find(|r| r.id == run.id)
                            .ok_or_else(|| invalid("FLOW_SSH_RUN_MISSING"))?;
                        if current.status != "running" {
                            return self.flow_ssh_terminal(&current, &run.warnings, input).await;
                        }
                        tokio::select! {
                            biased;
                            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                            _ = cancel.cancelled(), if !cancel_noted => {
                                cancel_noted = true;
                                let _ = self.cancel_ssh_task_run(SshTaskCancelInput {
                                    workspace_id: input.workspace_id.clone(),
                                    run_id: run.id.clone(),
                                }).await;
                            }
                        }
                    }
                }
            }
        })
    }
}

impl CommandBus {
    async fn flow_ssh_terminal(
        &self,
        current: &SshTaskRun,
        warnings: &[String],
        input: &FlowRunInput,
    ) -> AppResult<Value> {
        let mut log = self
            .read_ssh_task_run_log(input.workspace_id.clone(), current.id.clone())
            .await?;
        let log_truncated = log.len() > 65_536;
        if log_truncated {
            let mut end = 65_536;
            while !log.is_char_boundary(end) {
                end -= 1;
            }
            log.truncate(end);
        }
        if current.status == "success" || current.status == "succeeded" {
            return Ok(json!({
                "runId": current.id,
                "status": current.status,
                "startedAt": current.started_at,
                "finishedAt": current.finished_at,
                "log": log,
                "logTruncated": log_truncated,
                "warnings": warnings,
            }));
        }
        // A cancelled task may already have executed remote commands. That is
        // not a confirmed failure and it is not safe to treat as a no-op.
        let code = if current.status == "cancelled" {
            "FLOW_OUTCOME_UNCERTAIN"
        } else {
            "FLOW_SSH_FAILED"
        };
        let details = json!({
            "runId": current.id,
            "status": current.status,
            "errorMessage": current.error_message,
            "log": log,
            "logTruncated": log_truncated,
            "warnings": warnings,
        });
        Err(unfour_core::AppError::FlowActionFailed {
            source: Box::new(invalid(code)),
            details: self.flow_diagnostics(details, input).await?,
        })
    }
}

/// Poll `work` to completion even after `cancel` fires.
///
/// `select!` would drop `work` when cancellation wins. Callers that have
/// already started a remote effect must instead keep the future until it
/// returns or the flow runner drops this task.
async fn finish_unless_dropped<F: Future>(cancel: &CancellationToken, work: F) -> F::Output {
    let mut work = std::pin::pin!(work);
    let mut noted = false;
    loop {
        tokio::select! {
            biased;
            result = &mut work => return result,
            _ = cancel.cancelled(), if !noted => {
                noted = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::finish_unless_dropped;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn result_ready_with_cancellation_is_kept() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(finish_unless_dropped(&cancel, async { 1 }).await, 1);
    }

    #[tokio::test]
    async fn result_that_arrives_after_cancellation_is_kept() {
        let cancel = CancellationToken::new();
        let flag = cancel.clone();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            finish_unless_dropped(&flag, async {
                release_rx.await.unwrap();
                "kept"
            })
            .await
        });
        tokio::task::yield_now().await;
        cancel.cancel();
        tokio::task::yield_now().await;
        release_tx.send(()).unwrap();
        assert_eq!(task.await.unwrap(), "kept");
    }
}
