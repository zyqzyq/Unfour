use super::*;

pub(super) fn api_list_collections(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId"])?;
    let workspace_id = parse_optional_string(&arguments, "workspaceId")?;

    let result = command_bus
        .execute_read(ReadCommand::ApiListCollections { workspace_id })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    let ReadCommandResult::ApiCollections(collections) = result else {
        return Err(unexpected_result());
    };

    serialize_safe(collections)
}

pub(super) fn api_list_requests(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "collectionId"])?;
    let workspace_id = parse_optional_string(&arguments, "workspaceId")?;
    let collection_id = parse_optional_string(&arguments, "collectionId")?;

    let result = command_bus
        .execute_read(ReadCommand::ApiListRequests {
            workspace_id,
            collection_id,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    let ReadCommandResult::ApiRequests(mut requests) = result else {
        return Err(unexpected_result());
    };

    // Redact sensitive URL query params in urlPreview
    for request in &mut requests.requests {
        request.url_preview = redact_url_query(&request.url_preview);
    }

    serialize_safe(requests)
}

pub(super) fn api_get_request(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["requestId", "includeBody"])?;
    let request_id = parse_required_string(&arguments, "requestId", "unfour.api.get_request")?;
    let include_body = parse_optional_bool(&arguments, "includeBody")?.unwrap_or(true);

    let result = command_bus
        .execute_read(ReadCommand::ApiGetRequest { request_id })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    let ReadCommandResult::ApiRequest(detail) = result else {
        return Err(unexpected_result());
    };

    let saved = detail.request;

    // Parse and redact headers
    let headers: Vec<Value> = serde_json::from_str::<Vec<Value>>(&saved.headers_json)
        .unwrap_or_default()
        .into_iter()
        .map(mask_key_value_entry)
        .collect();

    // Parse and redact query params
    let query: Vec<Value> = serde_json::from_str::<Vec<Value>>(&saved.query_json)
        .unwrap_or_default()
        .into_iter()
        .map(mask_key_value_entry)
        .collect();

    // Redact and truncate body
    let (body_preview, body_truncated) = if include_body {
        let raw_body = saved.body.as_deref().unwrap_or("");
        let redacted = redact_body(raw_body, &saved.body_kind);
        truncate_body(&redacted, MAX_BODY_PREVIEW_BYTES)
    } else {
        (String::new(), false)
    };

    // Redact URL query params
    let url = redact_url_query(&saved.url);

    let request = json!({
        "id": saved.id,
        "name": saved.name,
        "method": saved.method,
        "url": url,
        "headers": headers,
        "query": query,
        "bodyPreview": body_preview,
        "bodyType": saved.body_kind,
        "truncated": body_truncated,
        "workspaceId": saved.workspace_id,
        "collectionId": saved.collection_id
    });

    Ok(json!({
        "request": request,
        "source": "command-bus"
    }))
}

pub(super) fn api_send_request(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "requestId",
            "workspaceId",
            "name",
            "method",
            "url",
            "headers",
            "query",
            "body",
            "bodyKind",
            "environmentId",
            "timeoutMs",
        ],
    )?;
    let timeout_ms = parse_optional_timeout(&arguments)?;
    let environment_id = parse_optional_string(&arguments, "environmentId")?;

    let response = if let Some(request_id) = parse_optional_string(&arguments, "requestId")? {
        let workspace_id = parse_optional_string(&arguments, "workspaceId")?;
        command_bus.execute_saved_api_request_with_scripts_in_workspace(
            workspace_id.as_deref(),
            &request_id,
            timeout_ms,
            environment_id.as_deref(),
        )
    } else {
        let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
        let method = parse_required_string(&arguments, "method", "unfour.api.send_request")?;
        let url = parse_required_string(&arguments, "url", "unfour.api.send_request")?;
        let input = ApiRequestInput {
            workspace_id,
            name: parse_optional_string(&arguments, "name")?,
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method,
            url,
            headers: parse_key_values(arguments.get("headers"))?,
            query: parse_key_values(arguments.get("query"))?,
            body: parse_optional_body(&arguments, None)?,
            body_kind: parse_optional_string(&arguments, "bodyKind")?
                .unwrap_or_else(|| "json".to_string()),
            timeout_ms,
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            temporary_variables: vec![],
        };
        command_bus.send_api_request_in_environment(input, environment_id.as_deref())
    };

    match response {
        Ok(response) => {
            let body_raw = response.body;
            let body_type = guess_body_type(&body_raw);
            let redacted_body = redact_body(&body_raw, &body_type);
            let (body_preview, truncated) = truncate_body(&redacted_body, MAX_BODY_PREVIEW_BYTES);
            let size_bytes = body_raw.len();

            let headers: Vec<Value> = response
                .headers
                .into_iter()
                .map(|kv| {
                    let value = redact_header_value(&kv.key, &kv.value);
                    json!({
                        "name": kv.key,
                        "value": value
                    })
                })
                .collect();

            Ok(json!({
                "ok": true,
                "status": response.status,
                "statusText": response.status_text,
                "durationMs": response.duration_ms,
                "headers": headers,
                "bodyPreview": body_preview,
                "bodyType": body_type,
                "sizeBytes": size_bytes,
                "truncated": truncated,
                "source": "command-bus"
            }))
        }
        Err(error) => Err(ToolCallError::Execution {
            code: error.code,
            message: error.message,
        }),
    }
}

