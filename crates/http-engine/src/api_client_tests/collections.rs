use super::super::*;
use super::support::{save_in_collection, service};

#[tokio::test]
async fn collection_lifecycle_create_rename_delete_cascades_requests() {
    let service = service().await;
    let collection_a = service
        .create_collection("workspace-a".to_string(), "APIs".to_string())
        .await
        .expect("create collection A");
    let collection_b = service
        .create_collection("workspace-a".to_string(), "Other".to_string())
        .await
        .expect("create collection B");

    let in_collection_a = save_in_collection(
        &service,
        "workspace-a",
        "Inside A",
        Some(collection_a.id.clone()),
    )
    .await;
    assert_eq!(in_collection_a.collection_id, collection_a.id);
    let in_collection_b = save_in_collection(
        &service,
        "workspace-a",
        "Inside B",
        Some(collection_b.id.clone()),
    )
    .await;
    assert_eq!(in_collection_b.collection_id, collection_b.id);

    let renamed = service
        .rename_collection(
            "workspace-a".to_string(),
            collection_a.id.clone(),
            "Public APIs".to_string(),
        )
        .await
        .expect("rename collection");
    assert_eq!(renamed.name, "Public APIs");

    let remaining_collections = service
        .delete_collection("workspace-a".to_string(), collection_a.id.clone())
        .await
        .expect("delete collection");
    assert_eq!(remaining_collections.len(), 1);
    assert_eq!(remaining_collections[0].id, collection_b.id);

    // The request inside the deleted collection was cascade soft-deleted;
    // the request in the other collection survives.
    let saved = service
        .list_saved_requests("workspace-a".to_string())
        .await
        .expect("list saved");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, in_collection_b.id);

    let deleted_again = service
        .delete_collection("workspace-a".to_string(), collection_a.id)
        .await;
    assert!(matches!(deleted_again, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn folders_and_parent_folder_requests_drive_collection_tree() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".to_string(), "APIs".to_string())
        .await
        .expect("create collection");

    let root_request = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Root".to_string()),
            parent_folder_id: None,
            collection_id: Some(collection.id.clone()),
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test/root".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
        })
        .await
        .expect("save root request");
    assert_eq!(root_request.parent_folder_id, None);
    assert_eq!(root_request.collection_id, collection.id);

    let auth = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id.clone(),
            None,
            "Auth".to_string(),
        )
        .await
        .expect("create folder");
    let tokens = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id.clone(),
            Some(auth.id.clone()),
            "Tokens".to_string(),
        )
        .await
        .expect("create child folder");
    let folders = service
        .list_collection_folders("workspace-a".to_string(), Some(collection.id.clone()))
        .await
        .expect("list folders");
    assert_eq!(folders.len(), 2);
    assert!(folders.iter().any(|folder| folder.id == auth.id));
    assert!(folders.iter().any(|folder| folder.id == tokens.id));

    let child_request = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Token".to_string()),
            parent_folder_id: Some(tokens.id.clone()),
            collection_id: Some(collection.id.clone()),
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test/token".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
        })
        .await
        .expect("save child request");
    assert_eq!(
        child_request.parent_folder_id.as_deref(),
        Some(tokens.id.as_str())
    );
    assert_eq!(child_request.collection_id, collection.id);

    let renamed = service
        .rename_collection_folder(
            "workspace-a".to_string(),
            tokens.id.clone(),
            "Session tokens".to_string(),
        )
        .await
        .expect("rename folder");
    assert_eq!(renamed.name, "Session tokens");
    let after_rename = service
        .get_saved_request(&child_request.id)
        .await
        .expect("child request remains after folder rename");
    assert_eq!(
        after_rename.parent_folder_id.as_deref(),
        Some(tokens.id.as_str())
    );

    let moved = service
        .move_collection_folder("workspace-a".to_string(), tokens.id.clone(), None)
        .await
        .expect("move folder to root");
    assert_eq!(moved.parent_folder_id, None);
    let after_move = service
        .get_saved_request(&child_request.id)
        .await
        .expect("child request remains after folder move");
    assert_eq!(
        after_move.parent_folder_id.as_deref(),
        Some(tokens.id.as_str())
    );
}

