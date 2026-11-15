use super::*;
use crate::{response::content_json, ToolRegistry};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;
use unfour_core::models::ApiRequestInput;

#[test]
fn saved_post_script_failure_keeps_http_success_and_reports_a_safe_warning() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let workspace = adapter
        .run(adapter.bus.create_workspace_with_options(
            "Post script outcome fixture".into(),
            Some("dev".into()),
            Some("auto".into()),
        ))
        .unwrap()
        .id;
    for (script, warning) in [
        (
            "throw new Error('private-post-script-canary')",
            "API_POST_RESPONSE_SCRIPT_FAILED",
        ),
        ("while (true) {}", "API_POST_RESPONSE_SCRIPT_TIMEOUT"),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/effect", listener.local_addr().unwrap());
        let server = adapter.run(async move {
            listener.set_nonblocking(true).unwrap();
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
                let (socket, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(socket);
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).await.unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    assert!(!line.is_empty());
                }
                reader
                    .get_mut()
                    .write_all(
                        b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .await
                    .unwrap();
            })
        });
        let request: ApiRequestInput = serde_json::from_value(json!({
            "workspaceId":workspace,"name":"Post fixture", "method":"POST", "url":url,
            "headers":[],"query":[],"bodyKind":"none",
            "postResponseScript":script
        }))
        .unwrap();
        let request = adapter.save_api_request(request).unwrap();
        let registry = ToolRegistry::with_command_bus(adapter.clone());
        let result = registry
            .call(
                "unfour.api.send_request",
                json!({"workspaceId":workspace,"requestId":request.id}),
            )
            .unwrap();
        let body = content_json(&result);
        assert_eq!(body["ok"], true);
        assert_eq!(body["status"], 201);
        assert!(body["warnings"]
            .as_array()
            .unwrap()
            .contains(&json!(warning)));
        assert!(!result.to_string().contains("private-post-script-canary"));
        adapter.run(server).unwrap();
    }
    adapter.shutdown();
}

#[test]
fn tunnel_failure_after_a_redirect_does_not_claim_the_request_was_unsent() {
    for code in [
        "SSH_TUNNEL_TARGET_UNREACHABLE",
        "SSH_TUNNEL_TEMPORARY_UNAVAILABLE",
    ] {
        let error = CommandBusAdapterError::from_api_app_error(&AppError::Validation(code.into()));
        assert_eq!(error.code, code);
        assert!(error
            .message
            .contains("Remote effects may already have occurred"));
    }
}

#[test]
fn api_mcp_transport_failures_preserve_classification_and_remote_effect_warning() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let workspace = adapter
        .run(adapter.bus.create_workspace_with_options(
            "Execution outcome fixture".into(),
            Some("dev".into()),
            Some("auto".into()),
        ))
        .unwrap()
        .id;
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    for saved in [false, true] {
        for timeout in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/commit", listener.local_addr().unwrap());
            let remote = std::thread::spawn(move || {
                listener.set_nonblocking(true).unwrap();
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let (mut socket, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "HTTP request never arrived"
                            );
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("fixture accept failed: {error}"),
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                let mut parts = request.split_whitespace();
                assert_eq!(parts.next(), Some("POST"));
                assert_eq!(parts.next().unwrap().trim_end_matches('?'), "/commit");
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    assert!(!line.is_empty());
                }
                // The server has applied the fixture's operation. Lose or delay
                // its acknowledgement after that boundary, with no second send.
                if timeout {
                    std::thread::sleep(Duration::from_millis(750));
                } else {
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\npartial").unwrap();
                }
                drop(reader);
                drop(socket);
                listener.set_nonblocking(true).unwrap();
                assert_eq!(
                    listener.accept().unwrap_err().kind(),
                    std::io::ErrorKind::WouldBlock
                );
            });
            let mut args =
                json!({"workspaceId":workspace,"method":"POST","url":url,"timeoutMs":500});
            if saved {
                let request: ApiRequestInput = serde_json::from_value(json!({
                    "workspaceId":workspace,"name":"Outcome fixture", "method":"POST", "url":url,
                    "headers":[],"query":[],"bodyKind":"none"
                }))
                .unwrap();
                let request = adapter.save_api_request(request).unwrap();
                args = json!({"workspaceId":workspace,"requestId":request.id,"timeoutMs":500});
            }
            let result = registry.call("unfour.api.send_request", args).unwrap();
            let error = content_json(&result)["error"].clone();
            assert_eq!(
                error["code"],
                if timeout { "API_TIMEOUT" } else { "HTTP_ERROR" }
            );
            assert!(
                error["message"]
                    .as_str()
                    .unwrap()
                    .contains("Remote effects may already have occurred"),
                "{error}"
            );
            assert!(!result.to_string().contains(&url));
            remote.join().unwrap();
        }
    }
    adapter.shutdown();
}