pub(super) fn api_create_request(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "collectionId",
            "parentId",
            "parentFolderId",
            "name",
            "method",
            "url",
            "headers",
            "query",
            "body",
            "bodyKind",
            "auth",
            "authJson",
        ],
    )?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let input = ApiRequestInput {
        workspace_id,
        name: Some(parse_required_string(
            &arguments,
            "name",
            "unfour.api.create_request",
        )?),
        parent_folder_id: parse_parent_folder_id(&arguments)?,
        collection_id: parse_optional_string(&arguments, "collectionId")?,
        auth_json: parse_auth_json(&arguments)?,
        method: parse_required_string(&arguments, "method", "unfour.api.create_request")?,
        url: parse_required_string(&arguments, "url", "unfour.api.create_request")?,
        headers: parse_key_values(arguments.get("headers"))?,
        query: parse_key_values(arguments.get("query"))?,
        body: parse_optional_body(&arguments, None)?,
        body_kind: parse_optional_string(&arguments, "bodyKind")?
            .unwrap_or_else(|| "json".to_string()),
        timeout_ms: None,
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        temporary_variables: vec![],
    };

    let saved = command_bus
        .save_api_request(input)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "request": safe_request_summary(&saved),
        "source": "command-bus"
    }))
}

pub(super) fn api_update_request(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "requestId",
            "workspaceId",
            "collectionId",
            "parentId",
            "parentFolderId",
            "name",
            "method",
            "url",
            "headers",
            "query",
            "body",
            "bodyKind",
            "auth",
            "authJson",
        ],
    )?;
    let request_id = parse_required_string(&arguments, "requestId", "unfour.api.update_request")?;
    let existing = get_saved_request(command_bus, &request_id)?;
    let workspace_id = parse_optional_string(&arguments, "workspaceId")?
        .unwrap_or_else(|| existing.workspace_id.clone());
    let input = ApiRequestInput {
        workspace_id: workspace_id.clone(),
        name: parse_optional_string(&arguments, "name")?.or(Some(existing.name.clone())),
        parent_folder_id: parse_parent_folder_id(&arguments)?.or(existing.parent_folder_id.clone()),
        collection_id: parse_optional_string(&arguments, "collectionId")?
            .or(Some(existing.collection_id.clone())),
        auth_json: parse_auth_json(&arguments)?.or(Some(existing.auth_json.clone())),
        method: parse_optional_string(&arguments, "method")?.unwrap_or(existing.method.clone()),
        url: parse_optional_string(&arguments, "url")?.unwrap_or(existing.url.clone()),
        headers: match arguments.get("headers") {
            Some(value) => parse_key_values(Some(value))?,
            None => serde_json::from_str(&existing.headers_json).unwrap_or_default(),
        },
        query: match arguments.get("query") {
            Some(value) => parse_key_values(Some(value))?,
            None => serde_json::from_str(&existing.query_json).unwrap_or_default(),
        },
        body: parse_optional_body(&arguments, existing.body.clone())?,
        body_kind: parse_optional_string(&arguments, "bodyKind")?
            .unwrap_or(existing.body_kind.clone()),
        timeout_ms: None,
        pre_request_script: existing.pre_request_script.clone(),
        post_response_script: existing.post_response_script.clone(),
        script_schema_version: existing.script_schema_version,
        temporary_variables: vec![],
    };

    let saved = command_bus
        .update_api_request(&workspace_id, &request_id, input)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "request": safe_request_summary(&saved),
        "source": "command-bus"
    }))
}

