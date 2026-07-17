use super::content::{
    append_url_query, build_parameters, build_request_body, build_responses, parse_key_values,
};
use super::model::*;
use super::source::{
    active_environment_variables, build_folder_paths, build_server, build_tags, document_security,
    environment_extension, infer_shared_auth, operation_security, parse_auth, prepare_request,
    register_security_scheme,
};
use std::collections::{BTreeMap, BTreeSet};
use unfour_core::{AppError, AppResult};

const HTTP_METHODS: [&str; 8] = [
    "delete", "get", "head", "options", "patch", "post", "put", "trace",
];

pub(super) fn build_document(source: &OpenApiExportSource) -> AppResult<OpenApiDocument> {
    let folder_paths = build_folder_paths(source);
    let tags = build_tags(source, &folder_paths);
    let prepared = source
        .requests
        .iter()
        .map(prepare_request)
        .collect::<Vec<_>>();

    let explicit_collection_auth = source.collection_auth_json.as_deref().and_then(parse_auth);
    let inferred_collection_auth = if explicit_collection_auth.is_none() {
        infer_shared_auth(&prepared)
    } else {
        None
    };
    let collection_auth = explicit_collection_auth
        .clone()
        .or_else(|| inferred_collection_auth.clone());

    let mut security_schemes = BTreeMap::new();
    if let Some(auth) = &collection_auth {
        register_security_scheme(&mut security_schemes, auth);
    }
    for request in &prepared {
        if let Some(auth) = &request.auth {
            register_security_scheme(&mut security_schemes, auth);
        }
    }

    let explicit_server = source
        .collection_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let distinct_servers = prepared
        .iter()
        .filter_map(|request| request.server.clone())
        .collect::<BTreeSet<_>>();
    let root_server = explicit_server.or_else(|| {
        (distinct_servers.len() == 1)
            .then(|| distinct_servers.iter().next().cloned())
            .flatten()
    });
    let active_variables = active_environment_variables(&source.environments);
    let servers = root_server
        .as_deref()
        .map(|server| vec![build_server(server, &active_variables)])
        .unwrap_or_default();

    let mut paths = BTreeMap::new();
    let mut warnings = Vec::new();
    for request in prepared {
        let (headers, raw_headers) = parse_key_values(&request.request.headers_json);
        let (mut query, raw_query) = parse_key_values(&request.request.query_json);
        append_url_query(&request.request.url, &mut query);
        let operation_servers = if root_server.is_none() && distinct_servers.len() > 1 {
            request
                .server
                .as_deref()
                .map(|server| vec![build_server(server, &active_variables)])
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let security = operation_security(collection_auth.as_ref(), request.auth.as_ref());
        let mut extensions = BTreeMap::from([(
            "x-unfour-request-id".to_string(),
            serde_json::json!(request.request.id),
        )]);
        extensions.insert(
            "x-unfour-original-url".to_string(),
            serde_json::json!(request.request.url),
        );
        if request.original_path != request.openapi_path {
            extensions.insert(
                "x-unfour-original-path".to_string(),
                serde_json::json!(request.original_path),
            );
        }
        if let Some(folder_id) = &request.request.parent_folder_id {
            extensions.insert(
                "x-unfour-folder-id".to_string(),
                serde_json::json!(folder_id),
            );
            if let Some(path) = folder_paths.get(folder_id) {
                extensions.insert("x-unfour-folder-path".to_string(), serde_json::json!(path));
            }
        }
        if !request.template_variables.is_empty() {
            extensions.insert(
                "x-unfour-template-variables".to_string(),
                serde_json::json!(request.template_variables),
            );
        }
        if let Some(raw) = raw_headers {
            extensions.insert(
                "x-unfour-raw-headers-json".to_string(),
                serde_json::json!(raw),
            );
        }
        if let Some(raw) = raw_query {
            extensions.insert(
                "x-unfour-raw-query-json".to_string(),
                serde_json::json!(raw),
            );
        }

        let operation = OpenApiOperation {
            tags: request
                .request
                .parent_folder_id
                .as_ref()
                .and_then(|id| folder_paths.get(id))
                .cloned()
                .into_iter()
                .collect(),
            summary: request.request.name.clone(),
            description: None,
            parameters: build_parameters(&request, &headers, &query),
            request_body: build_request_body(request.request, &headers),
            responses: build_responses(request.request, &source.histories),
            security,
            servers: operation_servers,
            extensions,
        };

        let method = request.request.method.trim().to_ascii_lowercase();
        let path_item = paths
            .entry(request.openapi_path.clone())
            .or_insert_with(OpenApiPathItem::default);
        if HTTP_METHODS.contains(&method.as_str()) {
            if let Some(existing) = path_item.operations.get_mut(&method) {
                append_conflicting_operation(existing, &operation)?;
                warnings.push(format!(
                    "{} {} is defined by more than one request; additional requests are preserved in x-unfour-conflicting-operations",
                    method.to_ascii_uppercase(),
                    request.openapi_path
                ));
            } else {
                path_item.operations.insert(method, operation);
            }
        } else {
            append_extension_array(
                &mut path_item.extensions,
                "x-unfour-unsupported-operations",
                serde_json::to_value(operation)?,
            );
            warnings.push(format!(
                "unsupported HTTP method {} is preserved as an extension",
                request.request.method
            ));
        }
    }

    let mut extensions = BTreeMap::from([(
        "x-unfour-collection-id".to_string(),
        serde_json::json!(source.collection.id),
    )]);
    if inferred_collection_auth.is_some() {
        extensions.insert(
            "x-unfour-collection-security-inferred".to_string(),
            serde_json::json!(true),
        );
    }
    if !source.environments.is_empty() {
        extensions.insert(
            "x-unfour-environments".to_string(),
            environment_extension(&source.environments),
        );
    }
    if !warnings.is_empty() {
        warnings.sort();
        warnings.dedup();
        extensions.insert(
            "x-unfour-export-warnings".to_string(),
            serde_json::json!(warnings),
        );
    }

    Ok(OpenApiDocument {
        openapi: "3.1.0".to_string(),
        info: OpenApiInfo {
            title: source.collection.name.clone(),
            description: source
                .collection
                .description
                .clone()
                .filter(|value| !value.trim().is_empty()),
            version: source
                .collection_version
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "1.0.0".to_string()),
        },
        servers,
        tags,
        paths,
        components: (!security_schemes.is_empty())
            .then_some(OpenApiComponents { security_schemes }),
        security: document_security(collection_auth.as_ref()),
        extensions,
    })
}

