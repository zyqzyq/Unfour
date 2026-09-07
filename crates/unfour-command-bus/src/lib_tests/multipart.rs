use super::*;
use unfour_core::models::ApiMultipartRuntimePart;

#[tokio::test]
async fn multipart_resolves_structured_text_and_keys_but_not_file_paths_or_names() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = api_script_test_input(workspace, "https://example.test".into());
    input.body_kind = "multipart-form-data".into();
    input.body = Some(
        serde_json::json!([
            {"id":"t","enabled":true,"key":"{{key}}","type":"text","value":"{{value}}"},
            {"id":"f","enabled":true,"key":"{{key}}","type":"file","fileName":"{{name}}.txt"}
        ])
        .to_string(),
    );
    input.temporary_variables = vec![
        KeyValue {
            key: "key".into(),
            value: "resolved".into(),
            enabled: true,
        },
        KeyValue {
            key: "value".into(),
            value: "quote\"and\\slash".into(),
            enabled: true,
        },
    ];
    input.multipart_parts = vec![ApiMultipartRuntimePart {
        id: "f".into(),
        file_path: "C:/{{home}}/file.txt".into(),
    }];
    let resolved = bus
        .resolve_api_request_input_for_environment(input, None)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_str(resolved.body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0]["key"], "resolved");
    assert_eq!(body[0]["value"], "quote\"and\\slash");
    assert_eq!(body[1]["key"], "resolved");
    assert_eq!(body[1]["fileName"], "{{name}}.txt");
    assert_eq!(
        resolved.multipart_parts[0].file_path,
        "C:/{{home}}/file.txt"
    );
}

#[tokio::test]
async fn multipart_pre_script_body_mutation_fails_before_send() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = api_script_test_input(workspace, "http://127.0.0.1:1".into());
    input.method = "POST".into();
    input.body_kind = "multipart-form-data".into();
    input.body = Some("[]".into());
    input.pre_request_script = Some("pm.request.body.raw = 'mutated';".into());
    let error = bus.send_api_request_with_scripts(input).await.unwrap_err();
    assert!(error.to_string().contains("Multipart body mutation"));
}

#[tokio::test]
async fn multipart_pre_script_headers_url_and_method_still_work() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let (url, request) = spawn_api_test_server();
    let mut input = api_script_test_input(workspace, url);
    input.body_kind = "multipart-form-data".into();
    input.body = Some("[]".into());
    input.pre_request_script = Some("pm.request.method = 'POST'; pm.request.headers.upsert({key:'X-Script',value:'yes'}); pm.request.url += '?script=yes';".into());
    let result = bus.send_api_request_with_scripts(input).await.unwrap();
    assert!(result.response.is_some(), "{result:?}");
    let outbound = request
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert!(outbound.starts_with("POST /echo?script=yes"));
    assert!(outbound.to_lowercase().contains("x-script: yes"));
}

#[tokio::test]
async fn saved_multipart_without_runtime_binding_fails_closed() {
    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut input = api_script_test_input(workspace, "http://127.0.0.1:1".into());
    input.method = "POST".into();
    input.body_kind = "multipart-form-data".into();
    input.body = Some(
        r#"[{"id":"file","enabled":true,"key":"avatar","type":"file","fileName":"avatar.png"}]"#
            .into(),
    );
    let saved = bus.save_api_request(input).await.unwrap();
    let error = bus
        .execute_saved_api_request(&saved.id, None)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("requires a local file selection"));
    let result = bus
        .execute_saved_api_request_with_scripts_in_workspace(
            Some(saved.workspace_id),
            &saved.id,
            None,
            None,
        )
        .await
        .unwrap();
    assert!(result.response.is_none());
    assert!(result
        .http_error
        .unwrap()
        .contains("requires a local file selection"));
}
