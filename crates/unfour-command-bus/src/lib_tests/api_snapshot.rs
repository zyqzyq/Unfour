use super::*;

#[tokio::test]
async fn api_script_snapshot_and_history_hide_derived_values_but_wire_receives_them() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Script environment".into())
        .await
        .unwrap();
    bus.workspace_environment_set_active(workspace.clone(), Some(environment.id))
        .await
        .unwrap();
    let (url, received) = spawn_api_test_server();
    let mut input = api_script_test_input(workspace.clone(), url);
    input.auth_json = Some(r#"{"type":"bearer","token":"auth-canary"}"#.into());
    input.temporary_variables = vec![KeyValue {
        key: "ordinary".into(),
        value: "temporary-canary".into(),
        enabled: true,
    }];
    input.query = vec![KeyValue {
        key: "note".into(),
        value: "{{ordinary}}".into(),
        enabled: true,
    }];
    input.query.push(KeyValue {
        key: "extra".into(),
        value: "{{runtimeSlot}}".into(),
        enabled: true,
    });
    input.pre_request_script = Some(
        r#"
pm.request.method = "POST";
pm.environment.set("runtimeSlot", "derived-env-canary");
pm.request.url = pm.request.url + "?sig=derived-url-canary";
pm.request.headers.upsert({key: "X-Opaque", value: "derived-header-canary"});
pm.request.body.raw = '{"ordinary":"derived-body-canary"}';
"#
        .into(),
    );
    let response = bus
        .send_api_request_with_scripts(input)
        .await
        .unwrap()
        .response
        .unwrap();
    let wire = received
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert!(wire.starts_with("POST /echo?sig=derived-url-canary&note=temporary-canary"));
    assert!(wire.contains("derived-header-canary"));
    assert!(wire.contains("derived-env-canary"));
    assert!(wire.contains("auth-canary"));
    let snapshot = response.request_snapshot.unwrap();
    assert_eq!(snapshot.method, "POST");
    assert_eq!(snapshot.body.as_deref(), Some("<redacted>"));
    let serialized = serde_json::to_string(&snapshot).unwrap();
    assert!(!serialized.contains("canary"), "{serialized}");
    let history = bus
        .api_history_detail(workspace, response.history_id)
        .await
        .unwrap();
    let serialized = serde_json::to_string(&history).unwrap();
    assert!(!serialized.contains("canary"), "{serialized}");
    assert!(!serialized.contains("pm.request"));
}

#[tokio::test]
async fn api_missing_auth_and_saved_redacted_credentials_fail_before_network() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    for auth in [
        r#"{"type":"bearer","token":""}"#,
        r#"{"type":"api-key","key":"credential","value":"","addTo":"query"}"#,
        r#"{"type":"bearer","token":"<redacted>"}"#,
    ] {
        let mut input = api_script_test_input(workspace.clone(), "http://127.0.0.1:1/never".into());
        input.auth_json = Some(auth.into());
        let error = bus
            .send_api_request(input.clone())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("API_AUTH_MISSING") || error.contains("API_REDACTED_REQUEST"));
        let saved = bus.save_api_request(input).await.unwrap();
        let result = bus
            .execute_saved_api_request_with_scripts_in_workspace(
                Some(workspace.clone()),
                &saved.id,
                None,
                None,
            )
            .await;
        match result {
            Err(error) => assert!(error.to_string().contains("API_AUTH_MISSING")),
            Ok(result) => assert!(result.http_error.unwrap().contains("API_REDACTED_REQUEST")),
        }
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_history")
        .fetch_one(bus.db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}
