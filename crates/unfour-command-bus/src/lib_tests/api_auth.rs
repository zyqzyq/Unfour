use super::*;

#[tokio::test]
async fn openapi_export_matches_real_auth_sends_with_sensitive_variable_references() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for (key, value, is_secret) in [
        ("private_value", "resolution-canary-value", true),
        ("auth_slot", "X-Custom-Credential", false),
        ("public_page", "2", false),
    ] {
        bus.workspace_variable_create(
            workspace.clone(),
            WorkspaceVariableInput {
                id: None,
                key: key.into(),
                value: value.into(),
                is_secret,
                is_enabled: true,
                description: None,
                sort_order: 0,
            },
        )
        .await
        .unwrap();
    }
    for (index, (auth, matches, public_variable)) in [
        (
            serde_json::json!({"type":"bearer","token":"{{private_value}}"}),
            true,
            false,
        ),
        (
            serde_json::json!({"type":"basic","username":"user","password":"{{private_value}}"}),
            true,
            false,
        ),
        (
            serde_json::json!({"type":"api-key","addTo":"header","key":"X-Custom-Credential","value":"{{private_value}}"}),
            true,
            false,
        ),
        (
            serde_json::json!({"type":"api-key","addTo":"query","key":"credential","value":"{{private_value}}"}),
            true,
            false,
        ),
        // Export cannot reconstruct historical public values or auth slot names.
        (
            serde_json::json!({"type":"api-key","addTo":"header","key":"{{auth_slot}}","value":"{{private_value}}"}),
            false,
            false,
        ),
        (
            serde_json::json!({"type":"bearer","token":"{{private_value}}"}),
            false,
            true,
        ),
    ].into_iter().enumerate() {
        let collection = bus
            .api_collection_create(workspace.clone(), format!("Auth round trip {index}"))
            .await
            .unwrap();
        let (url, received) = spawn_api_test_server();
        let mut input = api_script_test_input(workspace.clone(), url);
        input.name = Some("Auth round trip".into());
        input.collection_id = Some(collection.id.clone());
        input.auth_json = Some(auth.to_string());
        input.headers.push(KeyValue {
            key: "X-Ordinary".into(),
            value: "{{private_value}}".into(),
            enabled: true,
        });
        input.query.push(KeyValue {
            key: "note".into(),
            value: "{{private_value}}".into(),
            enabled: true,
        });
        if public_variable {
            input.query.push(KeyValue {
                key: "page".into(),
                value: "{{public_page}}".into(),
                enabled: true,
            });
        }
        let saved = bus.save_api_request(input.clone()).await.unwrap();
        let response = bus.send_api_request(input).await.unwrap();
        let wire = received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(
            wire.contains("resolution-canary-value"),
            "the loopback server must receive the resolved value"
        );
        let artifact = bus
            .api_collection_export(
                workspace.clone(),
                collection.id,
                ApiCollectionExportFormat::Json,
            )
            .await
            .unwrap();
        assert!(!artifact.content.contains("resolution-canary-value"));
        assert!(!artifact
            .content
            .contains("dXNlcjpyZXNvbHV0aW9uLWNhbmFyeS12YWx1ZQ=="));
        let document: serde_json::Value = serde_json::from_str(&artifact.content).unwrap();
        let responses = &document["paths"]["/echo"]["get"]["responses"];
        if matches {
            assert_eq!(responses["200"]["x-unfour-history-id"], response.history_id);
        } else {
            assert!(responses["default"].is_object());
        }
        let after = bus
            .list_saved_api_requests(workspace.clone())
            .await
            .unwrap();
        let after = after.iter().find(|request| request.id == saved.id).unwrap();
        assert!(after.auth_json == saved.auth_json);
        let history = bus
            .api_history_detail(workspace.clone(), response.history_id)
            .await
            .unwrap();
        assert!(!history
            .request_headers_json
            .contains("resolution-canary-value"));
        assert!(!history
            .request_query_json
            .contains("resolution-canary-value"));
        let activity: Vec<String> =
            sqlx::query_scalar("SELECT details_json FROM activity_events WHERE workspace_id=?")
                .bind(&workspace)
                .fetch_all(bus.db.pool())
                .await
                .unwrap();
        assert!(!activity.join("").contains("resolution-canary-value"));
    }
}

