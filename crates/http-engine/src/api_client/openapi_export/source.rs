use super::model::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use unfour_core::models::{ApiEnvironment, ApiSavedRequest};
use unfour_core::redaction::{is_sensitive_key, REDACTED_VALUE};

pub(super) fn export_sensitive_key(value: &str) -> bool {
    if is_sensitive_key(value) {
        return true;
    }
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    normalized.contains("password")
        || normalized.contains("passphrase")
        || normalized.contains("secret")
        || normalized.ends_with("token")
        || normalized.ends_with("api_key")
}

pub(super) fn build_folder_paths(source: &OpenApiExportSource) -> HashMap<String, String> {
    let folders = source
        .folders
        .iter()
        .map(|folder| (folder.id.as_str(), folder))
        .collect::<HashMap<_, _>>();
    source
        .folders
        .iter()
        .map(|folder| {
            let mut names = Vec::new();
            let mut current = Some(folder.id.as_str());
            let mut visited = HashSet::new();
            while let Some(id) = current {
                if !visited.insert(id.to_string()) {
                    break;
                }
                let Some(item) = folders.get(id) else {
                    break;
                };
                names.push(item.name.clone());
                current = item.parent_folder_id.as_deref();
            }
            names.reverse();
            (folder.id.clone(), names.join(" / "))
        })
        .collect()
}

pub(super) fn build_tags(
    source: &OpenApiExportSource,
    folder_paths: &HashMap<String, String>,
) -> Vec<OpenApiTag> {
    let mut folders = source.folders.iter().collect::<Vec<_>>();
    folders.sort_by_key(|folder| {
        (
            folder_paths.get(&folder.id).cloned().unwrap_or_default(),
            folder.sort_order,
            folder.id.clone(),
        )
    });
    folders
        .into_iter()
        .filter_map(|folder| {
            let path = folder_paths.get(&folder.id)?.clone();
            Some(OpenApiTag {
                name: path.clone(),
                extensions: BTreeMap::from([
                    (
                        "x-unfour-folder-id".to_string(),
                        serde_json::json!(folder.id),
                    ),
                    ("x-unfour-folder-path".to_string(), serde_json::json!(path)),
                ]),
            })
        })
        .collect()
}

pub(super) fn prepare_request(request: &ApiSavedRequest) -> PreparedRequest<'_> {
    let (server, original_path) = split_url(&request.url);
    let (openapi_path, path_parameters) = normalize_path(&original_path);
    let mut template_variables = extract_template_variables(&request.url);
    if let Some(body) = &request.body {
        template_variables.extend(extract_template_variables(body));
    }
    template_variables.extend(extract_template_variables(&request.headers_json));
    template_variables.extend(extract_template_variables(&request.query_json));
    template_variables.sort();
    template_variables.dedup();
    PreparedRequest {
        request,
        auth: parse_auth(&request.auth_json),
        server,
        original_path,
        openapi_path,
        path_parameters,
        template_variables,
    }
}

fn split_url(raw: &str) -> (Option<String>, String) {
    let without_fragment = raw.trim().split('#').next().unwrap_or_default();
    let without_query = without_fragment.split('?').next().unwrap_or_default();
    if let Some(scheme_end) = without_query.find("://") {
        let authority_start = scheme_end + 3;
        let path_start = without_query[authority_start..]
            .find('/')
            .map(|index| authority_start + index);
        return match path_start {
            Some(index) => (
                Some(without_query[..index].to_string()),
                normalize_leading_slash(&without_query[index..]),
            ),
            None => (Some(without_query.to_string()), "/".to_string()),
        };
    }
    if without_query.starts_with("{{") {
        if let Some(end) = without_query.find("}}") {
            let suffix_start = end + 2;
            if suffix_start == without_query.len() || without_query[suffix_start..].starts_with('/')
            {
                return (
                    Some(without_query[..suffix_start].to_string()),
                    normalize_leading_slash(&without_query[suffix_start..]),
                );
            }
        }
    }
    (None, normalize_leading_slash(without_query))
}

fn normalize_leading_slash(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "/".to_string()
    } else if value.starts_with('/') {
        value.to_string()
    } else {
        format!("/{value}")
    }
}

fn normalize_path(path: &str) -> (String, Vec<String>) {
    let mut parameters = Vec::new();
    let segments = path
        .split('/')
        .map(|segment| {
            if let Some(name) = segment
                .strip_prefix(':')
                .filter(|name| valid_parameter_name(name))
            {
                parameters.push(name.to_string());
                return format!("{{{name}}}");
            }
            let mut output = segment.to_string();
            for name in extract_template_variables(segment) {
                output = output.replace(&format!("{{{{{name}}}}}"), &format!("{{{name}}}"));
                parameters.push(name);
            }
            if output.starts_with('{') && output.ends_with('}') && !output.starts_with("{{") {
                let name = &output[1..output.len().saturating_sub(1)];
                if valid_parameter_name(name) {
                    parameters.push(name.to_string());
                }
            }
            output
        })
        .collect::<Vec<_>>();
    parameters.dedup();
    (normalize_leading_slash(&segments.join("/")), parameters)
}

fn valid_parameter_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn extract_template_variables(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut remaining = value;
    while let Some(start) = remaining.find("{{") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        let name = after_start[..end].trim();
        if valid_parameter_name(name) {
            result.push(name.to_string());
        }
        remaining = &after_start[end + 2..];
    }
    result
}

