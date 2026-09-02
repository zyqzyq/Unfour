use std::collections::{HashMap, HashSet};

use unfour_core::models::{
    ApiRequestInput, RequestExecutionResult, ScriptExecutionResult, ScriptExecutionStatus,
    WorkspaceEnvironment, WorkspaceVariableInput,
};
use unfour_core::redaction::is_sensitive_key;
use unfour_core::AppError;
use unfour_http_engine::{
    run_script, validate_script_config, ScriptPhase, ScriptRequestContext, ScriptResponseContext,
    ScriptRunInput, ScriptVariable,
};

use super::*;

impl CommandBus {
    /// Versioned API Client execution path that preserves the existing MCP and
    /// adapter contract while returning HTTP, script, console, and test data as
    /// one typed result to the desktop request editor.
    pub async fn send_api_request_with_scripts(
        &self,
        input: ApiRequestInput,
    ) -> AppResult<RequestExecutionResult> {
        self.send_api_request_with_scripts_in_environment(input, None)
            .await
    }

    pub async fn send_api_request_with_scripts_in_environment(
        &self,
        input: ApiRequestInput,
        environment_id_override: Option<String>,
    ) -> AppResult<RequestExecutionResult> {
        validate_script_config(
            input.pre_request_script.as_deref(),
            input.post_response_script.as_deref(),
            input.script_schema_version,
        )?;

        let environment_id = self
            .resolve_api_environment_id(&input.workspace_id, environment_id_override.as_deref())
            .await?;
        let pre_environment = self
            .script_environment(&input.workspace_id, environment_id.as_deref())
            .await?;
        let pre_input = script_input(
            ScriptPhase::PreRequest,
            &input,
            pre_environment.as_ref(),
            input
                .temporary_variables
                .iter()
                .map(script_variable_from_key_value)
                .collect(),
            None,
        );
        let pre = run_script(
            input.pre_request_script.clone().unwrap_or_default(),
            pre_input,
        )
        .await?;

        if matches!(
            pre.execution.status,
            ScriptExecutionStatus::Failed | ScriptExecutionStatus::Timeout
        ) {
            return Ok(RequestExecutionResult {
                response: None,
                http_error: None,
                pre_request: pre.execution,
                post_response: ScriptExecutionResult::skipped(),
            });
        }
        if pre.environment_changed {
            self.commit_script_environment(
                &input.workspace_id,
                pre_environment.as_ref(),
                &pre.environment,
            )
            .await?;
        }

        let mut request = input.clone();
        request.method = pre.request.method;
        request.url = pre.request.url;
        request.headers = pre.request.headers;
        request.body = pre.request.body_raw;
        request.temporary_variables = pre
            .variables
            .iter()
            .map(key_value_from_script_variable)
            .collect();
        let resolved_request = self
            .resolve_api_request_input_for_environment(request, environment_id.as_deref())
            .await?;
        let response = match self.api_client.send(resolved_request.clone()).await {
            Ok(response) => response,
            Err(error) => {
                return Ok(RequestExecutionResult {
                    response: None,
                    http_error: Some(error.to_string()),
                    pre_request: pre.execution,
                    post_response: ScriptExecutionResult::skipped(),
                });
            }
        };

        let post_environment = self
            .script_environment(&input.workspace_id, environment_id.as_deref())
            .await?;
        let post_input = script_input(
            ScriptPhase::PostResponse,
            &resolved_request,
            post_environment.as_ref(),
            pre.variables,
            Some(ScriptResponseContext::new(
                response.status,
                response.status_text.clone(),
                response.duration_ms,
                response.headers.clone(),
                &response.body,
            )),
        );
        let post = run_script(
            input.post_response_script.clone().unwrap_or_default(),
            post_input,
        )
        .await?;
        if post.execution.status == ScriptExecutionStatus::Success && post.environment_changed {
            self.commit_script_environment(
                &input.workspace_id,
                post_environment.as_ref(),
                &post.environment,
            )
            .await?;
        }

        self.activity_log
            .record(
                Some(&input.workspace_id),
                "api.send_request",
                Some(&response.history_id),
                serde_json::json!({
                    "method": input.method,
                    "url": input.url,
                    "status": response.status,
                    "preScriptStatus": pre.execution.status,
                    "postScriptStatus": post.execution.status,
                }),
            )
            .await?;

        Ok(RequestExecutionResult {
            response: Some(response),
            http_error: None,
            pre_request: pre.execution,
            post_response: post.execution,
        })
    }

