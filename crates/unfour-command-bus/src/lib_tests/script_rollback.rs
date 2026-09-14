use super::*;

#[tokio::test]
async fn saved_mcp_execution_id_cancels_http_and_cleans_up_registry() {
    use std::io::{BufRead, BufReader, Read};
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/pending", listener.local_addr().unwrap());
    let (sent, received) = tokio::sync::oneshot::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(socket.try_clone().unwrap());
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        sent.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(socket.read(&mut byte).unwrap(), 0);
    });
    let mut input = api_script_test_input(workspace_id.clone(), url);
    input.timeout_ms = Some(0);
    let saved = bus.save_api_request(input).await.unwrap();
    let execution = bus.execute_saved_api_request_with_scripts_controlled_in_workspace(
        "mcp-cancel-test",
        Some(workspace_id.clone()),
        &saved.id,
        Some(0),
        None,
    );
    tokio::pin!(execution);
    tokio::select! {
        _=received=>{},
        result=&mut execution=>panic!("request should still be waiting: {result:?}"),
        _=tokio::time::sleep(std::time::Duration::from_secs(5))=>panic!("request did not reach the isolated HTTP server"),
    }
    assert!(bus.cancel_api_request("mcp-cancel-test"));
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), execution)
        .await
        .unwrap()
        .unwrap();
    assert!(result.response.is_none());
    assert!(result.http_error.is_some());
    assert!(!bus.cancel_api_request("mcp-cancel-test"));
    tokio::task::spawn_blocking(move || server.join().unwrap())
        .await
        .unwrap();
    assert!(bus
        .list_api_history(workspace_id, Some(10))
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn failed_post_script_preserves_pre_commit_http_history_and_redacts_secret_output() {
    let bus = test_bus().await;
    let workspace_id = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Script rollback".into())
        .await
        .unwrap();
    bus.workspace_environment_variable_create(
        workspace_id.clone(),
        environment.id.clone(),
        WorkspaceVariableInput {
            id: None,
            key: "token".into(),
            value: "original-secret-canary".into(),
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
        },
    )
    .await
    .unwrap();
    bus.workspace_environment_set_active(workspace_id.clone(), Some(environment.id.clone()))
        .await
        .unwrap();
    let (url, request) = spawn_api_test_server();
    let mut input = api_script_test_input(workspace_id.clone(), url);
    input.pre_request_script = Some(r#"pm.environment.set("token", "pre-secret-canary")"#.into());
    input.post_response_script = Some(
        r#"
        const original = pm.environment.get("token");
        pm.environment.set("token", "post-secret-canary");
        pm.environment.set("partial_new_key", "must-rollback");
        console.log(original);
        throw new Error(original);
    "#
        .into(),
    );
    let result = bus.send_api_request_with_scripts(input).await.unwrap();
    request
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(result.pre_request.status, ScriptExecutionStatus::Success);
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Failed);
    assert_eq!(result.response.as_ref().unwrap().status, 200);
    let environments = bus
        .workspace_environments_list(workspace_id.clone())
        .await
        .unwrap();
    let persisted = environments
        .iter()
        .find(|item| item.id == environment.id)
        .unwrap();
    assert_eq!(
        persisted.variables.len(),
        1,
        "partial post writes must not be persisted"
    );
    assert_eq!(persisted.variables[0].value, "pre-secret-canary");
    assert!(persisted.variables[0].is_secret);
    let history = bus
        .list_api_history(workspace_id.clone(), Some(10))
        .await
        .unwrap();
    assert_eq!(
        history.len(),
        1,
        "post failure cannot erase or duplicate completed HTTP history"
    );
    let detail = bus
        .api_history_detail(
            workspace_id.clone(),
            result.response.as_ref().unwrap().history_id.clone(),
        )
        .await
        .unwrap();
    let activity = bus
        .activity_log
        .list_recent(Some(&workspace_id), 20)
        .await
        .unwrap();
    let activity_details = activity
        .iter()
        .map(|entry| &entry.details_json)
        .collect::<Vec<_>>();
    let visible = serde_json::to_string(&(result, detail, activity_details)).unwrap();
    for canary in [
        "original-secret-canary",
        "pre-secret-canary",
        "post-secret-canary",
    ] {
        assert!(
            !visible.contains(canary),
            "secret leaked through script/history/activity: {canary}"
        );
    }
}
