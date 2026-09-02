use super::*;

async fn create_environment(
    bus: &CommandBus,
    workspace_id: &str,
    name: &str,
    variables: Vec<(&str, String)>,
) -> WorkspaceEnvironment {
    let environment = bus
        .workspace_environment_create(workspace_id.to_string(), name.to_string())
        .await
        .expect("create environment");
    bus.workspace_environment_update(
        workspace_id.to_string(),
        environment.id,
        name.to_string(),
        variables
            .into_iter()
            .enumerate()
            .map(|(index, (key, value))| WorkspaceVariableInput {
                id: None,
                key: key.to_string(),
                value,
                is_secret: false,
                is_enabled: true,
                description: None,
                sort_order: i64::try_from(index).unwrap(),
            })
            .collect(),
    )
    .await
    .expect("seed environment")
}

#[tokio::test]
async fn ad_hoc_send_uses_per_call_environment_without_changing_active_environment() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let dev = create_environment(
        &bus,
        &workspace_id,
        "Dev",
        vec![("BASE_URL", "http://dev.example.invalid".to_string())],
    )
    .await;
    let (server_url, request) = spawn_api_test_server();
    let test = create_environment(
        &bus,
        &workspace_id,
        "Test",
        vec![("BASE_URL", server_url.trim_end_matches("/echo").to_string())],
    )
    .await;
    bus.workspace_environment_set_active(workspace_id.clone(), Some(dev.id.clone()))
        .await
        .unwrap();

    let input = api_script_test_input(workspace_id.clone(), "{{BASE_URL}}/echo".to_string());
    let response = bus
        .send_api_request_in_environment(input, Some(test.id.clone()))
        .await
        .expect("send with explicit environment");
    let outbound = request
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("explicit environment should reach test server");

    assert_eq!(response.status, 200);
    assert!(
        outbound.starts_with("GET /echo? HTTP/1.1"),
        "unexpected outbound request: {outbound:?}"
    );
    let environments = bus.workspace_environments_list(workspace_id).await.unwrap();
    assert!(environments
        .iter()
        .any(|environment| environment.id == dev.id && environment.is_active));
    assert!(environments
        .iter()
        .any(|environment| environment.id == test.id && !environment.is_active));
}

#[tokio::test]
async fn saved_replay_runs_scripts_against_the_same_per_call_environment() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let dev = create_environment(
        &bus,
        &workspace_id,
        "Dev",
        vec![
            ("BASE_URL", "http://dev.example.invalid".to_string()),
            ("MARKER", "dev".to_string()),
        ],
    )
    .await;
    let (server_url, request) = spawn_api_test_server();
    let test = create_environment(
        &bus,
        &workspace_id,
        "Test",
        vec![
            ("BASE_URL", server_url.trim_end_matches("/echo").to_string()),
            ("MARKER", "test".to_string()),
        ],
    )
    .await;
    bus.workspace_environment_set_active(workspace_id.clone(), Some(dev.id.clone()))
        .await
        .unwrap();

    let mut input = api_script_test_input(workspace_id.clone(), "{{BASE_URL}}/echo".to_string());
    input.name = Some("Scripted replay".to_string());
    input.pre_request_script = Some(
        r#"
pm.request.headers.upsert({ key: "X-Environment", value: pm.environment.get("MARKER") });
pm.environment.set("PRE_MUTATION", "from-pre");
"#
        .to_string(),
    );
    input.post_response_script = Some(
        r#"
pm.environment.set("POST_MUTATION", "from-post");
pm.test("response status", () => pm.response.to.have.status(200));
"#
        .to_string(),
    );
    let saved = bus
        .save_api_request(input)
        .await
        .expect("save scripted request");

    let result = bus
        .execute_saved_api_request_with_scripts_in_workspace(
            Some(workspace_id.clone()),
            &saved.id,
            Some(2_000),
            Some(test.id.clone()),
        )
        .await
        .expect("execute scripted saved request with explicit environment");
    let outbound = request
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("scripted request should reach test server");

    assert_eq!(result.pre_request.status, ScriptExecutionStatus::Success);
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Success);
    assert_eq!(result.response.expect("HTTP response").status, 200);
    assert!(
        outbound.starts_with("GET /echo? HTTP/1.1"),
        "unexpected outbound request: {outbound:?}"
    );
    assert!(outbound
        .to_ascii_lowercase()
        .contains("x-environment: test"));

    let environments = bus
        .workspace_environments_list(workspace_id.clone())
        .await
        .unwrap();
    let persisted_dev = environments
        .iter()
        .find(|environment| environment.id == dev.id)
        .unwrap();
    let persisted_test = environments
        .iter()
        .find(|environment| environment.id == test.id)
        .unwrap();
    assert!(persisted_dev.is_active);
    assert!(!persisted_test.is_active);
    assert!(persisted_dev
        .variables
        .iter()
        .all(|variable| variable.key != "PRE_MUTATION" && variable.key != "POST_MUTATION"));
    assert!(persisted_test
        .variables
        .iter()
        .any(|variable| { variable.key == "PRE_MUTATION" && variable.value == "from-pre" }));
    assert!(persisted_test
        .variables
        .iter()
        .any(|variable| { variable.key == "POST_MUTATION" && variable.value == "from-post" }));

    let error = bus
        .execute_saved_api_request_with_scripts_in_workspace(
            Some(workspace_id),
            &saved.id,
            Some(2_000),
            Some("missing-environment".to_string()),
        )
        .await
        .expect_err("missing explicit environment must not fall back to active");
    assert_eq!(error.code(), "NOT_FOUND");
}