#[tokio::test]
async fn auth_is_materialized_after_workspace_environment_and_temporary_resolution() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for (key, value) in [
        ("who", "user"),
        ("pw", "pass"),
        ("token", "workspace-token"),
        ("key", "X-Custom-Credential"),
    ] {
        bus.workspace_variable_create(
            workspace.clone(),
            WorkspaceVariableInput {
                id: None,
                key: key.into(),
                value: value.into(),
                is_secret: true,
                is_enabled: true,
                description: None,
                sort_order: 0,
            },
        )
        .await
        .unwrap();
    }
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Auth environment".into())
        .await
        .unwrap();
    bus.workspace_environment_variables_replace(
        workspace.clone(),
        environment.id.clone(),
        vec![WorkspaceVariableInput {
            id: None,
            key: "token".into(),
            value: "environment-token".into(),
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
        }],
    )
    .await
    .unwrap();
    bus.workspace_environment_set_active(workspace.clone(), Some(environment.id))
        .await
        .unwrap();
    for (auth, expected) in [
        (
            serde_json::json!({"type":"basic","username":"{{who}}","password":"{{pw}}"}),
            "Basic dXNlcjpwYXNz",
        ),
        (
            serde_json::json!({"type":"bearer","token":"{{token}}"}),
            "Bearer environment-token",
        ),
        (
            serde_json::json!({"type":"api-key","addTo":"header","key":"{{key}}","value":"{{token}}"}),
            "environment-token",
        ),
    ] {
        let mut input = api_script_test_input(workspace.clone(), "http://localhost/".into());
        input.auth_json = Some(auth.to_string());
        let resolved = bus.resolve_api_request_input(input.clone()).await.unwrap();
        assert_eq!(resolved.headers.len(), 1);
        assert_eq!(resolved.headers[0].value, expected);
        input.temporary_variables.push(KeyValue {
            key: "token".into(),
            value: "temporary-token".into(),
            enabled: true,
        });
        input.headers.push(KeyValue {
            key: resolved.headers[0].key.to_lowercase(),
            value: "manual".into(),
            enabled: true,
        });
        let resolved = bus.resolve_api_request_input(input).await.unwrap();
        assert_eq!(resolved.headers.len(), 1);
        assert_eq!(resolved.headers[0].value, "manual");
    }
    let mut input = api_script_test_input(workspace.clone(), "http://localhost/".into());
    input.auth_json = Some(
        serde_json::json!({"type":"api-key","addTo":"query","key":"{{key}}","value":"{{token}}"})
            .to_string(),
    );
    input.temporary_variables.push(KeyValue {
        key: "token".into(),
        value: "temporary-token".into(),
        enabled: true,
    });
    let resolved = bus.resolve_api_request_input(input).await.unwrap();
    assert_eq!(resolved.query.len(), 1);
    assert_eq!(resolved.query[0].key, "X-Custom-Credential");
    assert_eq!(resolved.query[0].value, "temporary-token");
}

