use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use unfour_core::models::{
    ApiCollection, ApiCollectionFolder, ApiEnvironment, ApiHistoryDetail, ApiSavedRequest,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenApiExportSource {
    pub(super) collection: ApiCollection,
    #[serde(default)]
    pub(super) collection_auth_json: Option<String>,
    #[serde(default)]
    pub(super) collection_base_url: Option<String>,
    #[serde(default)]
    pub(super) collection_version: Option<String>,
    #[serde(default)]
    pub(super) environments: Vec<ApiEnvironment>,
    #[serde(default)]
    pub(super) folders: Vec<ApiCollectionFolder>,
    #[serde(default)]
    pub(super) histories: Vec<ApiHistoryDetail>,
    #[serde(default)]
    pub(super) requests: Vec<ApiSavedRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiDocument {
    pub(super) openapi: String,
    pub(super) info: OpenApiInfo,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) servers: Vec<OpenApiServer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) tags: Vec<OpenApiTag>,
    pub(super) paths: BTreeMap<String, OpenApiPathItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) components: Option<OpenApiComponents>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) security: Option<Vec<SecurityRequirement>>,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiInfo {
    pub(super) title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) description: Option<String>,
    pub(super) version: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiServer {
    pub(super) url: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) variables: BTreeMap<String, OpenApiServerVariable>,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiServerVariable {
    pub(super) default: String,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiTag {
    pub(super) name: String,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(super) struct OpenApiPathItem {
    #[serde(flatten)]
    pub(super) operations: BTreeMap<String, OpenApiOperation>,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiOperation {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) tags: Vec<String>,
    pub(super) summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) parameters: Vec<OpenApiParameter>,
    #[serde(rename = "requestBody", skip_serializing_if = "Option::is_none")]
    pub(super) request_body: Option<OpenApiRequestBody>,
    pub(super) responses: BTreeMap<String, OpenApiResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) security: Option<Vec<SecurityRequirement>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) servers: Vec<OpenApiServer>,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiParameter {
    pub(super) name: String,
    #[serde(rename = "in")]
    pub(super) location: String,
    pub(super) required: bool,
    pub(super) schema: OpenApiSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) example: Option<Value>,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiRequestBody {
    pub(super) content: BTreeMap<String, OpenApiMediaType>,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiMediaType {
    pub(super) schema: OpenApiSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) example: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiResponse {
    pub(super) description: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) headers: BTreeMap<String, OpenApiHeader>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) content: BTreeMap<String, OpenApiMediaType>,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiHeader {
    pub(super) schema: OpenApiSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) example: Option<Value>,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(super) struct OpenApiSchema {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub(super) schema_type: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) properties: BTreeMap<String, OpenApiSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) items: Option<Box<OpenApiSchema>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(super) struct OpenApiComponents {
    #[serde(rename = "securitySchemes", skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) security_schemes: BTreeMap<String, OpenApiSecurityScheme>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct OpenApiSecurityScheme {
    #[serde(rename = "type")]
    pub(super) scheme_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scheme: Option<String>,
    #[serde(rename = "bearerFormat", skip_serializing_if = "Option::is_none")]
    pub(super) bearer_format: Option<String>,
    #[serde(rename = "in", skip_serializing_if = "Option::is_none")]
    pub(super) location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    pub(super) description: String,
    #[serde(flatten)]
    pub(super) extensions: BTreeMap<String, Value>,
}

pub(super) type SecurityRequirement = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AuthDescriptor {
    ApiKey { key: String, location: String },
    Basic,
    Bearer,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedRequest<'a> {
    pub(super) request: &'a ApiSavedRequest,
    pub(super) auth: Option<AuthDescriptor>,
    pub(super) server: Option<String>,
    pub(super) original_path: String,
    pub(super) openapi_path: String,
    pub(super) path_parameters: Vec<String>,
    pub(super) template_variables: Vec<String>,
}
