use super::*;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use unfour_core::models::{ApiCollectionImportResult, KeyValue};

mod value_parser;
use value_parser::{parse_auth_json, parse_parameters, parse_request_body};

const MAX_IMPORT_BYTES: usize = 10 * 1024 * 1024;
const MAX_IMPORT_ITEMS: usize = 10_000;
const HTTP_METHODS: [&str; 8] = [
    "delete", "get", "head", "options", "patch", "post", "put", "trace",
];

#[derive(Debug)]
struct ParsedImport {
    name: String,
    description: Option<String>,
    folders: Vec<ParsedFolder>,
    requests: Vec<ParsedRequest>,
}

#[derive(Debug)]
struct ParsedFolder {
    source_id: String,
    parent_source_id: Option<String>,
    name: String,
    sort_order: i64,
}

#[derive(Debug)]
struct ParsedRequest {
    parent_source_id: Option<String>,
    name: String,
    method: String,
    url: String,
    headers: Vec<KeyValue>,
    query: Vec<KeyValue>,
    body: Option<String>,
    body_kind: String,
    auth_json: String,
    sort_order: i64,
}

#[derive(Debug)]
struct FolderTag {
    source_id: String,
    path: String,
    sort_order: i64,
}

impl ApiClientService {
    pub async fn import_collection_openapi(
        &self,
        workspace_id: String,
        content: String,
    ) -> AppResult<ApiCollectionImportResult> {
        validate_workspace_id(&workspace_id)?;
        if content.len() > MAX_IMPORT_BYTES {
            return Err(import_validation("collection import file is too large"));
        }
        let parsed = parse_import(&content)?;
        let now = Utc::now().to_rfc3339();
        let collection_id = unfour_core::id::new_id();
        let mut tx = self.db.pool().begin().await?;

        let workspace_exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM workspaces WHERE id = ?1")
                .bind(&workspace_id)
                .fetch_optional(&mut *tx)
                .await?;
        if workspace_exists.is_none() {
            return Err(AppError::NotFound("workspace".to_string()));
        }

        sqlx::query(
            r#"
            INSERT INTO api_collections (
              id, workspace_id, name, description, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1, 'local')
            "#,
        )
        .bind(&collection_id)
        .bind(&workspace_id)
        .bind(&parsed.name)
        .bind(&parsed.description)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        let mut imported_folder_ids: HashMap<String, String> = HashMap::new();
        let mut pending_folders = parsed.folders.iter().collect::<Vec<_>>();
        while !pending_folders.is_empty() {
            let mut inserted = 0usize;
            let mut remaining = Vec::new();
            for folder in pending_folders {
                let parent_folder_id = match folder.parent_source_id.as_deref() {
                    Some(source_id) => match imported_folder_ids.get(source_id) {
                        Some(id) => Some(id.clone()),
                        None => {
                            remaining.push(folder);
                            continue;
                        }
                    },
                    None => None,
                };
                let id = unfour_core::id::new_id();
                sqlx::query(
                    r#"
                    INSERT INTO api_collection_folders (
                      id, workspace_id, collection_id, parent_folder_id, name,
                      sort_order, created_at, updated_at, revision, sync_status
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 1, 'local')
                    "#,
                )
                .bind(&id)
                .bind(&workspace_id)
                .bind(&collection_id)
                .bind(parent_folder_id)
                .bind(&folder.name)
                .bind(folder.sort_order)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
                imported_folder_ids.insert(folder.source_id.clone(), id);
                inserted += 1;
            }
            if inserted == 0 {
                return Err(import_validation(
                    "collection import contains an invalid folder hierarchy",
                ));
            }
            pending_folders = remaining;
        }

        for request in &parsed.requests {
            let parent_folder_id = request
                .parent_source_id
                .as_ref()
                .map(|source_id| {
                    imported_folder_ids.get(source_id).cloned().ok_or_else(|| {
                        import_validation("collection import references an unknown folder")
                    })
                })
                .transpose()?;
            let id = unfour_core::id::new_id();
            sqlx::query(
                r#"
                INSERT INTO api_requests (
                  id, workspace_id, name, collection_id, parent_folder_id,
                  sort_order, auth_json, method, url, headers_json, query_json,
                  body, body_kind, created_at, updated_at, revision, sync_status
                )
                VALUES (
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                  ?14, ?14, 1, 'local'
                )
                "#,
            )
            .bind(id)
            .bind(&workspace_id)
            .bind(&request.name)
            .bind(&collection_id)
            .bind(parent_folder_id)
            .bind(request.sort_order)
            .bind(&request.auth_json)
            .bind(&request.method)
            .bind(&request.url)
            .bind(serde_json::to_string(&request.headers)?)
            .bind(serde_json::to_string(&request.query)?)
            .bind(&request.body)
            .bind(&request.body_kind)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        let collection = self.get_collection(&workspace_id, &collection_id).await?;
        Ok(ApiCollectionImportResult {
            imported: true,
            collection: Some(collection),
            folder_count: parsed.folders.len() as u32,
            request_count: parsed.requests.len() as u32,
        })
    }
}

fn parse_import(content: &str) -> AppResult<ParsedImport> {
    let document = serde_json::from_str::<Value>(content)
        .or_else(|_| serde_yaml_ng::from_str::<Value>(content))
        .map_err(|_| import_validation("collection import must be valid JSON or YAML"))?;
    let root = document
        .as_object()
        .ok_or_else(|| import_validation("collection import must be an OpenAPI object"))?;
    if !non_empty_string(root.get("openapi")).is_some_and(|version| version.starts_with("3.")) {
        return Err(import_validation(
            "collection import only supports OpenAPI 3.x documents",
        ));
    }

    let info = object_field(root, "info")?;
    let name = required_string(info, "title", "collection import title cannot be empty")?;
    let description = non_empty_string(info.get("description")).map(str::to_string);
    let (folders, tag_folder_ids) = parse_folders(root)?;
    let folder_ids = folders
        .iter()
        .map(|folder| folder.source_id.as_str())
        .collect::<HashSet<_>>();
    let requests = parse_requests(root, &folder_ids, &tag_folder_ids)?;
    if folders.len() + requests.len() > MAX_IMPORT_ITEMS {
        return Err(import_validation(
            "collection import contains too many items",
        ));
    }
    Ok(ParsedImport {
        name,
        description,
        folders,
        requests,
    })
}

fn parse_folders(
    root: &Map<String, Value>,
) -> AppResult<(Vec<ParsedFolder>, HashMap<String, String>)> {
    let tags = match root.get("tags") {
        None => &[][..],
        Some(Value::Array(tags)) => tags,
        Some(_) => return Err(import_validation("collection import tags must be an array")),
    };
    let mut seen_ids = HashSet::new();
    let mut folder_tags = Vec::with_capacity(tags.len());
    let mut tag_folder_ids = HashMap::new();
    for (index, tag) in tags.iter().enumerate() {
        let tag = tag
            .as_object()
            .ok_or_else(|| import_validation("collection import contains an invalid folder"))?;
        let tag_name = required_string(tag, "name", "collection import tag name cannot be empty")?;
        let source_id = non_empty_string(tag.get("x-unfour-folder-id"))
            .map(str::to_string)
            .unwrap_or_else(|| format!("tag:{tag_name}"));
        if !seen_ids.insert(source_id.clone()) {
            tag_folder_ids.entry(tag_name).or_insert(source_id);
            continue;
        }
        let path = non_empty_string(tag.get("x-unfour-folder-path"))
            .unwrap_or(&tag_name)
            .to_string();
        tag_folder_ids.insert(tag_name, source_id.clone());
        folder_tags.push(FolderTag {
            source_id,
            path,
            sort_order: index as i64,
        });
    }

    let paths = object_field(root, "paths")?;
    for path_item in paths.values().filter_map(Value::as_object) {
        for method in HTTP_METHODS {
            let Some(operation) = path_item.get(method).and_then(Value::as_object) else {
                continue;
            };
            add_operation_tags(
                operation,
                &mut folder_tags,
                &mut tag_folder_ids,
                &mut seen_ids,
            )?;
            if let Some(conflicts) = operation
                .get("x-unfour-conflicting-operations")
                .and_then(Value::as_array)
            {
                for conflict in conflicts.iter().filter_map(Value::as_object) {
                    add_operation_tags(
                        conflict,
                        &mut folder_tags,
                        &mut tag_folder_ids,
                        &mut seen_ids,
                    )?;
                }
            }
        }
    }

    let folders = folder_tags
        .iter()
        .map(|folder| {
            let parent = folder_tags
                .iter()
                .filter(|candidate| {
                    candidate.source_id != folder.source_id
                        && candidate.path.len() < folder.path.len()
                        && folder.path.starts_with(&format!("{} / ", candidate.path))
                })
                .max_by_key(|candidate| candidate.path.len());
            let name = parent
                .and_then(|parent| folder.path.strip_prefix(&format!("{} / ", parent.path)))
                .unwrap_or(&folder.path)
                .trim()
                .to_string();
            if name.is_empty() {
                return Err(import_validation(
                    "collection import folder name cannot be empty",
                ));
            }
            Ok(ParsedFolder {
                source_id: folder.source_id.clone(),
                parent_source_id: parent.map(|parent| parent.source_id.clone()),
                name,
                sort_order: folder.sort_order,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok((folders, tag_folder_ids))
}

fn add_operation_tags(
    operation: &Map<String, Value>,
    folder_tags: &mut Vec<FolderTag>,
    tag_folder_ids: &mut HashMap<String, String>,
    seen_ids: &mut HashSet<String>,
) -> AppResult<()> {
    let Some(tags) = operation.get("tags") else {
        return Ok(());
    };
    let tags = tags
        .as_array()
        .ok_or_else(|| import_validation("collection import operation tags must be an array"))?;
    for tag in tags {
        let tag = non_empty_string(Some(tag))
            .ok_or_else(|| import_validation("collection import operation tag cannot be empty"))?;
        if tag_folder_ids.contains_key(tag) {
            continue;
        }
        let source_id = format!("tag:{tag}");
        tag_folder_ids.insert(tag.to_string(), source_id.clone());
        if seen_ids.insert(source_id.clone()) {
            folder_tags.push(FolderTag {
                source_id,
                path: tag.to_string(),
                sort_order: folder_tags.len() as i64,
            });
        }
    }
    Ok(())
}

fn parse_requests(
    root: &Map<String, Value>,
    folder_ids: &HashSet<&str>,
    tag_folder_ids: &HashMap<String, String>,
) -> AppResult<Vec<ParsedRequest>> {
    let paths = object_field(root, "paths")?;
    let mut requests = Vec::new();
    let mut seen_request_ids = HashSet::new();
    for (path, path_item) in paths {
        let path_item = path_item
            .as_object()
            .ok_or_else(|| import_validation("collection import contains an invalid path"))?;
        if path_item
            .get("x-unfour-unsupported-operations")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        {
            return Err(import_validation(
                "collection import contains unsupported HTTP methods",
            ));
        }
        for method in HTTP_METHODS {
            let Some(operation) = path_item.get(method) else {
                continue;
            };
            push_operation(
                root,
                path,
                path_item,
                operation,
                method,
                folder_ids,
                tag_folder_ids,
                &mut seen_request_ids,
                &mut requests,
            )?;
            if let Some(conflicts) = operation
                .get("x-unfour-conflicting-operations")
                .and_then(Value::as_array)
            {
                for conflict in conflicts {
                    push_operation(
                        root,
                        path,
                        path_item,
                        conflict,
                        method,
                        folder_ids,
                        tag_folder_ids,
                        &mut seen_request_ids,
                        &mut requests,
                    )?;
                }
            }
        }
    }
    Ok(requests)
}

fn push_operation(
    root: &Map<String, Value>,
    path: &str,
    path_item: &Map<String, Value>,
    operation: &Value,
    method: &str,
    folder_ids: &HashSet<&str>,
    tag_folder_ids: &HashMap<String, String>,
    seen_request_ids: &mut HashSet<String>,
    requests: &mut Vec<ParsedRequest>,
) -> AppResult<()> {
    let operation = operation
        .as_object()
        .ok_or_else(|| import_validation("collection import contains an invalid operation"))?;
    if non_empty_string(operation.get("x-unfour-request-id"))
        .is_some_and(|request_id| !seen_request_ids.insert(request_id.to_string()))
    {
        return Err(import_validation(
            "collection import contains duplicate request ids",
        ));
    }
    let name = non_empty_string(operation.get("summary"))
        .or_else(|| non_empty_string(operation.get("operationId")))
        .map(str::to_string)
        .unwrap_or_else(|| format!("{} {path}", method.to_ascii_uppercase()));
    let url = resolve_operation_url(root, operation, path);
    let parent_source_id = non_empty_string(operation.get("x-unfour-folder-id"))
        .map(str::to_string)
        .or_else(|| {
            operation
                .get("tags")
                .and_then(Value::as_array)
                .and_then(|tags| tags.first())
                .and_then(Value::as_str)
                .and_then(|tag| tag_folder_ids.get(tag))
                .cloned()
        });
    if parent_source_id
        .as_deref()
        .is_some_and(|folder_id| !folder_ids.contains(folder_id))
    {
        return Err(import_validation(
            "collection import references an unknown folder",
        ));
    }
    let (mut headers, query) = parse_parameters(path_item, operation)?;
    let (body, body_kind, content_type) = parse_request_body(operation)?;
    if let Some(content_type) = content_type {
        if !headers
            .iter()
            .any(|header| header.key.eq_ignore_ascii_case("content-type"))
        {
            headers.push(KeyValue {
                key: "Content-Type".to_string(),
                value: content_type,
                enabled: true,
            });
        }
    }
    let auth_json = parse_auth_json(root, operation)?;
    requests.push(ParsedRequest {
        parent_source_id,
        name,
        method: method.to_ascii_uppercase(),
        url,
        headers,
        query,
        body,
        body_kind,
        auth_json,
        sort_order: requests.len() as i64,
    });
    Ok(())
}

fn resolve_operation_url(
    root: &Map<String, Value>,
    operation: &Map<String, Value>,
    path: &str,
) -> String {
    if let Some(url) = non_empty_string(operation.get("x-unfour-original-url")) {
        return url.to_string();
    }
    let server_url = operation
        .get("servers")
        .and_then(Value::as_array)
        .and_then(|servers| servers.first())
        .and_then(Value::as_object)
        .and_then(|server| non_empty_string(server.get("url")))
        .or_else(|| {
            root.get("servers")
                .and_then(Value::as_array)
                .and_then(|servers| servers.first())
                .and_then(Value::as_object)
                .and_then(|server| non_empty_string(server.get("url")))
        });
    match server_url {
        Some(server_url) if path == "/" => format!("{}/", server_url.trim_end_matches('/')),
        Some(server_url) => format!(
            "{}/{}",
            server_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        ),
        None => path.to_string(),
    }
}

fn object_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> AppResult<&'a Map<String, Value>> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| import_validation(format!("collection import {key} must be an object")))
}

fn required_string(object: &Map<String, Value>, key: &str, error: &str) -> AppResult<String> {
    non_empty_string(object.get(key))
        .map(str::to_string)
        .ok_or_else(|| import_validation(error))
}

fn non_empty_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn import_validation(message: impl Into<String>) -> AppError {
    AppError::Validation(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_openapi_without_unfour_markers() {
        let parsed = parse_import(
            r#"{
              "openapi":"3.0.3",
              "info":{"title":"External","version":"1"},
              "servers":[{"url":"https://api.example.com/v1"}],
              "paths":{"/users":{"get":{"operationId":"listUsers","tags":["Users"]}}}
            }"#,
        )
        .expect("standard OpenAPI must be accepted");
        assert_eq!(parsed.name, "External");
        assert_eq!(parsed.folders[0].name, "Users");
        assert_eq!(parsed.requests[0].name, "listUsers");
        assert_eq!(parsed.requests[0].url, "https://api.example.com/v1/users");
    }

    #[test]
    fn parses_json_and_yaml_exports_equally() {
        let json = r#"{
          "openapi":"3.1.0",
          "info":{"title":"Users","description":"User endpoints","version":"1.0.0"},
          "tags":[{"name":"Admin","x-unfour-folder-id":"folder-1","x-unfour-folder-path":"Admin"}],
          "paths":{},
          "x-unfour-collection-id":"collection-1"
        }"#;
        let yaml = r#"
openapi: 3.1.0
info:
  title: Users
  description: User endpoints
  version: 1.0.0
tags:
  - name: Admin
    x-unfour-folder-id: folder-1
    x-unfour-folder-path: Admin
paths: {}
x-unfour-collection-id: collection-1
"#;
        let json = parse_import(json).expect("parse JSON");
        let yaml = parse_import(yaml).expect("parse YAML");
        assert_eq!(json.name, yaml.name);
        assert_eq!(json.description, yaml.description);
        assert_eq!(json.folders[0].name, yaml.folders[0].name);
    }
}
