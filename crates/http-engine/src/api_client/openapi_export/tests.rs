use super::convert::{build_document, sanitize_file_name, serialize_document};
use super::model::OpenApiExportSource;
use unfour_core::models::{
    ApiCollection, ApiCollectionExportFormat, ApiCollectionFolder, ApiEnvironment,
    ApiHistoryDetail, ApiSavedRequest, KeyValue,
};

fn source(requests: Vec<ApiSavedRequest>) -> OpenApiExportSource {
    OpenApiExportSource {
        collection: ApiCollection {
            id: "collection-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            name: "Public API".to_string(),
            description: Some("Public endpoints".to_string()),
            created_at: "2026-07-17T00:00:00Z".to_string(),
            updated_at: "2026-07-17T00:00:00Z".to_string(),
        },
        collection_auth_json: None,
        collection_base_url: None,
        collection_version: None,
        environments: Vec::new(),
        folders: Vec::new(),
        histories: Vec::new(),
        requests,
    }
}

fn request(id: &str, method: &str, url: &str) -> ApiSavedRequest {
    ApiSavedRequest {
        id: id.to_string(),
        workspace_id: "workspace-1".to_string(),
        name: format!("{method} request"),
        collection_id: "collection-1".to_string(),
        parent_folder_id: None,
        sort_order: 0,
        auth_json: r#"{"type":"none"}"#.to_string(),
        method: method.to_string(),
        url: url.to_string(),
        headers_json: "[]".to_string(),
        query_json: "[]".to_string(),
        body: None,
        body_kind: "none".to_string(),
        created_at: "2026-07-17T00:00:00Z".to_string(),
        updated_at: "2026-07-17T00:00:00Z".to_string(),
        deleted_at: None,
        revision: 1,
        sync_status: "local".to_string(),
        remote_id: None,
    }
}

fn folder(id: &str, parent_folder_id: Option<&str>, name: &str) -> ApiCollectionFolder {
    ApiCollectionFolder {
        id: id.to_string(),
        workspace_id: "workspace-1".to_string(),
        collection_id: "collection-1".to_string(),
        parent_folder_id: parent_folder_id.map(str::to_string),
        name: name.to_string(),
        sort_order: 0,
        created_at: "2026-07-17T00:00:00Z".to_string(),
        updated_at: "2026-07-17T00:00:00Z".to_string(),
        deleted_at: None,
        revision: 1,
        sync_status: "local".to_string(),
        remote_id: None,
    }
}

fn history(request: &ApiSavedRequest, id: &str, status: i64, body: &str) -> ApiHistoryDetail {
    ApiHistoryDetail {
        id: id.to_string(),
        workspace_id: request.workspace_id.clone(),
        name: Some(request.name.clone()),
        method: request.method.clone(),
        url: request.url.clone(),
        request_headers_json: request.headers_json.clone(),
        request_query_json: request.query_json.clone(),
        request_body: request.body.clone(),
        status: Some(status),
        duration_ms: Some(25),
        response_headers_json: serde_json::to_string(&vec![KeyValue {
            key: "Content-Type".to_string(),
            value: "application/json".to_string(),
            enabled: true,
        }])
        .expect("serialize response headers"),
        response_body_preview: Some(body.to_string()),
        created_at: "2026-07-17T00:00:00Z".to_string(),
        updated_at: "2026-07-17T00:00:00Z".to_string(),
    }
}

fn document_json(source: &OpenApiExportSource) -> serde_json::Value {
    serde_json::to_value(build_document(source).expect("build OpenAPI document"))
        .expect("serialize OpenAPI document")
}

#[test]
fn empty_collection_exports_valid_document() {
    let value = document_json(&source(Vec::new()));
    assert_eq!(value["openapi"], "3.1.0");
    assert_eq!(value["info"]["title"], "Public API");
    assert_eq!(value["info"]["description"], "Public endpoints");
    assert_eq!(value["info"]["version"], "1.0.0");
    assert_eq!(value["x-unfour-collection-id"], "collection-1");
    assert_eq!(value["paths"], serde_json::json!({}));
}

#[test]
fn get_request_maps_to_path_method_summary_and_server() {
    let value = document_json(&source(vec![request(
        "request-1",
        "GET",
        "https://api.example.test/v1/users",
    )]));
    assert_eq!(value["servers"][0]["url"], "https://api.example.test");
    assert_eq!(value["paths"]["/v1/users"]["get"]["summary"], "GET request");
    assert_eq!(
        value["paths"]["/v1/users"]["get"]["x-unfour-request-id"],
        "request-1"
    );
}

