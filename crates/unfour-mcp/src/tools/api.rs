use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::sanitize::{
    is_sensitive_key, mask_secret, redact_body, redact_header_value, redact_url_query,
    truncate_body, MAX_BODY_PREVIEW_BYTES,
};

use super::{
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
};

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.list_collections",
                title: "List API Collections",
                description:
                    "Lists API request collections for the active workspace through the Unfour command bus.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        }
                    },
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "collections": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "name": { "type": "string" },
                                    "requestCount": { "type": "integer", "minimum": 0 },
                                    "workspaceId": { "type": "string" }
                                },
                                "required": ["id", "name", "requestCount", "workspaceId"],
                                "additionalProperties": false
                            }
                        },
                        "count": { "type": "integer", "minimum": 0 },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["collections", "count", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: api_list_collections,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.list_requests",
                title: "List API Requests",
                description:
                    "Lists saved API requests for the active workspace through the Unfour command bus. Sensitive URL parameters are redacted.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        },
                        "collectionId": {
                            "type": "string",
                            "description": "Optional collection ID filter."
                        }
                    },
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "requests": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "name": { "type": "string" },
                                    "method": { "type": "string" },
                                    "urlPreview": { "type": "string" },
                                    "collectionId": { "type": "string" },
                                    "workspaceId": { "type": "string" },
                                    "hasBody": { "type": "boolean" },
                                    "headerCount": { "type": "integer", "minimum": 0 }
                                },
                                "required": ["id", "name", "method", "urlPreview", "collectionId", "workspaceId", "hasBody", "headerCount"],
                                "additionalProperties": false
                            }
                        },
                        "count": { "type": "integer", "minimum": 0 },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["requests", "count", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: api_list_requests,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.get_request",
                title: "Get API Request Detail",
                description:
                    "Returns a saved API request with sensitive headers, query parameters, body fields, and URL parameters redacted through the Unfour command bus.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "requestId": {
                            "type": "string",
                            "description": "The saved API request ID."
                        },
                        "includeBody": {
                            "type": "boolean",
                            "description": "Whether to include the request body preview. Defaults to true."
                        }
                    },
                    "required": ["requestId"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "request": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "name": { "type": "string" },
                                "method": { "type": "string" },
                                "url": { "type": "string" },
                                "headers": { "type": "array" },
                                "query": { "type": "array" },
                                "bodyPreview": { "type": "string" },
                                "bodyType": { "type": "string" },
                                "truncated": { "type": "boolean" },
                                "workspaceId": { "type": "string" },
                                "collectionId": { "type": "string" }
                            },
                            "required": ["id", "name", "method", "url", "headers", "query", "bodyPreview", "bodyType", "truncated", "workspaceId", "collectionId"],
                            "additionalProperties": false
                        },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["request", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: api_get_request,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.send_request",
                title: "Send Saved API Request",
                description:
                    "Sends a previously saved API request through the Unfour command bus and returns the response summary with sensitive data redacted.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "requestId": {
                            "type": "string",
                            "description": "The saved API request ID to send."
                        },
                        "environmentId": {
                            "type": "string",
                            "description": "Optional environment ID (currently uses the workspace default environment)."
                        },
                        "timeoutMs": {
                            "type": "number",
                            "description": "Optional timeout in milliseconds. Maximum 60000ms (60 seconds)."
                        }
                    },
                    "required": ["requestId"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "status": { "type": "integer" },
                        "statusText": { "type": "string" },
                        "durationMs": { "type": "integer" },
                        "headers": { "type": "array" },
                        "bodyPreview": { "type": "string" },
                        "bodyType": { "type": "string" },
                        "sizeBytes": { "type": "integer" },
                        "truncated": { "type": "boolean" },
                        "error": {
                            "type": "object",
                            "properties": {
                                "code": { "type": "string" },
                                "message": { "type": "string" },
                                "safeDetail": { "type": "string" }
                            }
                        },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["ok", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::remote_action(),
            },
            handler: api_send_request,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.list_history",
                title: "List API Request History",
                description:
                    "Lists recent API request/response history for the active workspace through the Unfour command bus. Sensitive URL parameters are masked. Useful for diagnosing when a request started failing.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of history entries to return (default 50, max 200)."
                        }
                    },
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "history": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "name": { "type": ["string", "null"] },
                                    "method": { "type": "string" },
                                    "url": { "type": "string" },
                                    "status": { "type": ["integer", "null"] },
                                    "durationMs": { "type": ["integer", "null"] },
                                    "createdAt": { "type": "string" }
                                },
                                "required": ["id", "method", "url", "createdAt"],
                                "additionalProperties": false
                            }
                        },
                        "count": { "type": "integer", "minimum": 0 },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["history", "count", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: api_list_history,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.get_history",
                title: "Get API Request History Detail",
                description:
                    "Returns a single API history entry with request and response detail through the Unfour command bus. Sensitive headers, query parameters, and body fields are masked.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "historyId": {
                            "type": "string",
                            "description": "The API history entry ID."
                        },
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        }
                    },
                    "required": ["historyId"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "history": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "name": { "type": ["string", "null"] },
                                "method": { "type": "string" },
                                "url": { "type": "string" },
                                "status": { "type": ["integer", "null"] },
                                "durationMs": { "type": ["integer", "null"] },
                                "requestHeaders": { "type": "array" },
                                "requestQuery": { "type": "array" },
                                "requestBody": { "type": "string" },
                                "requestBodyTruncated": { "type": "boolean" },
                                "responseHeaders": { "type": "array" },
                                "responseBodyPreview": { "type": "string" },
                                "responseBodyTruncated": { "type": "boolean" },
                                "createdAt": { "type": "string" }
                            },
                            "required": ["id", "method", "url", "createdAt"],
                            "additionalProperties": false
                        },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["history", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: api_get_history,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.list_environments",
                title: "List API Environments",
                description:
                    "Lists API environments and their variables for the active workspace through the Unfour command bus. Sensitive variable values are masked; non-sensitive values (e.g. base URLs) are shown so requests using variables can be understood.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID. Uses the active workspace if omitted."
                        }
                    },
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "environments": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "name": { "type": "string" },
                                    "isActive": { "type": "boolean" },
                                    "variableCount": { "type": "integer", "minimum": 0 },
                                    "variables": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "key": { "type": "string" },
                                                "value": { "type": "string" },
                                                "enabled": { "type": "boolean" }
                                            },
                                            "required": ["key", "value", "enabled"],
                                            "additionalProperties": false
                                        }
                                    },
                                    "workspaceId": { "type": "string" }
                                },
                                "required": ["id", "name", "isActive", "variableCount", "variables", "workspaceId"],
                                "additionalProperties": false
                            }
                        },
                        "count": { "type": "integer", "minimum": 0 },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["environments", "count", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::local_read(),
            },
            handler: api_list_environments,
        },
    ]
}

