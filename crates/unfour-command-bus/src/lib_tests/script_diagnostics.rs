use super::*;

#[tokio::test]
async fn script_diagnostics_are_safe_across_phases_while_wire_and_environment_keep_real_values() {
    use std::io::{Read, Write};

    let bus = test_bus().await;
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let environment = bus
        .workspace_environment_create(workspace.clone(), "Diagnostics".into())
        .await
        .unwrap();
    bus.workspace_environment_set_active(workspace.clone(), Some(environment.id.clone()))
        .await
        .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/echo", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut buffer = [0; 4096];
            let n = socket.read(&mut buffer).unwrap();
            assert!(n > 0);
            request.extend_from_slice(&buffer[..n]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .unwrap();
        String::from_utf8(request).unwrap()
    });
    let mut input = api_script_test_input(workspace.clone(), url);
    input.method = "POST".into();
    input.body_kind = "json".into();
    input.body = Some(r#"{"token":"existing-body-canary","ordinary":"{{slot}}"}"#.into());
    input.auth_json = Some(r#"{"type":"bearer","token":"existing-auth-canary"}"#.into());
    input.query = vec![KeyValue {
        key: "opaque".into(),
        value: "{{ordinary}}".into(),
        enabled: true,
    }];
    input.pre_request_script = Some(
        r#"
        const local = ["local", "canary"].join("-");
        const env = ["environment", "canary"].join("-");
        const header = ["header", "canary"].join("-");
        pm.variables.set("slot", local);
        pm.environment.set("ordinary", env);
        pm.request.headers.upsert({key: "X-Opaque", value: header});
        console.log(local, env, header, "existing-body-canary", "existing-auth-canary");
        pm.test(local, () => pm.expect(env).to.equal(header));
    "#
        .into(),
    );
    input.post_response_script = Some(r#"
        console.log(pm.variables.get("slot"), pm.environment.get("ordinary"), pm.request.headers.get("X-Opaque"));
        pm.test(pm.environment.get("ordinary"), () => pm.expect(pm.variables.get("slot")).to.equal("other"));
        pm.environment.set("after", "post-committed-canary");
        console.log(pm.environment.get("after"));
    "#.into());
    let result = bus.send_api_request_with_scripts(input).await.unwrap();
    assert_eq!(
        result.response.as_ref().map(|response| response.status),
        Some(200)
    );
    let wire = tokio::task::spawn_blocking(move || server.join().unwrap())
        .await
        .unwrap();
    assert!(wire.starts_with("POST /echo?opaque=environment-canary "));
    assert!(wire
        .to_ascii_lowercase()
        .contains("x-opaque: header-canary"));
    assert!(wire.contains("Bearer existing-auth-canary"));
    assert!(wire.ends_with(r#"{"token":"existing-body-canary","ordinary":"local-canary"}"#));
    assert_eq!(result.pre_request.status, ScriptExecutionStatus::Success);
    assert_eq!(result.post_response.status, ScriptExecutionStatus::Success);
    assert!(!result.pre_request.tests[0].passed);
    assert!(!result.post_response.tests[0].passed);
    let environments = bus
        .workspace_environments_list(workspace.clone())
        .await
        .unwrap();
    let persisted = environments
        .iter()
        .find(|item| item.id == environment.id)
        .unwrap();
    assert!(persisted
        .variables
        .iter()
        .any(|item| item.key == "ordinary" && item.value == "environment-canary"));
    assert!(persisted
        .variables
        .iter()
        .any(|item| item.key == "after" && item.value == "post-committed-canary"));
    let history = bus
        .api_history_detail(
            workspace.clone(),
            result.response.as_ref().unwrap().history_id.clone(),
        )
        .await
        .unwrap();
    let activity = bus
        .activity_log
        .list_recent(Some(&workspace), 30)
        .await
        .unwrap();
    let stored: String = sqlx::query_scalar("SELECT request_body || request_headers_json || request_query_json FROM api_history LIMIT 1").fetch_one(bus.db.pool()).await.unwrap();
    let activity = activity
        .iter()
        .map(|entry| &entry.details_json)
        .collect::<Vec<_>>();
    let visible = serde_json::to_string(&(result, history, activity, stored)).unwrap();
    assert!(
        !visible.contains("canary"),
        "script diagnostics/history/activity must be safe"
    );
}

#[tokio::test]
async fn failed_pre_scripts_redact_diagnostics_and_do_not_commit_or_record_history() {
    for tail in [
        "throw new Error(value)",
        "pm.request.method = ''",
        "Object.prototype.toJSON = function() { throw new Error(value); }",
    ] {
        let bus = test_bus().await;
        let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
        let environment = bus
            .workspace_environment_create(workspace.clone(), "Rollback".into())
            .await
            .unwrap();
        bus.workspace_environment_set_active(workspace.clone(), Some(environment.id.clone()))
            .await
            .unwrap();
        let mut input = api_script_test_input(workspace.clone(), "http://127.0.0.1:1/never".into());
        input.pre_request_script = Some(format!(
            r#"
            const value = ["failed", "canary"].join("-");
            pm.environment.set("ordinary", value);
            pm.variables.set("ordinary", value);
            pm.request.headers.upsert({{key: "X-Opaque", value}});
            console.log(value);
            pm.test(value, () => pm.expect(value).to.equal("other"));
            {tail}
        "#
        ));
        let result = bus.send_api_request_with_scripts(input).await.unwrap();
        assert_eq!(result.pre_request.status, ScriptExecutionStatus::Failed);
        assert_eq!(result.post_response.status, ScriptExecutionStatus::Skipped);
        assert!(result.response.is_none());
        assert!(result.http_error.is_none());
        let environments = bus
            .workspace_environments_list(workspace.clone())
            .await
            .unwrap();
        assert!(environments
            .iter()
            .find(|item| item.id == environment.id)
            .unwrap()
            .variables
            .is_empty());
        assert!(bus
            .list_api_history(workspace.clone(), Some(10))
            .await
            .unwrap()
            .is_empty());
        let activity = bus
            .activity_log
            .list_recent(Some(&workspace), 30)
            .await
            .unwrap();
        let activity = activity
            .iter()
            .map(|entry| &entry.details_json)
            .collect::<Vec<_>>();
        assert!(!serde_json::to_string(&(result, activity))
            .unwrap()
            .contains("failed-canary"));
    }
}
