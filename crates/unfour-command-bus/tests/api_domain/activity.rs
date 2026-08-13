use super::*;

#[tokio::test]
async fn api_save_and_import_record_primary_activity_targets() {
    let (bus, db) = bus_with_hook(RecordingHook {
        local_only: false,
        fail_on: None,
    })
    .await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;

    let request = bus
        .save_api_request(ApiRequestInput {
            workspace_id: workspace_id.clone(),
            name: None,
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method: "get".to_string(),
            url: "https://api.example.test/ping".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            temporary_variables: vec![],
        })
        .await
        .unwrap();
    let collection_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM api_collections WHERE workspace_id = ?1 AND deleted_at IS NULL",
    )
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        collection_count, 1,
        "save must auto-create a default collection"
    );

    let save_hook_types: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT entity_type FROM api_hook_effects
        WHERE command_name = 'api.save_request'
        ORDER BY id
        "#,
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        save_hook_types,
        vec!["ApiCollection".to_string(), "ApiRequest".to_string()]
    );

    let (save_target, save_details): (Option<String>, String) = sqlx::query_as(
        r#"
        SELECT target, details_json FROM activity_events
        WHERE action = 'api.save_request'
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(save_target.as_deref(), Some(request.id.as_str()));
    let save_details: serde_json::Value = serde_json::from_str(&save_details).unwrap();
    assert_eq!(save_details["name"].as_str(), Some(request.name.as_str()));
    assert_eq!(save_details["method"], "GET");

    let duplicate = bus
        .duplicate_api_request(workspace_id.clone(), request.id.clone())
        .await
        .unwrap();
    let (duplicate_target, duplicate_details): (Option<String>, String) = sqlx::query_as(
        r#"
        SELECT target, details_json FROM activity_events
        WHERE action = 'api.duplicate_request'
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(duplicate_target.as_deref(), Some(duplicate.id.as_str()));
    let duplicate_details: serde_json::Value = serde_json::from_str(&duplicate_details).unwrap();
    assert_eq!(
        duplicate_details["sourceId"].as_str(),
        Some(request.id.as_str())
    );
    assert_eq!(
        duplicate_details["name"].as_str(),
        Some(duplicate.name.as_str())
    );

    let openapi = r#"{
      "openapi":"3.0.3",
      "info":{"title":"Imported API","version":"1"},
      "servers":[{"url":"https://api.example.test"}],
      "paths":{"/users":{"get":{"operationId":"listUsers","tags":["Users"]}}}
    }"#;
    let imported = bus
        .api_collection_import(workspace_id.clone(), openapi.to_string())
        .await
        .unwrap();
    assert!(imported.imported);
    assert_eq!(imported.folder_count, 1);
    assert_eq!(imported.request_count, 1);
    let collection = imported.collection.expect("imported collection");

    let (import_target, import_details): (Option<String>, String) = sqlx::query_as(
        r#"
        SELECT target, details_json FROM activity_events
        WHERE action = 'api.collection.import'
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(import_target.as_deref(), Some(collection.id.as_str()));
    let import_details: serde_json::Value = serde_json::from_str(&import_details).unwrap();
    assert_eq!(import_details["folderCount"], 1);
    assert_eq!(import_details["requestCount"], 1);
    assert_eq!(import_details["contentBytes"], openapi.len());
}
