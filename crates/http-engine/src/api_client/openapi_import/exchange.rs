//! Format adapters share NormalizedCollection and the existing transactional writer.
use super::*;
use unfour_core::models::{
    ApiCollectionExportArtifact, ApiCollectionExportFormat, CollectionImportPreview,
};

#[path = "postman.rs"]
mod postman;

pub(super) fn decode(content: &str) -> AppResult<(NormalizedCollection, CollectionImportPreview)> {
    if content.len() > MAX_IMPORT_BYTES {
        return Err(import_validation("collection import file is too large"));
    }
    let value: Value = serde_json::from_str(content)
        .or_else(|_| serde_yaml_ng::from_str(content))
        .map_err(|_| import_validation("collection import must be valid JSON or YAML"))?;
    let mut warnings = Vec::new();
    let mut variables = Vec::new();
    let (mut parsed, format) = if value["format"] == "unfour.collection" && value["version"] == 1 {
        if value.as_object().is_some_and(|root| {
            root.keys()
                .any(|key| !["format", "version", "collection"].contains(&key.as_str()))
        }) {
            return Err(import_validation("unknown Unfour Collection v1 fields"));
        }
        let parsed = serde_json::from_value::<NormalizedCollection>(value["collection"].clone())
            .map_err(|_| import_validation("invalid Unfour Collection v1"))?;
        (parsed, "unfour")
    } else if value["info"]["schema"].as_str().is_some_and(|s| {
        s == "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
    }) {
        (
            postman::decode(&value, &mut warnings, &mut variables)?,
            "postman",
        )
    } else if value.get("openapi").is_some() {
        warnings.push("openapiProjection".into());
        if value.get("x-unfour-environments").is_some() {
            warnings.push("environmentsSeparate".into());
        }
        (parse_import(content)?, "openapi")
    } else {
        return Err(import_validation(
            "unsupported collection format or version",
        ));
    };
    validate(&mut parsed, &mut warnings)?;
    warnings.sort();
    warnings.dedup();
    let preview = CollectionImportPreview {
        format: format.into(),
        name: parsed.name.clone(),
        folder_count: parsed.folders.len(),
        request_count: parsed.requests.len(),
        script_count: parsed
            .requests
            .iter()
            .map(|r| {
                usize::from(r.pre_request_script.is_some())
                    + usize::from(r.post_response_script.is_some())
            })
            .sum(),
        variables,
        warnings,
    };
    Ok((parsed, preview))
}

fn validate(parsed: &mut NormalizedCollection, warnings: &mut Vec<String>) -> AppResult<()> {
    if parsed.name.trim().is_empty()
        || parsed.name.chars().count() > 120
        || parsed.folders.len() + parsed.requests.len() > MAX_IMPORT_ITEMS
    {
        return Err(import_validation("invalid collection name or item count"));
    }
    let mut ids = HashSet::new();
    for folder in &parsed.folders {
        if folder.source_id.is_empty()
            || !ids.insert(folder.source_id.as_str())
            || folder.name.trim().is_empty()
            || folder.name.chars().count() > 120
        {
            return Err(import_validation("invalid or duplicate folder"));
        }
    }
    for folder in &parsed.folders {
        let mut visited = HashSet::new();
        let mut current = Some(folder.source_id.as_str());
        while let Some(id) = current {
            if visited.len() >= 64 || !visited.insert(id) {
                return Err(import_validation(
                    "cyclic or excessively deep folder hierarchy",
                ));
            }
            let parent = parsed
                .folders
                .iter()
                .find(|f| f.source_id == id)
                .ok_or_else(|| import_validation("unknown parent folder"))?;
            current = parent.parent_source_id.as_deref();
        }
    }
    for request in &mut parsed.requests {
        if request
            .parent_source_id
            .as_deref()
            .is_some_and(|id| !ids.contains(id))
            || request.name.trim().is_empty()
            || request.name.chars().count() > 120
        {
            return Err(import_validation("invalid request name or parent"));
        }
        parse_method(&request.method)?;
        let settings: ApiRequestSettings = serde_json::from_str(&request.settings_json)?;
        if settings
            .timeout_ms
            .is_some_and(|ms| ms > MAX_API_TIMEOUT_MS)
        {
            return Err(import_validation("invalid request timeout"));
        }
        let _: Value = serde_json::from_str(&request.auth_json)?;
        crate::script_runtime::validate_script_config(
            request.pre_request_script.as_deref(),
            request.post_response_script.as_deref(),
            request.script_schema_version,
        )?;
        if request.pre_request_script.is_some() || request.post_response_script.is_some() {
            warnings.push("scriptCompatibility".into());
        }
        for script in [&request.pre_request_script, &request.post_response_script]
            .into_iter()
            .flatten()
        {
            if [
                "pm.sendRequest",
                "pm.collectionVariables",
                "pm.globals",
                "pm.cookies",
                "pm.vault",
                "pm.execution",
                "pm.visualizer",
                "pm.iterationData",
                "require(",
                "postman.",
                "setTimeout(",
                "setInterval(",
            ]
            .iter()
            .any(|api| script.contains(api))
            {
                warnings.push("unsupportedScriptApi".into());
            }
        }
        if request.body_kind == "multipart-form-data" {
            let parts = unfour_core::models::parse_multipart_definition(request.body.as_deref())?;
            if parts
                .iter()
                .any(|p| matches!(p, unfour_core::models::ApiMultipartPart::File { .. }))
            {
                warnings.push("reselectFiles".into());
            }
        }
    }
    Ok(())
}

fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace('-', "_");
    unfour_core::redaction::is_sensitive_key(&key)
        || key.contains("password")
        || key.contains("secret")
        || key.contains("passphrase")
        || key.ends_with("token")
        || key.ends_with("api_key")
}

fn redact(value: &str) -> String {
    if value
        .strip_prefix("{{")
        .and_then(|v| v.strip_suffix("}}"))
        .is_some_and(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        })
    {
        value.into()
    } else {
        String::new()
    }
}

fn safe_request(request: ApiSavedRequest) -> AppResult<NormalizedRequest> {
    let mut headers: Vec<KeyValue> = serde_json::from_str(&request.headers_json)?;
    let mut query: Vec<KeyValue> = serde_json::from_str(&request.query_json)?;
    for pair in headers.iter_mut().chain(query.iter_mut()) {
        if sensitive(&pair.key) {
            pair.value = redact(&pair.value);
        }
    }
    let mut auth: Value = serde_json::from_str(&request.auth_json)?;
    redact_auth(&mut auth);
    // Saved multipart definitions have no runtime paths; validate before export.
    if request.body_kind == "multipart-form-data" {
        unfour_core::models::parse_multipart_definition(request.body.as_deref())?;
    }
    Ok(NormalizedRequest {
        parent_source_id: request.parent_folder_id,
        name: request.name,
        method: request.method,
        url: super::super::domain::export_url(&request.url),
        headers,
        query,
        body: super::super::domain::export_body(request.body.as_deref(), &request.body_kind),
        body_kind: request.body_kind,
        auth_json: serde_json::to_string(&auth)?,
        settings_json: request.settings_json,
        pre_request_script: request.pre_request_script,
        post_response_script: request.post_response_script,
        script_schema_version: request.script_schema_version,
        sort_order: request.sort_order,
    })
}

fn redact_auth(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if !["type", "key", "username", "addTo"].contains(&key.as_str()) {
                    if let Some(text) = value.as_str() {
                        *value = Value::String(redact(text));
                    } else {
                        redact_auth(value);
                    }
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_auth(item);
            }
        }
        Value::String(text) => *text = redact(text),
        _ => {}
    }
}

impl ApiClientService {
    pub async fn export_collection_exchange(
        &self,
        workspace_id: String,
        collection_id: String,
        format: ApiCollectionExportFormat,
    ) -> AppResult<ApiCollectionExportArtifact> {
        validate_workspace_id(&workspace_id)?;
        let collection = self.get_collection(&workspace_id, &collection_id).await?;
        let folders = self
            .list_collection_folders(workspace_id.clone(), Some(collection_id.clone()))
            .await?;
        let requests = self
            .list_saved_requests(workspace_id)
            .await?
            .into_iter()
            .filter(|r| r.collection_id == collection_id)
            .map(safe_request)
            .collect::<AppResult<Vec<_>>>()?;
        let parsed = NormalizedCollection {
            name: collection.name,
            description: collection.description,
            folders: folders
                .into_iter()
                .map(|f| NormalizedFolder {
                    source_id: f.id,
                    parent_source_id: f.parent_folder_id,
                    name: f.name,
                    sort_order: f.sort_order,
                })
                .collect(),
            requests,
        };
        let (document, suffix) = match format {
            ApiCollectionExportFormat::Unfour => (
                serde_json::json!({"format":"unfour.collection", "version":1, "collection":parsed}),
                "unfour",
            ),
            ApiCollectionExportFormat::Postman => (postman::encode(&parsed)?, "postman_collection"),
            _ => return Err(import_validation("invalid exchange export format")),
        };
        Ok(ApiCollectionExportArtifact {
            content: serde_json::to_string_pretty(&document)?,
            media_type: "application/json".into(),
            suggested_file_name: format!("collection.{suffix}.json"),
        })
    }
}