pub(super) fn serialize_document(
    document: &OpenApiDocument,
    format: unfour_core::models::ApiCollectionExportFormat,
) -> AppResult<String> {
    match format {
        unfour_core::models::ApiCollectionExportFormat::Json => {
            Ok(serde_json::to_string_pretty(document)?)
        }
        unfour_core::models::ApiCollectionExportFormat::Yaml => serde_yaml_ng::to_string(document)
            .map_err(|error| {
                AppError::Config(format!("OpenAPI YAML serialization failed: {error}"))
            }),
    }
}

pub(super) fn sanitize_file_name(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_separator = false;
    for ch in value.trim().chars() {
        let invalid =
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        let separator = invalid || ch.is_whitespace();
        if separator {
            if !last_was_separator && !output.is_empty() {
                output.push('-');
            }
            last_was_separator = true;
        } else {
            output.push(ch);
            last_was_separator = false;
        }
        if output.chars().count() >= 100 {
            break;
        }
    }
    let output = output.trim_matches([' ', '.', '-']).to_string();
    let reserved = matches!(
        output.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if output.is_empty() || reserved {
        "collection".to_string()
    } else {
        output
    }
}

fn append_conflicting_operation(
    existing: &mut OpenApiOperation,
    conflicting: &OpenApiOperation,
) -> AppResult<()> {
    append_extension_array(
        &mut existing.extensions,
        "x-unfour-conflicting-operations",
        serde_json::to_value(conflicting)?,
    );
    Ok(())
}

fn append_extension_array(
    extensions: &mut BTreeMap<String, serde_json::Value>,
    key: &str,
    value: serde_json::Value,
) {
    let values = extensions
        .entry(key.to_string())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    if let serde_json::Value::Array(values) = values {
        values.push(value);
    }
}