    async fn script_environment(
        &self,
        workspace_id: &str,
        environment_id: Option<&str>,
    ) -> AppResult<Option<WorkspaceEnvironment>> {
        let environments = self
            .workspace
            .list_environments(workspace_id.to_string())
            .await?;
        match environment_id {
            Some(environment_id) => environments
                .into_iter()
                .find(|environment| environment.id == environment_id)
                .map(Some)
                .ok_or_else(|| AppError::NotFound("workspace environment".to_string())),
            None => Ok(None),
        }
    }

    async fn commit_script_environment(
        &self,
        workspace_id: &str,
        active_environment: Option<&WorkspaceEnvironment>,
        output: &[ScriptVariable],
    ) -> AppResult<()> {
        let environment = active_environment.ok_or_else(|| {
            AppError::Validation("no active workspace environment is selected".to_string())
        })?;
        let existing = environment
            .variables
            .iter()
            .map(|variable| (variable.key.as_str(), variable))
            .collect::<HashMap<_, _>>();
        let variables = output
            .iter()
            .enumerate()
            .map(|(index, variable)| {
                let current = existing.get(variable.key.as_str()).copied();
                WorkspaceVariableInput {
                    id: current.map(|item| item.id.clone()),
                    key: variable.key.clone(),
                    value: variable.value.clone(),
                    is_secret: current
                        .map(|item| item.is_secret)
                        .unwrap_or_else(|| is_sensitive_key(&variable.key)),
                    is_enabled: variable.enabled,
                    description: current.and_then(|item| item.description.clone()),
                    sort_order: i64::try_from(index).unwrap_or(i64::MAX),
                }
            })
            .collect();
        self.workspace_environment_variables_replace(
            workspace_id.to_string(),
            environment.id.clone(),
            variables,
        )
        .await?;
        Ok(())
    }
}

fn script_input(
    phase: ScriptPhase,
    input: &ApiRequestInput,
    active_environment: Option<&WorkspaceEnvironment>,
    variables: Vec<ScriptVariable>,
    response: Option<ScriptResponseContext>,
) -> ScriptRunInput {
    let environment = active_environment
        .map(|environment| {
            environment
                .variables
                .iter()
                .map(|variable| ScriptVariable {
                    key: variable.key.clone(),
                    value: variable.value.clone(),
                    enabled: variable.is_enabled,
                })
                .collect()
        })
        .unwrap_or_default();
    let mut redactions = HashSet::new();
    if let Some(environment) = active_environment {
        for variable in &environment.variables {
            if variable.is_secret || is_sensitive_key(&variable.key) {
                insert_redaction(&mut redactions, &variable.value);
            }
        }
    }
    for variable in &variables {
        if is_sensitive_key(&variable.key) {
            insert_redaction(&mut redactions, &variable.value);
        }
    }
    for header in &input.headers {
        if is_sensitive_key(&header.key) || header.key.eq_ignore_ascii_case("set-cookie") {
            insert_redaction(&mut redactions, &header.value);
        }
    }
    if let Some(response) = &response {
        for header in &response.headers {
            if is_sensitive_key(&header.key) || header.key.eq_ignore_ascii_case("set-cookie") {
                insert_redaction(&mut redactions, &header.value);
            }
        }
    }

    ScriptRunInput {
        phase,
        request: ScriptRequestContext {
            method: input.method.clone(),
            url: input.url.clone(),
            headers: input.headers.clone(),
            body_raw: input.body.clone(),
        },
        response,
        environment,
        variables,
        has_environment: active_environment.is_some(),
        redactions: redactions.into_iter().collect(),
    }
}

fn insert_redaction(redactions: &mut HashSet<String>, value: &str) {
    if !value.is_empty() {
        redactions.insert(value.to_string());
    }
}

fn script_variable_from_key_value(value: &KeyValue) -> ScriptVariable {
    ScriptVariable {
        key: value.key.clone(),
        value: value.value.clone(),
        enabled: value.enabled,
    }
}

fn key_value_from_script_variable(value: &ScriptVariable) -> KeyValue {
    KeyValue {
        key: value.key.clone(),
        value: value.value.clone(),
        enabled: value.enabled,
    }
}
