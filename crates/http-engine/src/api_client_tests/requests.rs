use super::super::*;
use super::support::{save_in_collection, service};

#[tokio::test]
async fn saved_request_reopen_and_duplicate_preserve_timeout_settings() {
    let service = service().await;
    let saved = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Custom timeout".to_string()),
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "none".to_string(),
            timeout_ms: Some(0),
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            temporary_variables: vec![],
        })
        .await
        .expect("save request");

    assert_eq!(saved.settings_json, r#"{"timeoutMs":0}"#);
    let reopened = service
        .list_saved_requests("workspace-a".to_string())
        .await
        .expect("reopen request");
    assert_eq!(reopened[0].settings_json, saved.settings_json);

    let duplicated = service
        .duplicate_request("workspace-a".to_string(), saved.id)
        .await
        .expect("duplicate request");
    assert_eq!(duplicated.settings_json, r#"{"timeoutMs":0}"#);
}

#[tokio::test]
async fn save_request_preserves_non_json_body_unchanged() {
    let service = service().await;

    let plain_text = "plain text body with no json structure";
    let saved = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Plain text body".to_string()),
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method: "POST".to_string(),
            url: "https://example.test".to_string(),
            headers: vec![],
            query: vec![],
            body: Some(plain_text.to_string()),
            body_kind: "text".to_string(),
            timeout_ms: None,
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            temporary_variables: vec![],
        })
        .await
        .expect("save request");

    assert_eq!(saved.body.as_deref(), Some(plain_text));
}