fn api_list_collections(
    command_bus: &dyn CommandBusAdapter,
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

fn api_list_requests(
    command_bus: &dyn CommandBusAdapter,
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

fn api_get_request(
    command_bus: &dyn CommandBusAdapter,
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
        "collectionId": saved.collection_id.unwrap_or_default()
    });

    Ok(json!({
        "request": request,
        "source": "command-bus"
    }))
}

fn api_send_request(
    command_bus: &dyn CommandBusAdapter,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let arguments =
        object_with_allowed_keys(arguments, &["requestId", "environmentId", "timeoutMs"])?;
    let request_id = parse_required_string(&arguments, "requestId", "unfour.api.send_request")?;
    let timeout_ms = parse_optional_timeout(&arguments)?;

    match command_bus.execute_saved_api_request(&request_id, timeout_ms) {
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

fn api_list_history(
    command_bus: &dyn CommandBusAdapter,
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

fn api_get_history(
    command_bus: &dyn CommandBusAdapter,
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

fn api_list_environments(
    command_bus: &dyn CommandBusAdapter,
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
        .map(|env| {
            let variables: Vec<Value> = env
                .variables
                .iter()
                .map(|kv| {
                    let value = if is_sensitive_key(&kv.key) {
                        mask_secret(&kv.value)
                    } else {
                        kv.value.clone()
                    };
                    json!({ "key": kv.key, "value": value, "enabled": kv.enabled })
                })
                .collect();
            json!({
                "id": env.id,
                "name": env.name,
                "isActive": env.is_active,
                "variableCount": env.variables.len(),
                "variables": variables,
                "workspaceId": env.workspace_id
            })
        })
        .collect();

    Ok(json!({
        "environments": environments,
        "count": environments.len(),
        "source": "command-bus"
    }))
}

// --- Helpers ---

/// Parse a JSON array of `{ key, value }` entries and mask sensitive values.
fn mask_kv_json_array(raw: &str) -> Vec<Value> {
    serde_json::from_str::<Vec<Value>>(raw)
        .unwrap_or_default()
        .into_iter()
        .map(mask_key_value_entry)
        .collect()
}

/// Redact sensitive fields from a body string and truncate to the preview limit.
fn redact_and_truncate(raw: &str) -> (String, bool) {
    let body_type = guess_body_type(raw);
    let redacted = redact_body(raw, &body_type);
    truncate_body(&redacted, MAX_BODY_PREVIEW_BYTES)
}

/// Mask the `value` of a `{ "key": ..., "value": ... }` entry when its key is
/// sensitive, preserving the entry's other fields.
fn mask_key_value_entry(mut entry: Value) -> Value {
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

fn parse_optional_string(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a string",
            key
        ))),
    }
}

fn parse_required_string(
    arguments: &Map<String, Value>,
    key: &str,
    tool_name: &str,
) -> Result<String, ToolCallError> {
    match arguments.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        Some(Value::String(_)) => Err(ToolCallError::InvalidArguments(format!(
            "{} argument `{}` cannot be empty",
            tool_name, key
        ))),
        _ => Err(ToolCallError::InvalidArguments(format!(
            "{} requires argument `{}`",
            tool_name, key
        ))),
    }
}