#[test]
fn path_query_header_and_cookie_parameters_are_mapped() {
    let mut item = request(
        "request-1",
        "GET",
        "https://api.example.test/users/:user_id?expand=true",
    );
    item.query_json = serde_json::to_string(&vec![KeyValue {
        key: "page".to_string(),
        value: "2".to_string(),
        enabled: true,
    }])
    .expect("serialize query");
    item.headers_json = serde_json::to_string(&vec![
        KeyValue {
            key: "X-Trace".to_string(),
            value: "trace-1".to_string(),
            enabled: true,
        },
        KeyValue {
            key: "Cookie".to_string(),
            value: "session=secret; theme=dark".to_string(),
            enabled: true,
        },
    ])
    .expect("serialize headers");
    let value = document_json(&source(vec![item]));
    let parameters = value["paths"]["/users/{user_id}"]["get"]["parameters"]
        .as_array()
        .expect("parameters array");
    assert!(parameters
        .iter()
        .any(|parameter| parameter["name"] == "user_id" && parameter["in"] == "path"));
    assert!(parameters
        .iter()
        .any(|parameter| parameter["name"] == "page" && parameter["in"] == "query"));
    assert!(parameters
        .iter()
        .any(|parameter| parameter["name"] == "expand" && parameter["in"] == "query"));
    assert!(parameters
        .iter()
        .any(|parameter| parameter["name"] == "X-Trace" && parameter["in"] == "header"));
    assert!(parameters
        .iter()
        .any(|parameter| parameter["name"] == "session" && parameter["in"] == "cookie"));
}