#[tokio::test]
async fn desktop_and_saved_script_sends_apply_auth_once_and_keep_secrets_out_of_history() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    bus.workspace_variable_create(
        workspace.clone(),
        WorkspaceVariableInput {
            id: None,
            key: "private_value".into(),
            value: "short".into(),
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
        },
    )
    .await
    .unwrap();
    for saved_script in [false, true] {
        let (url, received) = spawn_api_test_server();
        let mut input = api_script_test_input(workspace.clone(), url);
        input.auth_json = Some(
            serde_json::json!({"type":"basic","username":"user","password":"{{private_value}}"})
                .to_string(),
        );
        input.headers.push(KeyValue {
            key: "X-Ordinary".into(),
            value: "{{private_value}}".into(),
            enabled: true,
        });
        input.query.push(KeyValue {
            key: "note".into(),
            value: "{{private_value}}".into(),
            enabled: true,
        });
        let response = if saved_script {
            let saved = bus.save_api_request(input).await.unwrap();
            bus.execute_saved_api_request_with_scripts_in_workspace(
                Some(workspace.clone()),
                &saved.id,
                None,
                None,
            )
            .await
            .unwrap()
            .response
            .unwrap()
        } else {
            bus.send_api_request(input).await.unwrap()
        };
        let request = received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert_eq!(request.to_lowercase().matches("authorization:").count(), 1);
        assert!(request.contains("Basic dXNlcjpzaG9ydA=="));
        assert!(request.contains("short"));
        let row: (String, String, String) = sqlx::query_as(
            "SELECT url, request_headers_json, request_query_json FROM api_history WHERE id=?",
        )
        .bind(&response.history_id)
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
        let stored = format!("{}{}{}", row.0, row.1, row.2);
        assert!(!stored.contains("short"));
        assert!(!stored.contains("dXNlcjpzaG9ydA=="));
        let activity: Vec<String> =
            sqlx::query_scalar("SELECT details_json FROM activity_events WHERE workspace_id=?")
                .bind(&workspace)
                .fetch_all(bus.db.pool())
                .await
                .unwrap();
        assert!(!activity.join("").contains("short"));
    }
}

#[tokio::test]
async fn invalid_runtime_url_and_headers_never_return_secret_input_in_errors() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for (url, header, code) in [
        ("http://invalid:secret-canary@", None, "API_URL_INVALID"),
        (
            "http://localhost/",
            Some(("invalid secret-canary", "value")),
            "API_HEADER_NAME_INVALID",
        ),
        (
            "http://localhost/",
            Some(("X-Header", "secret-canary\nvalue")),
            "API_HEADER_VALUE_INVALID",
        ),
    ] {
        let mut input = api_script_test_input(workspace.clone(), url.into());
        if let Some((key, value)) = header {
            input.headers.push(KeyValue {
                key: key.into(),
                value: value.into(),
                enabled: true,
            });
        }
        let error = bus.send_api_request(input).await.unwrap_err().to_string();
        assert!(error.contains(code));
        assert!(!error.contains("secret-canary"));
    }
}

#[tokio::test]
async fn resolved_request_and_secret_provenance_keep_the_same_value_after_a_concurrent_edit() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let variable = bus
        .workspace_variable_create(
            workspace.clone(),
            WorkspaceVariableInput {
                id: None,
                key: "private_value".into(),
                value: "before-edit-secret".into(),
                is_secret: true,
                is_enabled: true,
                description: None,
                sort_order: 0,
            },
        )
        .await
        .unwrap();
    let (url, received) = spawn_api_test_server();
    let mut input = api_script_test_input(workspace.clone(), url);
    input.headers.push(KeyValue {
        key: "X-Ordinary".into(),
        value: "{{private_value}}".into(),
        enabled: true,
    });
    let (resolved, secrets) = bus
        .resolve_api_request_input_for_environment(input.clone(), None)
        .await
        .unwrap();
    bus.workspace_variable_update(
        workspace.clone(),
        variable.id,
        WorkspaceVariableInput {
            id: None,
            key: "private_value".into(),
            value: "after-edit-secret".into(),
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(resolved.headers[0].value, "before-edit-secret");
    assert!(secrets.contains(&"before-edit-secret".into()));
    assert!(!secrets.contains(&"after-edit-secret".into()));
    let response = bus
        .api_client
        .send_cancellable_with_secrets(
            resolved,
            tokio_util::sync::CancellationToken::new(),
            &secrets,
        )
        .await
        .unwrap();
    assert!(received
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
        .contains("before-edit-secret"));
    let stored: String =
        sqlx::query_scalar("SELECT request_headers_json FROM api_history WHERE id=?")
            .bind(response.history_id)
            .fetch_one(bus.db.pool())
            .await
            .unwrap();
    assert!(!stored.contains("before-edit-secret"));
    input.temporary_variables.push(KeyValue {
        key: "private_value".into(),
        value: "temporary-secret".into(),
        enabled: true,
    });
    let (resolved, secrets) = bus
        .resolve_api_request_input_for_environment(input, None)
        .await
        .unwrap();
    assert_eq!(resolved.headers[0].value, "temporary-secret");
    assert!(secrets.contains(&"temporary-secret".into()));
}