fn parse_optional_bool(
    arguments: &Map<String, Value>,
    key: &str,
) -> Result<Option<bool>, ToolCallError> {
    match arguments.get(key) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(ToolCallError::InvalidArguments(format!(
            "argument `{}` must be a boolean",
            key
        ))),
    }
}

const DEFAULT_HISTORY_LIMIT: i64 = 50;
const MAX_HISTORY_LIMIT: i64 = 200;

fn parse_optional_history_limit(
    arguments: &Map<String, Value>,
) -> Result<Option<i64>, ToolCallError> {
    match arguments.get("limit") {
        None => Ok(Some(DEFAULT_HISTORY_LIMIT)),
        Some(Value::Number(n)) => {
            let value = n.as_i64().ok_or_else(|| {
                ToolCallError::InvalidArguments(
                    "argument `limit` must be a positive integer".to_string(),
                )
            })?;
            Ok(Some(value.clamp(1, MAX_HISTORY_LIMIT)))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `limit` must be a number".to_string(),
        )),
    }
}

fn parse_optional_timeout(arguments: &Map<String, Value>) -> Result<Option<u64>, ToolCallError> {
    match arguments.get("timeoutMs") {
        None => Ok(None),
        Some(Value::Number(n)) => {
            let ms = n.as_u64().ok_or_else(|| {
                ToolCallError::InvalidArguments(
                    "argument `timeoutMs` must be a positive number".to_string(),
                )
            })?;
            Ok(Some(ms.min(60_000)))
        }
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `timeoutMs` must be a number".to_string(),
        )),
    }
}