pub(super) fn parse_auth(value: &str) -> Option<AuthDescriptor> {
    let value = serde_json::from_str::<serde_json::Value>(value).ok()?;
    match value.get("type")?.as_str()? {
        "bearer" => Some(AuthDescriptor::Bearer),
        "basic" => Some(AuthDescriptor::Basic),
        "api-key" => Some(AuthDescriptor::ApiKey {
            key: value
                .get("key")
                .and_then(serde_json::Value::as_str)
                .filter(|key| !key.trim().is_empty())
                .unwrap_or("X-API-Key")
                .to_string(),
            location: match value.get("addTo").and_then(serde_json::Value::as_str) {
                Some("query") => "query".to_string(),
                _ => "header".to_string(),
            },
        }),
        _ => None,
    }
}

pub(super) fn infer_shared_auth(prepared: &[PreparedRequest<'_>]) -> Option<AuthDescriptor> {
    let first = prepared.first()?.auth.clone()?;
    prepared
        .iter()
        .all(|request| request.auth.as_ref() == Some(&first))
        .then_some(first)
}

fn security_scheme_name(auth: &AuthDescriptor) -> String {
    match auth {
        AuthDescriptor::Bearer => "bearerAuth".to_string(),
        AuthDescriptor::Basic => "basicAuth".to_string(),
        AuthDescriptor::ApiKey { key, location } => {
            let key = key
                .chars()
                .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
                .collect::<String>()
                .trim_matches('_')
                .to_ascii_lowercase();
            format!(
                "apiKey_{location}_{}",
                if key.is_empty() { "key" } else { &key }
            )
        }
    }
}

pub(super) fn register_security_scheme(
    schemes: &mut BTreeMap<String, OpenApiSecurityScheme>,
    auth: &AuthDescriptor,
) {
    let name = security_scheme_name(auth);
    schemes.entry(name).or_insert_with(|| {
        let mut extensions = BTreeMap::new();
        extensions.insert(
            "x-unfour-credential-values-omitted".to_string(),
            serde_json::json!(true),
        );
        match auth {
            AuthDescriptor::Bearer => OpenApiSecurityScheme {
                scheme_type: "http".to_string(),
                scheme: Some("bearer".to_string()),
                bearer_format: Some("Bearer token".to_string()),
                location: None,
                name: None,
                description:
                    "Bearer authentication exported from Unfour. Credential values are omitted."
                        .to_string(),
                extensions,
            },
            AuthDescriptor::Basic => OpenApiSecurityScheme {
                scheme_type: "http".to_string(),
                scheme: Some("basic".to_string()),
                bearer_format: None,
                location: None,
                name: None,
                description:
                    "HTTP Basic authentication exported from Unfour. Credential values are omitted."
                        .to_string(),
                extensions,
            },
            AuthDescriptor::ApiKey { key, location } => OpenApiSecurityScheme {
                scheme_type: "apiKey".to_string(),
                scheme: None,
                bearer_format: None,
                location: Some(location.clone()),
                name: Some(key.clone()),
                description:
                    "API key authentication exported from Unfour. Credential values are omitted."
                        .to_string(),
                extensions,
            },
        }
    });
}

fn security_requirement(auth: &AuthDescriptor) -> SecurityRequirement {
    BTreeMap::from([(security_scheme_name(auth), Vec::new())])
}

pub(super) fn operation_security(
    collection_auth: Option<&AuthDescriptor>,
    request_auth: Option<&AuthDescriptor>,
) -> Option<Vec<SecurityRequirement>> {
    match (collection_auth, request_auth) {
        (Some(collection), Some(request)) if collection == request => None,
        (Some(_), Some(request)) => Some(vec![security_requirement(request)]),
        (Some(_), None) => Some(Vec::new()),
        (None, Some(request)) => Some(vec![security_requirement(request)]),
        (None, None) => None,
    }
}

pub(super) fn document_security(
    collection_auth: Option<&AuthDescriptor>,
) -> Option<Vec<SecurityRequirement>> {
    collection_auth.map(|auth| vec![security_requirement(auth)])
}

pub(super) fn active_environment_variables(
    environments: &[ApiEnvironment],
) -> HashMap<String, String> {
    environments
        .iter()
        .find(|environment| environment.is_active)
        .into_iter()
        .flat_map(|environment| environment.variables.iter())
        .filter(|variable| variable.enabled && !export_sensitive_key(&variable.key))
        .map(|variable| (variable.key.clone(), variable.value.clone()))
        .collect()
}

pub(super) fn build_server(raw: &str, active_variables: &HashMap<String, String>) -> OpenApiServer {
    let mut url = raw.to_string();
    let mut variables = BTreeMap::new();
    for name in extract_template_variables(raw) {
        let placeholder = format!("{{{{{name}}}}}");
        url = url.replace(&placeholder, &format!("{{{name}}}"));
        variables.insert(
            name.clone(),
            OpenApiServerVariable {
                default: active_variables
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| placeholder.clone()),
                extensions: BTreeMap::from([(
                    "x-unfour-original-placeholder".to_string(),
                    serde_json::json!(placeholder),
                )]),
            },
        );
    }
    OpenApiServer {
        url,
        variables,
        extensions: BTreeMap::from([("x-unfour-original-url".to_string(), serde_json::json!(raw))]),
    }
}

pub(super) fn environment_extension(environments: &[ApiEnvironment]) -> serde_json::Value {
    serde_json::Value::Array(
        environments
            .iter()
            .map(|environment| {
                serde_json::json!({
                    "id": environment.id,
                    "name": environment.name,
                    "isActive": environment.is_active,
                    "variables": environment.variables.iter().map(|variable| {
                        let redacted = export_sensitive_key(&variable.key);
                        serde_json::json!({
                            "key": variable.key,
                            "value": if redacted { REDACTED_VALUE } else { variable.value.as_str() },
                            "enabled": variable.enabled,
                            "redacted": redacted,
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}
