use serde_json::{json, Map, Value};
use unfour_command_bus::{ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiRequestInput, ApiSavedRequest, KeyValue, WorkspaceEnvironment, WorkspaceEnvironmentVariable,
    WorkspaceVariableInput,
};

use crate::command_bus_adapter::CommandBusAdapter;
use crate::sanitize::{
    is_sensitive_key, mask_secret, redact_body, redact_header_value, redact_url_query,
    truncate_body, MAX_BODY_PREVIEW_BYTES,
};

use super::policy::ToolPolicyEvaluation;
use super::{
    confirmation::ensure_confirmed_if_guarded, object_with_allowed_keys, RegisteredTool,
    ToolAnnotations, ToolCallError, ToolDefinition,
};

#[path = "api_environment_variables.rs"]
mod api_environment_variables;
#[path = "api_handlers.rs"]
mod api_handlers;
#[path = "api_support.rs"]
mod api_support;

use api_environment_variables::*;
use api_handlers::*;
use api_support::*;

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
                            "description": "Optional per-call environment override. It is used for variable resolution and saved request scripts without changing the workspace active environment."
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
                name: "unfour.api.create_environment",
                title: "Create API Environment",
                description:
                    "Creates an empty API environment in local Unfour metadata through the command bus. Dev/test allow it; prod blocks by workspace policy.",
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
            handler: api_create_environment,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.update_environment",
                title: "Update API Environment",
                description:
                    "Updates an API environment name and variables through the command bus. Sensitive variable values are masked in the result. Dev/test allow it; prod blocks by workspace policy.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "environmentId": { "type": "string" },
                        "name": { "type": "string" },
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
                        }
                    },
                    "required": ["environmentId", "name", "variables"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: api_update_environment,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.delete_environment",
                title: "Delete API Environment",
                description:
                    "Soft-deletes an API environment through the command bus. Guarded policy requires a content-bound confirmation_text; read-only policy blocks deletion.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "environmentId": { "type": "string" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["environmentId"],
                    "additionalProperties": false
                }),
                output_schema: json!({ "type": "object" }),
                annotations: ToolAnnotations::local_write_destructive(),
            },
            handler: api_delete_environment,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.set_environment_variable",
                title: "Set API Environment Variable",
                description:
                    "Creates or updates one API environment variable by environmentId and key through the command bus. Empty values are allowed. Omitted isSecret/description preserve existing metadata, and returned secrets are masked.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "environmentId": { "type": "string" },
                        "key": { "type": "string" },
                        "value": { "type": "string" },
                        "enabled": { "type": "boolean", "default": true },
                        "isSecret": { "type": "boolean" },
                        "description": { "type": ["string", "null"] }
                    },
                    "required": ["environmentId", "key", "value"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "variable": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "environmentId": { "type": "string" },
                                "key": { "type": "string" },
                                "value": { "type": "string" },
                                "enabled": { "type": "boolean" },
                                "isSecret": { "type": "boolean" },
                                "description": { "type": ["string", "null"] }
                            },
                            "required": ["id", "environmentId", "key", "value", "enabled", "isSecret", "description"],
                            "additionalProperties": false
                        },
                        "created": { "type": "boolean" },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["variable", "created", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::local_write(),
            },
            handler: api_set_environment_variable,
        },
        RegisteredTool {
            definition: ToolDefinition {
                name: "unfour.api.delete_environment_variable",
                title: "Delete API Environment Variable",
                description:
                    "Deletes one API environment variable by environmentId and key through the command bus. Guarded policy requires confirmation; read-only policy blocks deletion.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "workspaceId": { "type": "string" },
                        "environmentId": { "type": "string" },
                        "key": { "type": "string" },
                        "confirm": { "type": "boolean" },
                        "confirmationText": { "type": "string" },
                        "confirmation_text": { "type": "string" }
                    },
                    "required": ["environmentId", "key"],
                    "additionalProperties": false
                }),
                output_schema: json!({
                    "type": "object",
                    "properties": {
                        "deleted": { "type": "boolean", "const": true },
                        "environmentId": { "type": "string" },
                        "key": { "type": "string" },
                        "source": { "type": "string", "const": "command-bus" }
                    },
                    "required": ["deleted", "environmentId", "key", "source"],
                    "additionalProperties": false
                }),
                annotations: ToolAnnotations::local_write_destructive(),
            },
            handler: api_delete_environment_variable,
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

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