#[tokio::test]
async fn history_detail_is_scoped_to_workspace() {
    let service = service().await;
    sqlx::query(
        r#"
        INSERT INTO api_history (
          id, workspace_id, name, method, url, request_headers_json, request_query_json,
          request_body, status, duration_ms, response_headers_json, response_body_preview,
          created_at, updated_at
        )
        VALUES (
          'history-a', 'workspace-a', 'Health', 'GET', 'https://example.test',
          '[]', '[]', NULL, 200, 12, '[]', '{}', ?1, ?1
        )
        "#,
    )
    .bind(Utc::now().to_rfc3339())
    .execute(service.db.pool())
    .await
    .expect("insert history");

    let detail = service
        .history_detail("workspace-a".to_string(), "history-a".to_string())
        .await
        .expect("load detail");
    let wrong_workspace = service
        .history_detail("workspace-b".to_string(), "history-a".to_string())
        .await;

    assert_eq!(detail.method, "GET");
    assert!(matches!(wrong_workspace, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn save_request_defaults_name_and_lists_by_workspace() {
    let service = service().await;
    service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: None,
            parent_folder_id: None,
            collection_id: None,
            auth_json: Some(r#"{"type":"bearer","token":"{{api_token}}"}"#.to_string()),
            method: "post".to_string(),
            url: "https://example.test/users".to_string(),
            headers: vec![],
            query: vec![],
            body: Some("{}".to_string()),
            body_kind: "json".to_string(),
            timeout_ms: None,
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            temporary_variables: vec![],
        })
        .await
        .expect("save request");
    service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-b".to_string(),
            name: Some("Other workspace".to_string()),
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method: "GET".to_string(),
            url: "https://other.example.test".to_string(),
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
        .expect("save other request");

    let workspace_a = service
        .list_saved_requests("workspace-a".to_string())
        .await
        .expect("list workspace a");

    assert_eq!(workspace_a.len(), 1);
    assert_eq!(workspace_a[0].workspace_id, "workspace-a");
    assert_eq!(workspace_a[0].parent_folder_id, None);
    assert_eq!(workspace_a[0].name, "POST https://example.test/users");
    assert_eq!(
        workspace_a[0].auth_json,
        r#"{"type":"bearer","token":"{{api_token}}"}"#
    );
}

#[tokio::test]
async fn duplicate_request_copies_template_inside_workspace() {
    let service = service().await;
    let saved = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Create user".to_string()),
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method: "POST".to_string(),
            url: "https://example.test/users".to_string(),
            headers: vec![KeyValue {
                key: "Accept".to_string(),
                value: "application/json".to_string(),
                enabled: true,
            }],
            query: vec![],
            body: Some("{}".to_string()),
            body_kind: "json".to_string(),
            timeout_ms: None,
            pre_request_script: None,
            post_response_script: None,
            script_schema_version: 1,
            temporary_variables: vec![],
        })
        .await
        .expect("save request");

    let duplicate = service
        .duplicate_request("workspace-a".to_string(), saved.id.clone())
        .await
        .expect("duplicate request");
    let wrong_workspace = service
        .duplicate_request("workspace-b".to_string(), saved.id.clone())
        .await;

    assert_ne!(duplicate.id, saved.id);
    assert_eq!(duplicate.name, "Create user Copy");
    assert_eq!(duplicate.parent_folder_id, saved.parent_folder_id);
    assert_eq!(duplicate.url, saved.url);
    assert!(matches!(wrong_workspace, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn delete_request_soft_deletes_and_returns_remaining_workspace_items() {
    let service = service().await;
    let first = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("First".to_string()),
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test/first".to_string(),
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
        .expect("save first request");
    let second = service
        .save_request(ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Second".to_string()),
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
            method: "GET".to_string(),
            url: "https://example.test/second".to_string(),
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
        .expect("save second request");

    let remaining = service
        .delete_request("workspace-a".to_string(), first.id.clone())
        .await
        .expect("delete request");
    let deleted_again = service
        .delete_request("workspace-a".to_string(), first.id)
        .await;

    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, second.id);
    assert!(matches!(deleted_again, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn move_request_reassigns_collection_and_folder() {
    let service = service().await;
    let collection_a = service
        .create_collection("workspace-a".to_string(), "APIs".to_string())
        .await
        .expect("create collection A");
    let collection_b = service
        .create_collection("workspace-a".to_string(), "Other".to_string())
        .await
        .expect("create collection B");
    let request = save_in_collection(
        &service,
        "workspace-a",
        "Movable",
        Some(collection_a.id.clone()),
    )
    .await;
    assert_eq!(request.collection_id, collection_a.id);

    let target_folder = service
        .create_collection_folder(
            "workspace-a".to_string(),
            collection_b.id.clone(),
            None,
            "Sub".to_string(),
        )
        .await
        .expect("create target folder");

    let moved = service
        .move_request(
            "workspace-a".to_string(),
            request.id.clone(),
            Some(collection_b.id.clone()),
            Some(target_folder.id.clone()),
        )
        .await
        .expect("move into collection B");
    assert_eq!(moved.collection_id, collection_b.id);
    assert_eq!(
        moved.parent_folder_id.as_deref(),
        Some(target_folder.id.as_str())
    );

    // Moving with None moves the request to the first collection.
    let moved_to_first = service
        .move_request("workspace-a".to_string(), request.id.clone(), None, None)
        .await
        .expect("move to first collection");
    assert_eq!(moved_to_first.collection_id, collection_a.id);

    // Moving into a collection that does not exist is rejected.
    let missing = service
        .move_request(
            "workspace-a".to_string(),
            request.id,
            Some("does-not-exist".to_string()),
            None,
        )
        .await;
    assert!(matches!(missing, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn update_request_reuses_existing_record_and_validates_collection() {
    let service = service().await;
    let collection = service
        .create_collection("workspace-a".to_string(), "APIs".to_string())
        .await
        .expect("create collection");
    let collection_id = collection.id.clone();
    let request = save_in_collection(
        &service,
        "workspace-a",
        "Original",
        Some(collection_id.clone()),
    )
    .await;

    let updated = service
        .update_request(
            "workspace-a".to_string(),
            request.id.clone(),
            ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Updated".to_string()),
                parent_folder_id: None,
                collection_id: None,
                auth_json: None,
                method: "POST".to_string(),
                url: "https://example.test/updated".to_string(),
                headers: vec![],
                query: vec![],
                body: Some("{}".to_string()),
                body_kind: "json".to_string(),
                timeout_ms: None,
                pre_request_script: None,
                post_response_script: None,
                script_schema_version: 1,
                temporary_variables: vec![],
            },
        )
        .await
        .expect("update request");

    assert_eq!(updated.id, request.id);
    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.method, "POST");
    assert_eq!(updated.collection_id, collection_id);
    assert_eq!(updated.parent_folder_id, None);

    let saved = service
        .list_saved_requests("workspace-a".to_string())
        .await
        .expect("list saved");
    assert_eq!(saved.len(), 1);

    let missing_collection = service
        .update_request(
            "workspace-a".to_string(),
            request.id,
            ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Bad collection".to_string()),
                parent_folder_id: None,
                collection_id: Some("does-not-exist".to_string()),
                auth_json: None,
                method: "GET".to_string(),
                url: "https://example.test".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
                pre_request_script: None,
                post_response_script: None,
                script_schema_version: 1,
                temporary_variables: vec![],
            },
        )
        .await;
    assert!(matches!(missing_collection, Err(AppError::NotFound(_))));
}
