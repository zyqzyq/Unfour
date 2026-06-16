use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::sanitize::{
    is_sensitive_key, redact_body, redact_header_value, redact_url_query, truncate_body,
    MAX_BODY_PREVIEW_BYTES, REDACTED,
};

use super::{object_with_allowed_keys, RegisteredTool, ToolCallError, ToolDefinition, ToolHandler};

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    vec![
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.list_collections",
                title: "List API Collections",
                description:
                    "Lists API request collections (derived from folder paths) for the active workspace through the Unfour command bus.",
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
            },
            handler: ToolHandler::Real(api_list_collections),
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
                            "description": "Optional collection (folder path) filter."
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
            },
            handler: ToolHandler::Real(api_list_requests),
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
            },
            handler: ToolHandler::Real(api_get_request),
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
            },
            handler: ToolHandler::Real(api_send_request),
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
        .map(|mut h| {
            if let Some(name) = h.get("key").and_then(|v| v.as_str()) {
                if is_sensitive_key(name) {
                    if let Some(obj) = h.as_object_mut() {
                        obj.insert("value".to_string(), Value::String(REDACTED.to_string()));
                    }
                }
            }
            h
        })
        .collect();

    // Parse and redact query params
    let query: Vec<Value> = serde_json::from_str::<Vec<Value>>(&saved.query_json)
        .unwrap_or_default()
        .into_iter()
        .map(|mut q| {
            if let Some(name) = q.get("key").and_then(|v| v.as_str()) {
                if is_sensitive_key(name) {
                    if let Some(obj) = q.as_object_mut() {
                        obj.insert("value".to_string(), Value::String(REDACTED.to_string()));
                    }
                }
            }
            q
        })
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
        "collectionId": saved.folder_path.unwrap_or_default()
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
    let request_id =
        parse_required_string(&arguments, "requestId", "unfour.api.send_request")?;
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

// --- Helpers ---

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
        ApiCollectionListResult, ApiCollectionSummary, ApiRequestDetailResult,
        ApiRequestListResult, ApiRequestSummary, ReadCommand, ReadCommandResult,
    };
    use unfour_core::models::{
        ApiResponse, ApiSavedRequest, DatabaseConnection, DatabaseQueryInput, DatabaseQueryResult,
        DatabaseQuerySafety, DatabaseSchema, KeyValue,
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
        assert!(definitions.iter().any(|d| d.name == "unfour.api.list_collections"));
        assert!(definitions.iter().any(|d| d.name == "unfour.api.list_requests"));
        assert!(definitions.iter().any(|d| d.name == "unfour.api.get_request"));
        assert!(definitions.iter().any(|d| d.name == "unfour.api.send_request"));
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
            assert_eq!(def.input_schema["type"], "object", "{} should have object input schema", name);
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
        assert_eq!(result["structuredContent"]["collections"][0]["name"], "Users");
        assert_eq!(result["structuredContent"]["collections"][0]["requestCount"], 3);
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
        assert!(url_preview.contains("[REDACTED]"), "token should be redacted in urlPreview");
        assert!(!url_preview.contains("secret123"), "raw token should not appear");
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

        // URL query params redacted
        let url = request["url"].as_str().unwrap();
        assert!(url.contains("[REDACTED]"), "api_key should be redacted in URL");
        assert!(!url.contains("secret"), "raw secret should not appear");

        // Authorization header redacted
        let headers = request["headers"].as_array().unwrap();
        let auth_header = headers.iter().find(|h| h["key"] == "Authorization").unwrap();
        assert_eq!(auth_header["value"], "[REDACTED]");

        // Content-Type preserved
        let ct_header = headers.iter().find(|h| h["key"] == "Content-Type").unwrap();
        assert_eq!(ct_header["value"], "application/json");

        // Query param token redacted
        let query = request["query"].as_array().unwrap();
        let token_param = query.iter().find(|q| q["key"] == "token").unwrap();
        assert_eq!(token_param["value"], "[REDACTED]");

        // Body password redacted
        let body = request["bodyPreview"].as_str().unwrap();
        assert!(body.contains("[REDACTED]"), "password should be redacted in body");
        assert!(!body.contains("secret123"), "raw password should not appear");
        assert!(body.contains("test"), "non-sensitive body values preserved");

        assert_eq!(result["structuredContent"]["source"], "command-bus");
    }

    #[test]
    fn get_request_requires_request_id() {
        let result = api_registry()
            .call("unfour.api.get_request", json!({}));
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

        // Set-Cookie response header redacted
        let headers = content["headers"].as_array().unwrap();
        let set_cookie = headers.iter().find(|h| h["name"] == "Set-Cookie").unwrap();
        assert_eq!(set_cookie["value"], "[REDACTED]");

        // Body token redacted
        let body = content["bodyPreview"].as_str().unwrap();
        assert!(body.contains("[REDACTED]"), "token should be redacted in response body");
        assert!(!body.contains("secret-jwt"), "raw token should not appear");
    }

    #[test]
    fn send_request_clamps_timeout_to_60s() {
        // Sending with 120000ms should be clamped - the stub ignores timeout,
        // but we verify the tool doesn't reject the call
        let result = api_registry()
            .call("unfour.api.send_request", json!({ "requestId": "req-1", "timeoutMs": 120000 }))
            .expect("should succeed");
        assert_eq!(result["structuredContent"]["ok"], true);
    }

    #[test]
    fn send_request_rejects_missing_request_id() {
        let result = api_registry()
            .call("unfour.api.send_request", json!({}));
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

    #[test]
    fn unknown_tool_returns_error() {
        let result = api_registry()
            .call("unfour.api.nonexistent", json!({}));
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
