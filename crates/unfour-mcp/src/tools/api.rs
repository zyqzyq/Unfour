use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::{ApiRequestInput, ApiSavedRequest, KeyValue};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::sanitize::{
    is_sensitive_key, mask_secret, redact_body, redact_header_value, redact_url_query,
    truncate_body, MAX_BODY_PREVIEW_BYTES,
};

use super::{
    confirmation::ensure_confirmed_if_guarded, object_with_allowed_keys, RegisteredTool,
    ToolAnnotations, ToolCallError, ToolDefinition,
};
use super::policy::ToolPolicyEvaluation;

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
                title: "Send API Request",
                description:
                    "Sends either a saved API request by requestId or one ad-hoc request described by method/url/headers/query/body through the Unfour command bus. Use it to reproduce API failures during agent troubleshooting. Dev allows all HTTP methods; test allows sends but marks mutating methods as write risk; prod only allows GET/HEAD/OPTIONS. Non-2xx HTTP responses return structured status/body data rather than MCP tool failure. Sensitive headers, cookies, URL tokens, and JSON body fields are masked.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "requestId": {
                            "type": "string",
                            "description": "Optional saved API request ID to replay. Omit when sending method/url directly."
                        },
                        "workspaceId": {
                            "type": "string",
                            "description": "Optional workspace ID for ad-hoc requests. Uses the active workspace if omitted."
                        },
                        "name": {
                            "type": "string",
                            "description": "Optional display name for ad-hoc request history."
                        },
                        "method": {
                            "type": "string",
                            "description": "HTTP method for ad-hoc sends, such as GET, POST, PUT, PATCH, DELETE."
                        },
                        "url": {
                            "type": "string",
                            "description": "URL for ad-hoc sends."
                        },
                        "headers": {
                            "description": "Optional headers as an object or array of {key,value,enabled}. Sensitive values are redacted in results."
                        },
                        "query": {
                            "description": "Optional query parameters as an object or array of {key,value,enabled}."
                        },
                        "body": {
                            "type": ["string", "null"],
                            "description": "Optional request body for ad-hoc sends."
                        },
                        "bodyKind": {
                            "type": "string",
                            "description": "Optional body kind for ad-hoc sends (json, text, form, xml). Defaults to json."
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
                name: "unfour.api.create_request",
                title: "Create API Request",
                description:
                    "Creates a saved API request record in the workspace through the Unfour command bus. This mutates local Unfour metadata only; it does not send traffic. Dev/test allow it by default, while prod blocks by workspace policy. Returns the new request id and a redacted summary.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "collectionId": { "type": "string" },
                        "parentId": { "type": "string" },
                        "parentFolderId": { "type": "string" },
                        "name": { "type": "string" },
                        "method": { "type": "string" },
                        "url": { "type": "string" },
                        "headers": {},
                        "query": {},
                        "body": { "type": ["string", "null"] },
                        "bodyKind": { "type": "string" },
                        "auth": {},
                        "authJson": { "type": "string" }
                    },
                    "required": ["name", "method", "url"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: api_create_request,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.update_request",
                title: "Update API Request",
                description:
                    "Updates a saved API request record through the Unfour command bus. Omitted fields keep their existing values. This mutates local metadata only; dev/test allow it, prod blocks or requires future explicit policy. Returns the updated redacted request summary.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "requestId": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "collectionId": { "type": "string" },
                        "parentId": { "type": "string" },
                        "parentFolderId": { "type": "string" },
                        "name": { "type": "string" },
                        "method": { "type": "string" },
                        "url": { "type": "string" },
                        "headers": {},
                        "query": {},
                        "body": { "type": ["string", "null"] },
                        "bodyKind": { "type": "string" },
                        "auth": {},
                        "authJson": { "type": "string" }
                    },
                    "required": ["requestId"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: api_update_request,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.delete_request",
                title: "Delete API Request",
                description:
                    "Soft-deletes a saved API request through the Unfour command bus. Dev/test require a content-bound confirmation_text before deletion; prod blocks by policy. Returns the remaining request count and confirms softDelete=true.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "requestId": { "type": "string" },
                        "workspaceId": { "type": "string" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["requestId"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write_destructive(),
            },
            handler: api_delete_request,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.create_collection",
                title: "Create API Collection",
                description:
                    "Creates an API collection in local Unfour metadata through the command bus. Dev/test allow it; prod blocks by workspace policy. Returns the new collection id and summary.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "name": { "type": "string" }
                    },
                    "required": ["name"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: api_create_collection,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.update_collection",
                title: "Update API Collection",
                description:
                    "Renames an API collection in local Unfour metadata through the command bus. Dev/test allow it; prod blocks by workspace policy.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "collectionId": { "type": "string" },
                        "name": { "type": "string" }
                    },
                    "required": ["collectionId", "name"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: api_update_collection,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.delete_collection",
                title: "Delete API Collection",
                description:
                    "Soft-deletes an API collection and cascades soft-deletion to its folders and requests through the command bus. Dev/test require a content-bound confirmation_text; prod blocks by policy.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "collectionId": { "type": "string" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["collectionId"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write_destructive(),
            },
            handler: api_delete_collection,
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

fn api_list_requests(
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

fn api_get_request(
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

fn api_send_request(
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

    let response = if let Some(request_id) = parse_optional_string(&arguments, "requestId")? {
        let workspace_id = parse_optional_string(&arguments, "workspaceId")?;
        command_bus.execute_saved_api_request_in_workspace(
            workspace_id.as_deref(),
            &request_id,
            timeout_ms,
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
        };
        command_bus.send_api_request(input)
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

fn api_create_request(
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

fn api_update_request(
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

fn api_delete_request(
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

fn api_create_collection(
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

fn api_update_collection(
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

fn api_delete_collection(
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

fn api_list_history(
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

fn api_get_history(
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

fn api_list_environments(
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

fn resolve_workspace_id(
    command_bus: &dyn CommandBusAdapter,
    arguments: &Map<String, Value>,
) -> Result<String, ToolCallError> {
    match parse_optional_string(arguments, "workspaceId")? {
        Some(id) => Ok(id),
        None => {
            let ws_result = command_bus
                .execute_read(ReadCommand::CurrentWorkspace)
                .map_err(|e| ToolCallError::Execution {
                    code: e.code,
                    message: e.message,
                })?;
            let ReadCommandResult::CurrentWorkspace(ws) = ws_result else {
                return Err(unexpected_result());
            };
            Ok(ws.workspace_id)
        }
    }
}

fn get_saved_request(
    command_bus: &dyn CommandBusAdapter,
    request_id: &str,
) -> Result<ApiSavedRequest, ToolCallError> {
    let result = command_bus
        .execute_read(ReadCommand::ApiGetRequest {
            request_id: request_id.to_string(),
        })
        .map_err(|error| ToolCallError::Execution {
            code: error.code,
            message: error.message,
        })?;
    let ReadCommandResult::ApiRequest(detail) = result else {
        return Err(unexpected_result());
    };
    Ok(detail.request)
}

fn parse_parent_folder_id(arguments: &Map<String, Value>) -> Result<Option<String>, ToolCallError> {
    parse_optional_string(arguments, "parentFolderId")
        .and_then(|value| Ok(value.or(parse_optional_string(arguments, "parentId")?)))
}

fn parse_auth_json(arguments: &Map<String, Value>) -> Result<Option<String>, ToolCallError> {
    if let Some(raw) = parse_optional_string(arguments, "authJson")? {
        return Ok(Some(raw));
    }
    match arguments.get("auth") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            serde_json::to_string(value)
                .map(Some)
                .map_err(|_| ToolCallError::Execution {
                    code: "TOOL_RESULT_SERIALIZATION_FAILED",
                    message: "The tool result could not be serialized.",
                })
        }
    }
}

fn parse_optional_body(
    arguments: &Map<String, Value>,
    existing: Option<String>,
) -> Result<Option<String>, ToolCallError> {
    match arguments.get("body") {
        None => Ok(existing),
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ToolCallError::InvalidArguments(
            "argument `body` must be a string or null".to_string(),
        )),
    }
}

fn parse_key_values(value: Option<&Value>) -> Result<Vec<KeyValue>, ToolCallError> {
    match value {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Object(object)) => Ok(object
            .iter()
            .map(|(key, value)| KeyValue {
                key: key.clone(),
                value: value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string()),
                enabled: true,
            })
            .collect()),
        Some(Value::Array(_)) => serde_json::from_value::<Vec<KeyValue>>(
            value.cloned().unwrap_or_else(|| Value::Array(vec![])),
        )
        .map_err(|_| {
            ToolCallError::InvalidArguments(
                "headers/query arrays must contain { key, value, enabled } objects".to_string(),
            )
        }),
        Some(_) => Err(ToolCallError::InvalidArguments(
            "headers/query must be an object or array".to_string(),
        )),
    }
}

fn safe_request_summary(saved: &ApiSavedRequest) -> Value {
    let header_count = serde_json::from_str::<Vec<Value>>(&saved.headers_json)
        .map(|headers| headers.len())
        .unwrap_or(0);
    json!({
        "id": saved.id,
        "workspaceId": saved.workspace_id,
        "collectionId": saved.collection_id,
        "parentId": saved.parent_folder_id,
        "name": saved.name,
        "method": saved.method,
        "url": redact_url_query(&saved.url),
        "hasBody": saved.body.as_ref().is_some_and(|body| !body.is_empty()),
        "headerCount": header_count,
        "updatedAt": saved.updated_at
    })
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
#[path = "api_tests.rs"]
mod tests;