pub(super) fn api_delete_request(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "requestId",
            "workspaceId",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let request_id = parse_required_string(&arguments, "requestId", "unfour.api.delete_request")?;
    let workspace_id = match parse_optional_string(&arguments, "workspaceId")? {
        Some(workspace_id) => workspace_id,
        None => get_saved_request(command_bus, &request_id)?.workspace_id,
    };
    ensure_confirmed_if_guarded(evaluation,
        &arguments,
        "API_DELETE_REQUEST",
        "Deleting an API request hides local request metadata. This operation is soft-delete but still requires confirmation.",
        json!({
            "tool": "unfour.api.delete_request",
            "workspaceId": workspace_id,
            "requestId": request_id
        }),
    )?;

    let remaining = command_bus
        .delete_api_request(&workspace_id, &request_id)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "softDelete": true,
        "deletedRequestId": request_id,
        "remainingCount": remaining.len(),
        "source": "command-bus"
    }))
}

pub(super) fn api_create_collection(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "name"])?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let name = parse_required_string(&arguments, "name", "unfour.api.create_collection")?;
    let collection = command_bus
        .create_api_collection(&workspace_id, &name)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "collection": collection,
        "source": "command-bus"
    }))
}

pub(super) fn api_update_collection(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "collectionId", "name"])?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let collection_id =
        parse_required_string(&arguments, "collectionId", "unfour.api.update_collection")?;
    let name = parse_required_string(&arguments, "name", "unfour.api.update_collection")?;
    let collection = command_bus
        .update_api_collection(&workspace_id, &collection_id, &name)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "collection": collection,
        "source": "command-bus"
    }))
}

pub(super) fn api_delete_collection(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "collectionId",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let collection_id =
        parse_required_string(&arguments, "collectionId", "unfour.api.delete_collection")?;
    ensure_confirmed_if_guarded(evaluation,
        &arguments,
        "API_DELETE_COLLECTION",
        "Deleting an API collection cascades soft-delete to local folders and requests. Confirmation is required.",
        json!({
            "tool": "unfour.api.delete_collection",
            "workspaceId": workspace_id,
            "collectionId": collection_id
        }),
    )?;
    let remaining = command_bus
        .delete_api_collection(&workspace_id, &collection_id)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "softDelete": true,
        "deletedCollectionId": collection_id,
        "remainingCount": remaining.len(),
        "source": "command-bus"
    }))
}

pub(super) fn api_list_history(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "limit"])?;
    let workspace_id = parse_optional_string(&arguments, "workspaceId")?;
    let limit = parse_optional_history_limit(&arguments)?;

    let result = command_bus
        .execute_read(ReadCommand::ApiListHistory {
            workspace_id,
            limit,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    let ReadCommandResult::ApiHistory(history) = result else {
        return Err(unexpected_result());
    };

    let items: Vec<Value> = history
        .history
        .iter()
        .map(|item| {
            json!({
                "id": item.id,
                "name": item.name,
                "method": item.method,
                "url": redact_url_query(&item.url),
                "status": item.status,
                "durationMs": item.duration_ms,
                "createdAt": item.created_at
            })
        })
        .collect();

    Ok(json!({
        "history": items,
        "count": history.count,
        "source": "command-bus"
    }))
}

pub(super) fn api_get_history(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["historyId", "workspaceId"])?;
    let history_id = parse_required_string(&arguments, "historyId", "unfour.api.get_history")?;
    let workspace_id = parse_optional_string(&arguments, "workspaceId")?;

    let result = command_bus
        .execute_read(ReadCommand::ApiGetHistory {
            workspace_id,
            history_id,
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    let ReadCommandResult::ApiHistoryDetailResult(detail) = result else {
        return Err(unexpected_result());
    };
    let detail = detail.detail;

    let request_headers = mask_kv_json_array(&detail.request_headers_json);
    let request_query = mask_kv_json_array(&detail.request_query_json);
    let response_headers = mask_kv_json_array(&detail.response_headers_json);

    let (request_body, request_body_truncated) =
        redact_and_truncate(detail.request_body.as_deref().unwrap_or(""));
    let (response_body_preview, response_body_truncated) =
        redact_and_truncate(detail.response_body_preview.as_deref().unwrap_or(""));

    Ok(json!({
        "history": {
            "id": detail.id,
            "name": detail.name,
            "method": detail.method,
            "url": redact_url_query(&detail.url),
            "status": detail.status,
            "durationMs": detail.duration_ms,
            "requestHeaders": request_headers,
            "requestQuery": request_query,
            "requestBody": request_body,
            "requestBodyTruncated": request_body_truncated,
            "responseHeaders": response_headers,
            "responseBodyPreview": response_body_preview,
            "responseBodyTruncated": response_body_truncated,
            "createdAt": detail.created_at
        },
        "source": "command-bus"
    }))
}

pub(super) fn api_list_environments(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId"])?;
    let workspace_id = parse_optional_string(&arguments, "workspaceId")?;

    let result = command_bus
        .execute_read(ReadCommand::ApiListEnvironments { workspace_id })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;

    let ReadCommandResult::ApiEnvironments(environments) = result else {
        return Err(unexpected_result());
    };

    let environments: Vec<Value> = environments
        .environments
        .iter()
        .map(safe_environment)
        .collect();

    Ok(json!({
        "environments": environments,
        "count": environments.len(),
        "source": "command-bus"
    }))
}

pub(super) fn api_create_environment(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(arguments, &["workspaceId", "name"])?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let name = parse_required_string(&arguments, "name", "unfour.api.create_environment")?;
    let environment = command_bus
        .create_api_environment(&workspace_id, &name)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "apiEnvironment": safe_environment(&environment),
        "source": "command-bus"
    }))
}

