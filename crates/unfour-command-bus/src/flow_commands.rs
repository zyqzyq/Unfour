use crate::flow_authoring::{apply_api_arguments, ssh_inputs, validate_sql_argument};
use crate::CommandBus;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
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
        for step in &input.steps {
            let action = match &step.node {
                FlowNode::Action { action } => action,
                FlowNode::Poll { probe, .. } | FlowNode::WaitUntil { probe, .. } => probe,
                _ => continue,
            };
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
                for (key, patch) in [("headers", "headersPatch"), ("query", "queryPatch")] {
                    if arguments.contains_key(key) && arguments.contains_key(patch) {
                        return Err(invalid("FLOW_API_REPLACE_PATCH_CONFLICT"));
                    }
                }
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
                let resolved = self
                    .resolve_api_request_input_for_environment(
                        request,
                        input.environment_id.as_deref(),
                    )
                    .await?;
                snapshot["request"] =
                    serde_json::to_value(self.api_client.materialize_auth(resolved)?)?;
            }
            if probe && action.capability == FlowCapability::Ssh {
                return Err(invalid("FLOW_SSH_PROBE_UNSUPPORTED"));
            }
            if action.capability == FlowCapability::Ssh {
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
                    let response = self.api_client.send_cancellable(request, cancel).await?;
                    let body = serde_json::from_str::<Value>(&response.body)
                        .unwrap_or(Value::String(response.body));
                    if response.status >= 400 {
                        return Err(unfour_core::AppError::HttpStatus(response.status));
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
                    };
                    tokio::select! { _ = cancel.cancelled() => Err(invalid("FLOW_CANCELLED")), result = self.execute_database_query(query) => Ok(serde_json::to_value(result?)?) }
                }
                FlowCapability::Ssh => {
                    let steps: Vec<SshTaskStep> =
                        serde_json::from_value(snapshot["resource"]["steps"].clone())?;
                    let names = unfour_ssh_engine::SshService::detected_task_inputs(&steps)?;
                    let defaults = self.flow_ssh_defaults(action, input).await?;
                    let inputs = ssh_inputs(&action.arguments, &names, &defaults, false)?;
                    let secret_input_names = inputs.keys().cloned().collect();
                    let run = self
                        .run_ssh_task(SshTaskRunInput {
                            workspace_id: input.workspace_id.clone(),
                            task_id: action.resource_id.clone(),
                            connection_id: action.connection_id.clone(),
                            inputs,
                            secret_input_names,
                        })
                        .await?;
                    loop {
                        tokio::select! {
                            _ = cancel.cancelled() => { let _ = self.cancel_ssh_task_run(SshTaskCancelInput { workspace_id: input.workspace_id.clone(), run_id: run.id.clone() }).await; return Err(invalid("FLOW_CANCELLED")); }
                            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                        }
                        let current = self
                            .list_ssh_task_runs(
                                input.workspace_id.clone(),
                                action.resource_id.clone(),
                            )
                            .await?
                            .into_iter()
                            .find(|r| r.id == run.id)
                            .ok_or_else(|| invalid("FLOW_SSH_RUN_MISSING"))?;
                        if current.status == "running" {
                            continue;
                        }
                        if current.status != "success" && current.status != "succeeded" {
                            return Err(invalid("FLOW_SSH_FAILED"));
                        }
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
                        return Ok(
                            json!({"runId": current.id, "status": current.status, "startedAt": current.started_at, "finishedAt": current.finished_at, "log": log, "logTruncated": log_truncated}),
                        );
                    }
                }
            }
        })
    }
}
