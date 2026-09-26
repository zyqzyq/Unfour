use super::*;
use serde_json::json;
use std::io::{Read, Write};
use unfour_core::models::FlowRunStatus;

#[tokio::test]
async fn flow_http_failure_scrubs_saved_request_secrets_from_persisted_history() {
    for (kind, body, suffix, userinfo, secrets) in [
        (
            "json",
            r#"{"token":"saved-secret","name":"ordinary-body"}"#,
            "?page=ordinary-query",
            "",
            vec!["saved-secret"],
        ),
        (
            "json",
            r#"{"name":"ordinary-body"}"#,
            "?access_token=url%2Bsecret&page=ordinary-query",
            "alice:user%2Bsecret@",
            vec!["url+secret", "url%2Bsecret", "user+secret", "user%2Bsecret"],
        ),
        (
            "form-urlencoded",
            "password=form%2Bsecret&name=ordinary-body",
            "?page=ordinary-query",
            "",
            vec!["form+secret", "form%2Bsecret"],
        ),
    ] {
        let bus = test_bus().await;
        let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{userinfo}{}{suffix}",
            listener.local_addr().unwrap()
        );
        let echo = secrets.join(" ");
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_owned)
                        })
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let wire = String::from_utf8_lossy(&request);
            assert!(wire.contains("ordinary-query"));
            assert!(wire.contains("ordinary-body"));
            let response = format!(
                "upstream validation failed: {echo}; ordinary-body ordinary-query safe-context"
            );
            write!(socket, "HTTP/1.1 422 Failed\r\nX-Echo: {echo}\r\nX-Trace: safe-context\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
        });
        let mut api = api_script_test_input(workspace.clone(), url);
        api.method = "POST".into();
        api.body_kind = kind.into();
        api.body = Some(body.into());
        let api = bus.save_api_request(api).await.unwrap();
        let flow = bus
            .save_flow(super::flow::definition(
                &workspace,
                json!([super::flow::action("api", "api", &api.id, json!({}))]),
            ))
            .await
            .unwrap();
        let run = bus
            .run_flow(super::flow::request(&workspace, &flow.id))
            .await
            .unwrap();
        let result = super::flow::finished(&bus, &run).await;
        server.join().unwrap();
        assert_eq!(result.status, FlowRunStatus::Failed);
        assert_eq!(result.error.as_deref(), Some("FLOW_HTTP_STATUS_422"));
        let stored: String = sqlx::query_scalar("SELECT run_json FROM flow_runs WHERE id = ?")
            .bind(&run.id)
            .fetch_one(bus.db.pool())
            .await
            .unwrap();
        for secret in secrets {
            assert!(!serde_json::to_string(&run).unwrap().contains(secret));
            assert!(!stored.contains(secret), "{secret}: {stored}");
        }
        let output = result.steps[0].output.as_ref().unwrap();
        assert_eq!(output["status"], 422);
        assert_eq!(output["diagnosticsTruncated"], false);
        for context in [
            "upstream validation failed",
            "ordinary-body",
            "ordinary-query",
            "safe-context",
        ] {
            assert!(output.to_string().contains(context), "{output}");
        }
        assert_eq!(result.steps[0].attempts[0].output.as_ref(), Some(output));
    }
}