fn guess_body_type(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if serde_json::from_str::<Value>(trimmed).is_ok() {
            return "json".to_string();
        }
    }
    if trimmed.starts_with('<') {
        return "xml".to_string();
    }
    "text".to_string()
}

fn serialize_safe(value: impl serde::Serialize) -> Result<Value, ToolCallError> {
    serde_json::to_value(value).map_err(|_| ToolCallError::Execution {
        code: "TOOL_RESULT_SERIALIZATION_FAILED",
        message: "The tool result could not be serialized.",
    })
}

fn unexpected_result() -> ToolCallError {
    ToolCallError::Execution {
        code: "COMMAND_BUS_RESULT_MISMATCH",
        message: "The command-bus returned an unexpected result.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use unfour_command_bus::{
        ApiCollectionListResult, ApiCollectionSummary, ApiEnvironmentListResult,
        ApiHistoryDetailResult, ApiHistoryListResult, ApiRequestDetailResult, ApiRequestListResult,
        ApiRequestSummary, ReadCommand, ReadCommandResult,
    };
    use unfour_core::models::{
        ApiEnvironment, ApiHistoryDetail, ApiHistoryItem, ApiResponse, ApiSavedRequest,
        DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult, DatabaseQuerySafety,
        DatabaseSchema, KeyValue,
    };

    use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
    use crate::tools::ToolRegistry;

    // --- Test stubs ---

    struct ApiStubCommandBus;

    impl CommandBusAdapter for ApiStubCommandBus {
        fn execute_read(
            &self,
            command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            Ok(match command {
                ReadCommand::ApiListCollections { .. } => {
                    ReadCommandResult::ApiCollections(ApiCollectionListResult {
                        collections: vec![
                            ApiCollectionSummary {
                                id: "users".to_string(),
                                name: "Users".to_string(),
                                request_count: 3,
                                workspace_id: "ws-1".to_string(),
                            },
                            ApiCollectionSummary {
                                id: String::new(),
                                name: "General".to_string(),
                                request_count: 1,
                                workspace_id: "ws-1".to_string(),
                            },
                        ],
                        count: 2,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListRequests { .. } => {
                    ReadCommandResult::ApiRequests(ApiRequestListResult {
                        requests: vec![ApiRequestSummary {
                            id: "req-1".to_string(),
                            name: "Get Users".to_string(),
                            method: "GET".to_string(),
                            url_preview: "https://api.example.com/users?token=secret123&page=1".to_string(),
                            collection_id: "users".to_string(),
                            workspace_id: "ws-1".to_string(),
                            has_body: false,
                            header_count: 2,
                        }],
                        count: 1,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiGetRequest { request_id } => {
                    ReadCommandResult::ApiRequest(ApiRequestDetailResult {
                        request: ApiSavedRequest {
                            id: request_id,
                            workspace_id: "ws-1".to_string(),
                            name: "Create User".to_string(),
                            folder_path: Some("users".to_string()),
                            collection_id: Some("users".to_string()),
                            auth_json: r#"{"type":"none"}"#.to_string(),
                            method: "POST".to_string(),
                            url: "https://api.example.com/users?api_key=secret".to_string(),
                            headers_json: r#"[{"key":"Authorization","value":"Bearer secret-token","enabled":true},{"key":"Content-Type","value":"application/json","enabled":true}]"#.to_string(),
                            query_json: r#"[{"key":"page","value":"1","enabled":true},{"key":"token","value":"secret","enabled":true}]"#.to_string(),
                            body: Some(r#"{"name":"test","password":"secret123"}"#.to_string()),
                            body_kind: "json".to_string(),
                            created_at: String::new(),
                            updated_at: String::new(),
                            deleted_at: None,
                            revision: 1,
                            sync_status: "local".to_string(),
                            remote_id: None,
                        },
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListHistory { .. } => {
                    ReadCommandResult::ApiHistory(ApiHistoryListResult {
                        history: vec![ApiHistoryItem {
                            id: "hist-1".to_string(),
                            workspace_id: "ws-1".to_string(),
                            name: Some("Get Users".to_string()),
                            method: "GET".to_string(),
                            url: "https://api.example.com/users?token=secret123&page=2".to_string(),
                            status: Some(500),
                            duration_ms: Some(87),
                            created_at: "2026-06-20T00:00:00Z".to_string(),
                            updated_at: "2026-06-20T00:00:00Z".to_string(),
                            deleted_at: None,
                            revision: 1,
                            sync_status: "local".to_string(),
                            remote_id: None,
                        }],
                        count: 1,
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiGetHistory { history_id, .. } => {
                    ReadCommandResult::ApiHistoryDetailResult(ApiHistoryDetailResult {
                        detail: ApiHistoryDetail {
                            id: history_id,
                            workspace_id: "ws-1".to_string(),
                            name: Some("Create User".to_string()),
                            method: "POST".to_string(),
                            url: "https://api.example.com/users?api_key=secret".to_string(),
                            request_headers_json: r#"[{"key":"Authorization","value":"Bearer secret-token","enabled":true}]"#.to_string(),
                            request_query_json: r#"[{"key":"token","value":"secret","enabled":true}]"#.to_string(),
                            request_body: Some(r#"{"name":"test","password":"secret123"}"#.to_string()),
                            status: Some(401),
                            duration_ms: Some(120),
                            response_headers_json: r#"[{"key":"Set-Cookie","value":"session=secret-session-id","enabled":true}]"#.to_string(),
                            response_body_preview: Some(r#"{"error":"unauthorized","token":"secret-jwt"}"#.to_string()),
                            created_at: "2026-06-20T00:00:00Z".to_string(),
                            updated_at: "2026-06-20T00:00:00Z".to_string(),
                            deleted_at: None,
                            revision: 1,
                            sync_status: "local".to_string(),
                            remote_id: None,
                        },
                        source: "command-bus".to_string(),
                    })
                }
                ReadCommand::ApiListEnvironments { .. } => {
                    ReadCommandResult::ApiEnvironments(ApiEnvironmentListResult {
                        environments: vec![ApiEnvironment {
                            id: "env-1".to_string(),
                            workspace_id: "ws-1".to_string(),
                            name: "Staging".to_string(),
                            variables: vec![
                                KeyValue {
                                    key: "baseUrl".to_string(),
                                    value: "https://api.staging.example.com".to_string(),
                                    enabled: true,
                                },
                                KeyValue {
                                    key: "token".to_string(),
                                    value: "Bearer secret-token".to_string(),
                                    enabled: true,
                                },
                            ],
                            is_active: true,
                            created_at: String::new(),
                            updated_at: String::new(),
                        }],
                        count: 1,
                        source: "command-bus".to_string(),
                    })
                }
                _ => ReadCommandResult::ApiCollections(ApiCollectionListResult {
                    collections: vec![],
                    count: 0,
                    source: "command-bus".to_string(),
                }),
            })
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            Ok(ApiResponse {
                history_id: "hist-1".to_string(),
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![
                    KeyValue {
                        key: "Content-Type".to_string(),
                        value: "application/json".to_string(),
                        enabled: true,
                    },
                    KeyValue {
                        key: "Set-Cookie".to_string(),
                        value: "session=secret-session-id".to_string(),
                        enabled: true,
                    },
                ],
                body: r#"{"ok":true,"token":"secret-jwt"}"#.to_string(),
                duration_ms: 123,
            })
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            Ok(vec![])
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            _connection_id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            Ok(DatabaseSchema {
                connection_id: String::new(),
                tables: vec![],
            })
        }

        fn execute_db_query(
            &self,
            _input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            Ok(DatabaseQueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: 0,
                duration_ms: 0,
                safety: DatabaseQuerySafety {
                    classification: "read".to_string(),
                    requires_confirmation: false,
                    confirmed: true,
                    message: None,
                },
            })
        }
    }

    struct FailingApiCommandBus;

    impl CommandBusAdapter for FailingApiCommandBus {
        fn execute_read(
            &self,
            _command: ReadCommand,
        ) -> Result<ReadCommandResult, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_READ_FAILED",
                message: "The command-bus read operation failed.",
            })
        }

        fn execute_saved_api_request(
            &self,
            _request_id: &str,
            _timeout_ms: Option<u64>,
        ) -> Result<ApiResponse, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_API_SEND_FAILED",
                message: "The command-bus API send operation failed.",
            })
        }

        fn list_db_connections(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_DB_LIST_FAILED",
                message: "The command-bus database list operation failed.",
            })
        }

        fn get_db_schema(
            &self,
            _workspace_id: &str,
            _connection_id: &str,
        ) -> Result<DatabaseSchema, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_DB_SCHEMA_FAILED",
                message: "The command-bus database schema operation failed.",
            })
        }

        fn execute_db_query(
            &self,
            _input: DatabaseQueryInput,
        ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_DB_QUERY_FAILED",
                message: "The command-bus database query operation failed.",
            })
        }
    }

    fn api_registry() -> ToolRegistry {
        ToolRegistry::with_command_bus(Arc::new(ApiStubCommandBus))
    }

    // --- Schema tests ---

    #[test]
    fn api_tools_are_registered() {
        let definitions = api_registry().definitions();
        assert!(definitions
            .iter()
            .any(|d| d.name == "unfour.api.list_collections"));
        assert!(definitions
            .iter()
            .any(|d| d.name == "unfour.api.list_requests"));
        assert!(definitions
            .iter()
            .any(|d| d.name == "unfour.api.get_request"));
        assert!(definitions
            .iter()
            .any(|d| d.name == "unfour.api.send_request"));
    }

    #[test]
    fn api_tools_have_valid_input_schemas() {
        let definitions = api_registry().definitions();
        for name in &[
            "unfour.api.list_collections",
            "unfour.api.list_requests",
            "unfour.api.get_request",
            "unfour.api.send_request",
        ] {
            let def = definitions.iter().find(|d| d.name == *name).unwrap();
            assert_eq!(
                def.input_schema["type"], "object",
                "{} should have object input schema",
                name
            );
        }
    }

    // --- list_collections tests ---

    #[test]
    fn list_collections_returns_structured_result() {
        let result = api_registry()
            .call("unfour.api.list_collections", json!({}))
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["count"], 2);
        assert_eq!(
            result["structuredContent"]["collections"][0]["name"],
            "Users"
        );
        assert_eq!(
            result["structuredContent"]["collections"][0]["requestCount"],
            3
        );
        assert_eq!(result["structuredContent"]["source"], "command-bus");
    }

    // --- list_requests tests ---

    #[test]
    fn list_requests_redacts_sensitive_url_params() {
        let result = api_registry()
            .call("unfour.api.list_requests", json!({}))
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        let requests = &result["structuredContent"]["requests"];
        assert_eq!(requests[0]["id"], "req-1");
        // token should be redacted in urlPreview
        let url_preview = requests[0]["urlPreview"].as_str().unwrap();
        assert!(
            url_preview.contains("token=[mask "),
            "token should be masked in urlPreview"
        );
        assert!(
            !url_preview.contains("secret123"),
            "raw token should not appear"
        );
        assert!(url_preview.contains("page=1"), "safe params preserved");
    }

    // --- get_request tests ---

    #[test]
    fn get_request_redacts_sensitive_data() {
        let result = api_registry()
            .call("unfour.api.get_request", json!({ "requestId": "req-1" }))
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        let request = &result["structuredContent"]["request"];

        // URL query params masked
        let url = request["url"].as_str().unwrap();
        assert!(
            url.contains("api_key=[mask "),
            "api_key should be masked in URL"
        );
        assert!(!url.contains("=secret"), "raw secret should not appear");

        // Authorization header masked (scheme preserved for diagnosis)
        let headers = request["headers"].as_array().unwrap();
        let auth_header = headers
            .iter()
            .find(|h| h["key"] == "Authorization")
            .unwrap();
        let auth_value = auth_header["value"].as_str().unwrap();
        assert!(auth_value.starts_with("[mask "));
        assert!(auth_value.contains("scheme=Bearer"));
        assert!(!auth_value.contains("secret-token"));

        // Content-Type preserved
        let ct_header = headers.iter().find(|h| h["key"] == "Content-Type").unwrap();
        assert_eq!(ct_header["value"], "application/json");

        // Query param token masked
        let query = request["query"].as_array().unwrap();
        let token_param = query.iter().find(|q| q["key"] == "token").unwrap();
        assert!(token_param["value"].as_str().unwrap().starts_with("[mask "));

        // Body password masked
        let body = request["bodyPreview"].as_str().unwrap();
        assert!(body.contains("[mask "), "password should be masked in body");
        assert!(
            !body.contains("secret123"),
            "raw password should not appear"
        );
        assert!(body.contains("test"), "non-sensitive body values preserved");

        assert_eq!(request["collectionId"], "users");
        assert_eq!(result["structuredContent"]["source"], "command-bus");
    }

    #[test]
    fn get_request_requires_request_id() {
        let result = api_registry().call("unfour.api.get_request", json!({}));
        assert!(result.is_err(), "should fail without requestId");
    }

    // --- send_request tests ---

    #[test]
    fn send_request_returns_success_with_redacted_response() {
        let result = api_registry()
            .call("unfour.api.send_request", json!({ "requestId": "req-1" }))
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        let content = &result["structuredContent"];
        assert_eq!(content["ok"], true);
        assert_eq!(content["status"], 200);
        assert_eq!(content["statusText"], "OK");
        assert_eq!(content["durationMs"], 123);
        assert_eq!(content["source"], "command-bus");

        // Set-Cookie response header masked
        let headers = content["headers"].as_array().unwrap();
        let set_cookie = headers.iter().find(|h| h["name"] == "Set-Cookie").unwrap();
        assert!(set_cookie["value"].as_str().unwrap().starts_with("[mask "));

        // Body token masked
        let body = content["bodyPreview"].as_str().unwrap();
        assert!(
            body.contains("[mask "),
            "token should be masked in response body"
        );
        assert!(!body.contains("secret-jwt"), "raw token should not appear");
    }

    #[test]
    fn send_request_clamps_timeout_to_60s() {
        // Sending with 120000ms should be clamped - the stub ignores timeout,
        // but we verify the tool doesn't reject the call
        let result = api_registry()
            .call(
                "unfour.api.send_request",
                json!({ "requestId": "req-1", "timeoutMs": 120000 }),
            )
            .expect("should succeed");
        assert_eq!(result["structuredContent"]["ok"], true);
    }

    #[test]
    fn send_request_rejects_missing_request_id() {
        let result = api_registry().call("unfour.api.send_request", json!({}));
        assert!(result.is_err(), "should fail without requestId");
    }

    #[test]
    fn send_request_returns_structured_error_on_failure() {
        let registry = ToolRegistry::with_command_bus(Arc::new(FailingApiCommandBus));
        let result = registry
            .call("unfour.api.send_request", json!({ "requestId": "req-1" }))
            .expect("execution errors become MCP tool results");

        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "COMMAND_BUS_API_SEND_FAILED"
        );
    }

    #[test]
    fn command_bus_read_failure_returns_structured_error() {
        let registry = ToolRegistry::with_command_bus(Arc::new(FailingApiCommandBus));
        let result = registry
            .call("unfour.api.list_collections", json!({}))
            .expect("execution errors become MCP tool results");

        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "COMMAND_BUS_READ_FAILED"
        );
    }

    // --- history tests ---

    #[test]
    fn list_history_masks_url_and_returns_status() {
        let result = api_registry()
            .call("unfour.api.list_history", json!({}))
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        let content = &result["structuredContent"];
        assert_eq!(content["count"], 1);
        let item = &content["history"][0];
        assert_eq!(item["status"], 500);
        let url = item["url"].as_str().unwrap();
        assert!(url.contains("token=[mask "), "token should be masked");
        assert!(!url.contains("secret123"), "raw token should not appear");
        assert!(url.contains("page=2"), "safe params preserved");
    }

    #[test]
    fn get_history_masks_request_and_response() {
        let result = api_registry()
            .call("unfour.api.get_history", json!({ "historyId": "hist-1" }))
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        let h = &result["structuredContent"]["history"];
        assert_eq!(h["status"], 401);

        let url = h["url"].as_str().unwrap();
        assert!(url.contains("api_key=[mask "));
        assert!(!url.contains("=secret"));

        let req_headers = h["requestHeaders"].as_array().unwrap();
        let auth = req_headers
            .iter()
            .find(|x| x["key"] == "Authorization")
            .unwrap();
        let auth_val = auth["value"].as_str().unwrap();
        assert!(auth_val.starts_with("[mask "));
        assert!(auth_val.contains("scheme=Bearer"));
        assert!(!auth_val.contains("secret-token"));

        let resp_headers = h["responseHeaders"].as_array().unwrap();
        let cookie = resp_headers
            .iter()
            .find(|x| x["key"] == "Set-Cookie")
            .unwrap();
        assert!(cookie["value"].as_str().unwrap().starts_with("[mask "));

        let req_body = h["requestBody"].as_str().unwrap();
        assert!(req_body.contains("[mask "));
        assert!(!req_body.contains("secret123"));

        let resp_body = h["responseBodyPreview"].as_str().unwrap();
        assert!(resp_body.contains("[mask "));
        assert!(!resp_body.contains("secret-jwt"));
    }

    #[test]
    fn get_history_requires_history_id() {
        let result = api_registry().call("unfour.api.get_history", json!({}));
        assert!(result.is_err(), "should fail without historyId");
    }

    // --- environment tests ---

    #[test]
    fn list_environments_masks_sensitive_variables_only() {
        let result = api_registry()
            .call("unfour.api.list_environments", json!({}))
            .expect("should succeed");

        assert_eq!(result["isError"], false);
        let env = &result["structuredContent"]["environments"][0];
        assert_eq!(env["name"], "Staging");
        assert_eq!(env["isActive"], true);
        assert_eq!(env["variableCount"], 2);

        let vars = env["variables"].as_array().unwrap();
        let base = vars.iter().find(|v| v["key"] == "baseUrl").unwrap();
        // Non-sensitive value is shown verbatim so requests are intelligible.
        assert_eq!(base["value"], "https://api.staging.example.com");

        let token = vars.iter().find(|v| v["key"] == "token").unwrap();
        let token_val = token["value"].as_str().unwrap();
        assert!(token_val.starts_with("[mask "));
        assert!(token_val.contains("scheme=Bearer"));
        assert!(!token_val.contains("secret-token"));
    }

    #[test]
    fn unknown_tool_returns_error() {
        let result = api_registry().call("unfour.api.nonexistent", json!({}));
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolCallError::UnknownTool(name) => assert_eq!(name, "unfour.api.nonexistent"),
            other => panic!("expected UnknownTool, got {:?}", other),
        }
    }

    #[test]
    fn body_truncation_works_at_20kb() {
        let large_body = "x".repeat(30_000);
        let (truncated, was_truncated) = truncate_body(&large_body, MAX_BODY_PREVIEW_BYTES);
        assert!(was_truncated);
        assert_eq!(truncated.len(), MAX_BODY_PREVIEW_BYTES);
    }
}