#[test]
fn json_request_body_uses_one_media_model_and_redacts_sensitive_fields() {
    let mut item = request("request-1", "POST", "https://api.example.test/users");
    item.body_kind = "json".to_string();
    item.body = Some(r#"{"name":"Ada","token":"secret","age":37}"#.to_string());
    item.headers_json = serde_json::to_string(&vec![KeyValue {
        key: "Content-Type".to_string(),
        value: "application/json".to_string(),
        enabled: true,
    }])
    .expect("serialize headers");
    let value = document_json(&source(vec![item]));
    let media = &value["paths"]["/users"]["post"]["requestBody"]["content"]["application/json"];
    assert_eq!(media["schema"]["type"], "object");
    assert_eq!(media["schema"]["properties"]["age"]["type"], "integer");
    assert_eq!(media["example"]["name"], "Ada");
    assert_eq!(media["example"]["token"], "<redacted>");
}

#[test]
fn saved_response_statuses_headers_and_examples_are_exported() {
    let item = request("request-1", "GET", "https://api.example.test/users/1");
    let mut exported = source(vec![item.clone()]);
    exported.histories = vec![
        history(&item, "history-200", 200, r#"{"id":1}"#),
        history(&item, "history-404", 404, r#"{"error":"missing"}"#),
    ];
    let value = document_json(&exported);
    let responses = &value["paths"]["/users/1"]["get"]["responses"];
    assert_eq!(responses["200"]["description"], "OK");
    assert_eq!(responses["404"]["description"], "Not Found");
    assert_eq!(
        responses["200"]["content"]["application/json"]["example"]["id"],
        1
    );
    assert_eq!(responses["404"]["x-unfour-history-id"], "history-404");
}

#[test]
fn folder_and_nested_folder_paths_become_tags() {
    let mut item = request("request-1", "GET", "https://api.example.test/profile");
    item.parent_folder_id = Some("profile-folder".to_string());
    let mut exported = source(vec![item]);
    exported.folders = vec![
        folder("user-folder", None, "User"),
        folder("profile-folder", Some("user-folder"), "Profile"),
    ];
    let value = document_json(&exported);
    let tag_names = value["tags"]
        .as_array()
        .expect("tags")
        .iter()
        .map(|tag| tag["name"].as_str().expect("tag name"))
        .collect::<Vec<_>>();
    assert_eq!(tag_names, vec!["User", "User / Profile"]);
    assert_eq!(
        value["paths"]["/profile"]["get"]["tags"][0],
        "User / Profile"
    );
    assert_eq!(
        value["paths"]["/profile"]["get"]["x-unfour-folder-id"],
        "profile-folder"
    );
}

#[test]
fn collection_auth_and_request_auth_override_map_to_security() {
    let mut inherited = request("request-1", "GET", "https://api.example.test/users");
    inherited.auth_json = r#"{"type":"bearer","token":"secret"}"#.to_string();
    let mut overridden = request("request-2", "POST", "https://api.example.test/users");
    overridden.auth_json = r#"{"type":"basic","username":"ada","password":"secret"}"#.to_string();
    let mut public = request("request-3", "GET", "https://api.example.test/health");
    public.auth_json = r#"{"type":"none"}"#.to_string();
    let mut exported = source(vec![inherited, overridden, public]);
    exported.collection_auth_json =
        Some(r#"{"type":"bearer","token":"collection-secret"}"#.to_string());
    let value = document_json(&exported);
    assert_eq!(value["security"][0]["bearerAuth"], serde_json::json!([]));
    assert_eq!(
        value["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );
    assert!(value["paths"]["/users"]["get"].get("security").is_none());
    assert_eq!(
        value["paths"]["/users"]["post"]["security"][0]["basicAuth"],
        serde_json::json!([])
    );
    assert_eq!(
        value["paths"]["/health"]["get"]["security"],
        serde_json::json!([])
    );
    let serialized = serde_json::to_string(&value).expect("serialize value");
    assert!(!serialized.contains("collection-secret"));
    assert!(!serialized.contains("password"));
}

#[test]
fn identical_request_auth_is_promoted_to_collection_security() {
    let mut first = request("request-1", "GET", "https://api.example.test/users");
    first.auth_json = r#"{"type":"bearer","token":"one"}"#.to_string();
    let mut second = request("request-2", "GET", "https://api.example.test/teams");
    second.auth_json = r#"{"type":"bearer","token":"two"}"#.to_string();
    let value = document_json(&source(vec![first, second]));
    assert_eq!(value["security"][0]["bearerAuth"], serde_json::json!([]));
    assert_eq!(value["x-unfour-collection-security-inferred"], true);
}

#[test]
fn yaml_and_json_serialize_the_same_document() {
    let document = build_document(&source(vec![request(
        "request-1",
        "GET",
        "https://api.example.test/users",
    )]))
    .expect("build document");
    let json =
        serialize_document(&document, ApiCollectionExportFormat::Json).expect("serialize JSON");
    let yaml =
        serialize_document(&document, ApiCollectionExportFormat::Yaml).expect("serialize YAML");
    let json_value: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");
    let yaml_value: serde_json::Value = serde_yaml_ng::from_str(&yaml).expect("parse YAML");
    assert_eq!(json_value, yaml_value);
    assert!(yaml.starts_with("openapi: 3.1.0"));
}

#[test]
fn illegal_file_name_characters_and_reserved_names_are_sanitized() {
    assert_eq!(sanitize_file_name(" User: API / v1? * "), "User-API-v1");
    assert_eq!(sanitize_file_name("CON"), "collection");
    assert_eq!(sanitize_file_name("..."), "collection");
    assert_eq!(sanitize_file_name("用户 API"), "用户-API");
}

#[test]
fn environment_placeholders_are_preserved_with_unfour_extensions() {
    let item = request("request-1", "GET", "{{base_url}}/users/{{user_id}}");
    let mut exported = source(vec![item]);
    exported.environments = vec![ApiEnvironment {
        id: "environment-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        name: "Development".to_string(),
        variables: vec![
            KeyValue {
                key: "base_url".to_string(),
                value: "https://dev.example.test".to_string(),
                enabled: true,
            },
            KeyValue {
                key: "token".to_string(),
                value: "secret".to_string(),
                enabled: true,
            },
        ],
        is_active: true,
        created_at: "2026-07-17T00:00:00Z".to_string(),
        updated_at: "2026-07-17T00:00:00Z".to_string(),
    }];
    let value = document_json(&exported);
    assert_eq!(value["servers"][0]["url"], "{base_url}");
    assert_eq!(
        value["servers"][0]["variables"]["base_url"]["default"],
        "https://dev.example.test"
    );
    assert_eq!(
        value["paths"]["/users/{user_id}"]["get"]["x-unfour-original-url"],
        "{{base_url}}/users/{{user_id}}"
    );
    assert_eq!(
        value["x-unfour-environments"][0]["variables"][1]["value"],
        "<redacted>"
    );
}

#[test]
fn full_collection_fixture_exports_multiple_folders_and_requests() {
    let fixture = include_str!("fixtures/full_collection.json");
    let source: OpenApiExportSource =
        serde_json::from_str(fixture).expect("parse full collection fixture");
    let value = document_json(&source);
    assert_eq!(value["info"]["title"], "Commerce API");
    assert_eq!(value["info"]["version"], "2.4.0");
    assert!(value["paths"].get("/users/{user_id}").is_some());
    assert!(value["paths"].get("/orders/{order_id}/refunds").is_some());
    assert_eq!(
        value["paths"]["/orders/{order_id}/refunds"]["post"]["tags"][0],
        "Order / Refund"
    );
    assert_eq!(value["tags"].as_array().expect("tags").len(), 4);
    assert_eq!(value["paths"].as_object().expect("paths").len(), 3);
    assert!(!serde_json::to_string(&value)
        .expect("serialize fixture output")
        .contains("secret"));
    println!(
        "{}",
        serialize_document(
            &build_document(&source).expect("build fixture document"),
            ApiCollectionExportFormat::Yaml,
        )
        .expect("serialize fixture YAML")
    );
}