pub(super) fn api_update_environment(
    command_bus: &dyn CommandBusAdapter,
    _evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &["workspaceId", "environmentId", "name", "variables"],
    )?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let environment_id =
        parse_required_string(&arguments, "environmentId", "unfour.api.update_environment")?;
    let name = parse_required_string(&arguments, "name", "unfour.api.update_environment")?;
    let variables = parse_environment_variables(arguments.get("variables"))?;
    let environment = command_bus
        .update_api_environment(&workspace_id, &environment_id, &name, variables)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "apiEnvironment": safe_environment(&environment),
        "source": "command-bus"
    }))
}

pub(super) fn api_delete_environment(
    command_bus: &dyn CommandBusAdapter,
    evaluation: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments = object_with_allowed_keys(
        arguments,
        &[
            "workspaceId",
            "environmentId",
            "confirm",
            "confirmationText",
            "confirmation_text",
        ],
    )?;
    let workspace_id = resolve_workspace_id(command_bus, &arguments)?;
    let environment_id =
        parse_required_string(&arguments, "environmentId", "unfour.api.delete_environment")?;
    ensure_confirmed_if_guarded(
        evaluation,
        &arguments,
        "API_DELETE_ENVIRONMENT",
        "Deleting an API environment removes its local variables. Confirmation is required.",
        json!({
            "tool": "unfour.api.delete_environment",
            "workspaceId": workspace_id,
            "environmentId": environment_id
        }),
    )?;
    let remaining = command_bus
        .delete_api_environment(&workspace_id, &environment_id)
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    Ok(json!({
        "softDelete": true,
        "deletedEnvironmentId": environment_id,
        "remainingCount": remaining.len(),
        "source": "command-bus"
    }))
}

fn parse_environment_variables(value: Option<&Value>) -> Result<Vec<KeyValue>, ToolCallError> {
    let Some(Value::Array(_)) = value else {
        return Err(ToolCallError::InvalidArguments(
            "argument `variables` must be an array of { key, value, enabled } objects".to_string(),
        ));
    };
    serde_json::from_value::<Vec<KeyValue>>(value.cloned().unwrap_or_default()).map_err(|_| {
        ToolCallError::InvalidArguments(
            "argument `variables` must be an array of { key, value, enabled } objects".to_string(),
        )
    })
}

fn safe_environment(environment: &unfour_core::models::ApiEnvironment) -> Value {
    let variables: Vec<Value> = environment
        .variables
        .iter()
        .map(|variable| {
            let value = if is_sensitive_key(&variable.key) {
                mask_secret(&variable.value)
            } else {
                variable.value.clone()
            };
            json!({ "key": variable.key, "value": value, "enabled": variable.enabled })
        })
        .collect();
    json!({
        "id": environment.id,
        "name": environment.name,
        "isActive": environment.is_active,
        "variableCount": environment.variables.len(),
        "variables": variables,
        "workspaceId": environment.workspace_id
    })
}

// --- Helpers ---

/// Parse a JSON array of `{ key, value }` entries and mask sensitive values.
pub(super) fn mask_kv_json_array(raw: &str) -> Vec<Value> {
    serde_json::from_str::<Vec<Value>>(raw)
        .unwrap_or_default()
        .into_iter()
        .map(mask_key_value_entry)
        .collect()
}

/// Redact sensitive fields from a body string and truncate to the preview limit.
pub(super) fn redact_and_truncate(raw: &str) -> (String, bool) {
    let body_type = guess_body_type(raw);
    let redacted = redact_body(raw, &body_type);
    truncate_body(&redacted, MAX_BODY_PREVIEW_BYTES)
}

/// Mask the `value` of a `{ "key": ..., "value": ... }` entry when its key is
/// sensitive, preserving the entry's other fields.
pub(super) fn mask_key_value_entry(mut entry: Value) -> Value {
    let name = entry
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if is_sensitive_key(&name) {
        if let Some(obj) = entry.as_object_mut() {
            let current = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let masked = mask_secret(current);
            obj.insert("value".to_string(), Value::String(masked));
        }
    }
    entry
}