#[tokio::test]
async fn folder_delete_recursively_soft_deletes_descendants_and_requests() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".to_string(), "APIs".to_string())
        .await
        .expect("create collection");
    let parent = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id.clone(),
            None,
            "Parent".to_string(),
        )
        .await
        .expect("create parent folder");
    let child = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id.clone(),
            Some(parent.id.clone()),
            "Child".to_string(),
        )
        .await
        .expect("create child folder");
    let request = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Nested".to_string()),
            parent_folder_id: Some(child.id.clone()),
            collection_id: Some(collection.id.clone()),
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test/nested".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
        })
        .await
        .expect("save nested request");

    service
        .delete_collection_folder("workspace-a".to_string(), parent.id.clone())
        .await
        .expect("delete parent recursively");

    let active_folders = service
        .list_collection_folders("workspace-a".to_string(), Some(collection.id))
        .await
        .expect("list active folders");
    let active_requests = service
        .list_saved_requests("workspace-a".to_string())
        .await
        .expect("list active requests");
    assert!(active_folders.is_empty());
    assert!(active_requests.iter().all(|saved| saved.id != request.id));

    let (deleted_folder_count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM api_collection_folders
        WHERE id IN (?1, ?2) AND deleted_at IS NOT NULL
        "#,
    )
    .bind(&parent.id)
    .bind(&child.id)
    .fetch_one(service.db.pool())
    .await
    .expect("count soft-deleted folders");
    let (deleted_request_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM api_requests WHERE id = ?1 AND deleted_at IS NOT NULL",
    )
    .bind(&request.id)
    .fetch_one(service.db.pool())
    .await
    .expect("count soft-deleted request");

    assert_eq!(deleted_folder_count, 2);
    assert_eq!(deleted_request_count, 1);
}

#[tokio::test]
async fn collection_delete_soft_deletes_folders_and_requests() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".to_string(), "APIs".to_string())
        .await
        .expect("create collection");
    let folder = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id.clone(),
            None,
            "Auth".to_string(),
        )
        .await
        .expect("create folder");
    let request = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Nested".to_string()),
            parent_folder_id: Some(folder.id.clone()),
            collection_id: Some(collection.id.clone()),
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test/nested".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
        })
        .await
        .expect("save request");

    service
        .delete_collection("workspace-a".to_string(), collection.id.clone())
        .await
        .expect("delete collection");

    let (deleted_folder_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM api_collection_folders WHERE id = ?1 AND deleted_at IS NOT NULL",
    )
    .bind(&folder.id)
    .fetch_one(service.db.pool())
    .await
    .expect("count soft-deleted folder");
    let (deleted_request_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM api_requests WHERE id = ?1 AND deleted_at IS NOT NULL",
    )
    .bind(&request.id)
    .fetch_one(service.db.pool())
    .await
    .expect("count soft-deleted request");

    assert_eq!(deleted_folder_count, 1);
    assert_eq!(deleted_request_count, 1);
}

#[tokio::test]
async fn folder_and_request_reorder_use_separate_sibling_sort_orders() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".to_string(), "APIs".to_string())
        .await
        .expect("create collection");
    let folder_b = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id.clone(),
            None,
            "B".to_string(),
        )
        .await
        .expect("create folder b");
    let folder_a = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id.clone(),
            None,
            "A".to_string(),
        )
        .await
        .expect("create folder a");
    let request_b = save_in_collection(
        &service,
        "workspace-a",
        "B Request",
        Some(collection.id.clone()),
    )
    .await;
    let request_a = save_in_collection(
        &service,
        "workspace-a",
        "A Request",
        Some(collection.id.clone()),
    )
    .await;

    let folders = service
        .reorder_collection_folders(
            "workspace-a".to_string(),
            collection.id.clone(),
            None,
            vec![folder_a.id.clone(), folder_b.id.clone()],
        )
        .await
        .expect("reorder folders");
    let requests = service
        .reorder_requests(
            "workspace-a".to_string(),
            collection.id.clone(),
            None,
            vec![request_a.id.clone(), request_b.id.clone()],
        )
        .await
        .expect("reorder requests");

    assert_eq!(
        folders
            .iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>(),
        vec![folder_a.id.as_str(), folder_b.id.as_str()]
    );
    assert_eq!(
        requests
            .iter()
            .map(|request| request.id.as_str())
            .collect::<Vec<_>>(),
        vec![request_a.id.as_str(), request_b.id.as_str()]
    );
}

#[tokio::test]
async fn moving_folder_to_self_or_descendant_is_rejected() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".to_string(), "APIs".to_string())
        .await
        .expect("create collection");
    let auth = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id.clone(),
            None,
            "Auth".to_string(),
        )
        .await
        .expect("create folder");
    let tokens = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection.id,
            Some(auth.id.clone()),
            "Tokens".to_string(),
        )
        .await
        .expect("create child folder");

    let into_self = service
        .move_collection_folder(
            "workspace-a".to_string(),
            auth.id.clone(),
            Some(auth.id.clone()),
        )
        .await;
    let into_child = service
        .move_collection_folder("workspace-a".to_string(), auth.id, Some(tokens.id))
        .await;

    assert!(matches!(into_self, Err(AppError::Validation(message)) if message.contains("cycle")));
    assert!(matches!(into_child, Err(AppError::Validation(message)) if message.contains("cycle")));
}

