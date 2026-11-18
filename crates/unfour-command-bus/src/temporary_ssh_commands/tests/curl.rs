use super::*;
use unfour_core::models::ApiRequestInput;

fn request(workspace: &str, path: &str) -> ApiRequestInput {
    serde_json::from_value(serde_json::json!({
        "workspaceId": workspace, "method":"POST", "url":format!("http://terminal.fixture.invalid/{path}"),
        "headers":[{"key":"Authorization","value":"Bearer synthetic-api-secret","enabled":true}],
        "query":[], "bodyKind":"json", "body":"{\"input\":\"中文\"}", "timeoutMs":5000,
        "postResponseScript":"pm.test('large response', () => pm.expect(pm.response.json().values.length).to.eql(17000));"
    })).unwrap()
}

#[tokio::test]
async fn terminal_curl_api_keeps_scripts_response_and_redaction_without_tcp_channels() {
    let bus = CommandBus::ephemeral().await.unwrap();
    let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut server = Server {
        raw_terminal: Some(Default::default()),
        ..Default::default()
    };
    let calls = server.calls.clone();
    let config = Arc::new(russh::server::Config {
        keys: vec![russh::keys::PrivateKey::random(
            &mut rand::rng(),
            russh::keys::Algorithm::Ed25519,
        )
        .unwrap()],
        ..Default::default()
    });
    let task = tokio::spawn(async move {
        server.run_on_socket(config, &listener).await.unwrap();
    });
    let handle = bus
        .receive_temporary_ssh(
            &LaunchReceipt::default(),
            TemporaryRequest::password(
                "127.0.0.1".into(),
                port,
                "synthetic-ticket-user".into(),
                "synthetic-password".into(),
            )
            .unwrap(),
        )
        .unwrap();
    bus.claim_temporary_ssh(&ws, &handle).await.unwrap();
    bus.start_temporary_ssh(&ws, &handle).unwrap();
    wait_for(&bus, &ws, "connected").await;

    for (kind, expected) in [
        (
            TemporaryProbeKind::Transport,
            vec!["transport-roundtrip", "transport-recovery"],
        ),
        (
            TemporaryProbeKind::Curl,
            vec![
                "transport-roundtrip",
                "transport-recovery",
                "curl-tools",
                "curl-recovery",
            ],
        ),
    ] {
        bus.start_temporary_ssh_probe(&ws, &handle, kind, true)
            .unwrap();
        tokio::time::timeout(Duration::from_secs(8), async {
            while bus
                .temporary_ssh_probe(&ws, &handle)
                .unwrap()
                .unwrap()
                .status
                == "running"
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let report = bus.temporary_ssh_probe(&ws, &handle).unwrap().unwrap();
        assert_eq!(
            report
                .checks
                .iter()
                .map(|check| check.id.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(report
            .checks
            .iter()
            .all(|check| check.code == "OK" && check.status == "pass"));
        assert_eq!(report.checks[0].expected_exit_code, Some(23));
        assert_eq!(report.checks[0].actual_exit_code, Some(23));
        assert!(!bus.list_temporary_ssh(&ws).unwrap()[0].terminal_task_busy);
    }

    for (path, status) in [("json", 200), ("error", 500), ("redirect", 302)] {
        let result = bus
            .send_api_request_via_with_mode(
                "terminal-http",
                request(&ws, path),
                None,
                None,
                None,
                Some(&handle),
                "terminalCurl",
                true,
            )
            .await
            .unwrap();
        assert!(result.http_error.is_none(), "{:?}", result.http_error);
        assert_eq!(
            result.post_response.status,
            unfour_core::models::ScriptExecutionStatus::Success
        );
        let response = result.response.unwrap();
        assert_eq!(response.status, status);
        assert!(response.body.len() > 64 * 1024);
        assert_eq!(
            response
                .headers
                .iter()
                .filter(|h| h.key == "x-repeat")
                .count(),
            2
        );
        assert!(!bus.list_temporary_ssh(&ws).unwrap()[0].terminal_task_busy);
        let history: String =
            sqlx::query_scalar("SELECT request_headers_json FROM api_history WHERE id=?")
                .bind(&response.history_id)
                .fetch_one(bus.db.pool())
                .await
                .unwrap();
        assert!(!history.contains("synthetic-api-secret"));
        if status == 302 {
            assert!(response
                .warnings
                .iter()
                .any(|w| w == "SSH_TERMINAL_HTTP_REDIRECT_NOT_FOLLOWED"));
        }
    }
    let failed = bus
        .send_api_request_via_with_mode(
            "terminal-failed",
            request(&ws, "connect-failed"),
            None,
            None,
            None,
            Some(&handle),
            "terminalCurl",
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        failed.http_error_code.as_deref(),
        Some("SSH_TERMINAL_HTTP_CONNECT_FAILED")
    );
    assert!(failed.response.is_none());

    let running_bus = bus.clone();
    let running_ws = ws.clone();
    let running_handle = handle.clone();
    let run = tokio::spawn(async move {
        running_bus
            .send_api_request_via_with_mode(
                "terminal-wait",
                request(&running_ws, "wait"),
                None,
                None,
                None,
                Some(&running_handle),
                "terminalCurl",
                true,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !bus.list_temporary_ssh(&ws).unwrap()[0].terminal_task_busy {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let env = bus
        .workspace_environment_create(ws.clone(), "busy-guard".into())
        .await
        .unwrap();
    let mut second = request(&ws, "json");
    second.pre_request_script = Some("pm.environment.set('unexpected', 'changed')".into());
    let err = bus
        .send_api_request_via_with_mode(
            "terminal-busy",
            second,
            Some(env.id.clone()),
            None,
            None,
            Some(&handle),
            "terminalCurl",
            true,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "SSH_TERMINAL_BUSY");
    assert!(bus
        .workspace_environments_list(ws.clone())
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.id == env.id)
        .unwrap()
        .variables
        .is_empty());
    bus.cancel_api_request("terminal-wait");
    let cancelled = run.await.unwrap().unwrap();
    assert_eq!(
        cancelled.http_error_code.as_deref(),
        Some("SSH_TERMINAL_CANCELLED")
    );
    assert!(!bus.list_temporary_ssh(&ws).unwrap()[0].terminal_task_busy);
    assert!(!bus.list_temporary_ssh(&ws).unwrap()[0].terminal_recovery_required);
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| **c == "login")
            .count(),
        1
    );
    assert!(!calls
        .lock()
        .unwrap()
        .iter()
        .any(|c| matches!(*c, "forward" | "task")));
    for table in [
        "ssh_task_run",
        "ssh_connections",
        "ssh_command_history",
        "ssh_terminal_history",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(bus.db.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
    bus.cancel_temporary_ssh(&ws, &handle).unwrap();
    task.abort();
}