#[tokio::test]
async fn collection_is_scoped_to_workspace() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".to_string(), "Shared".to_string())
        .await
        .expect("create in a");

    let wrong = service
        .rename_collection(
            "workspace-b".to_string(),
            collection.id.clone(),
            "Renamed".to_string(),
        )
        .await;
    assert!(matches!(wrong, Err(AppError::NotFound(_))));

    let list_b = service
        .list_collections("workspace-b".to_string())
        .await
        .expect("list b");
    assert!(list_b.is_empty());

    let save_wrong_workspace = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-b".to_string(),
            name: Some("Wrong workspace".to_string()),
            parent_folder_id: None,
            collection_id: Some(collection.id),
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
        })
        .await;
    assert!(matches!(save_wrong_workspace, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn unfour_openapi_export_imports_as_a_new_workspace_scoped_collection() {
    let service = service().await;
    let source = service
        .create_collection("workspace-a".to_string(), "Commerce API".to_string())
        .await
        .expect("create source collection");
    let parent = service
        .create_collection_folder(
            "workspace-a".to_string(),
            source.id.clone(),
            None,
            "Orders".to_string(),
        )
        .await
        .expect("create parent folder");
    let child = service
        .create_collection_folder(
            "workspace-a".to_string(),
            source.id.clone(),
            Some(parent.id.clone()),
            "Refunds".to_string(),
        )
        .await
        .expect("create child folder");
    service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Create refund".to_string()),
            parent_folder_id: Some(child.id.clone()),
            collection_id: Some(source.id.clone()),
            auth_json: Some(serde_json::json!({ "type": "bearer", "token": "secret" }).to_string()),
            method: "POST".to_string(),
            url: "https://api.example.test/orders/1/refunds?notify=true".to_string(),
            headers: vec![KeyValue {
                key: "X-Trace".to_string(),
                value: "trace-1".to_string(),
                enabled: true,
            }],
            query: vec![KeyValue {
                key: "notify".to_string(),
                value: "true".to_string(),
                enabled: true,
            }],
            body: Some(r#"{"amount":1999}"#.to_string()),
            body_kind: "json".to_string(),
            timeout_ms: None,
        })
        .await
        .expect("save source request");

    let artifact = service
        .export_collection_openapi(
            "workspace-a".to_string(),
            source.id.clone(),
            unfour_core::models::ApiCollectionExportFormat::Json,
        )
        .await
        .expect("export collection");
    assert!(!artifact.content.contains("secret"));

    let result = service
        .import_collection_openapi("workspace-b".to_string(), artifact.content)
        .await
        .expect("import collection");
    assert!(result.imported);
    assert_eq!(result.folder_count, 2);
    assert_eq!(result.request_count, 1);
    let imported = result.collection.expect("imported collection");
    assert_ne!(imported.id, source.id);
    assert_eq!(imported.workspace_id, "workspace-b");
    assert_eq!(imported.name, "Commerce API");

    let folders = service
        .list_collection_folders("workspace-b".to_string(), Some(imported.id.clone()))
        .await
        .expect("list imported folders");
    let imported_parent = folders
        .iter()
        .find(|folder| folder.name == "Orders")
        .expect("imported parent folder");
    let imported_child = folders
        .iter()
        .find(|folder| folder.name == "Refunds")
        .expect("imported child folder");
    assert_eq!(
        imported_child.parent_folder_id.as_deref(),
        Some(imported_parent.id.as_str())
    );

    let requests = service
        .list_saved_requests("workspace-b".to_string())
        .await
        .expect("list imported requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].collection_id, imported.id);
    assert_eq!(
        requests[0].parent_folder_id.as_deref(),
        Some(imported_child.id.as_str())
    );
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].url,
        "https://api.example.test/orders/1/refunds?notify=true"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&requests[0].auth_json)
            .expect("parse imported auth"),
        serde_json::json!({ "type": "bearer", "token": "" })
    );
}

#[tokio::test]
async fn collection_import_accepts_standard_openapi_without_unfour_extensions() {
    let service = service().await;
    let external = r#"{
      "openapi":"3.0.3",
      "info":{"title":"External API","version":"1.0.0"},
      "servers":[{"url":"https://api.example.com/v1"}],
      "paths":{
        "/users":{
          "parameters":[{"name":"tenant","in":"header","example":"acme"}],
          "get":{
            "operationId":"listUsers",
            "tags":["Users"],
            "parameters":[{"name":"limit","in":"query","schema":{"example":25}}]
          }
        }
      }
    }"#;

    let result = service
        .import_collection_openapi("workspace-a".to_string(), external.to_string())
        .await
        .expect("standard OpenAPI must be imported");
    assert!(result.imported);
    assert_eq!(result.folder_count, 1);
    assert_eq!(result.request_count, 1);

    let collection = result.collection.expect("imported collection");
    assert_eq!(collection.name, "External API");
    let requests = service
        .list_saved_requests("workspace-a".to_string())
        .await
        .expect("list imported requests")
        .into_iter()
        .filter(|request| request.collection_id == collection.id)
        .collect::<Vec<_>>();
    assert_eq!(requests[0].name, "listUsers");
    assert_eq!(requests[0].url, "https://api.example.com/v1/users");
    let headers = serde_json::from_str::<Vec<KeyValue>>(&requests[0].headers_json)
        .expect("parse imported headers");
    let query = serde_json::from_str::<Vec<KeyValue>>(&requests[0].query_json)
        .expect("parse imported query");
    assert_eq!(headers[0].key, "tenant");
    assert_eq!(query[0].value, "25");
}
