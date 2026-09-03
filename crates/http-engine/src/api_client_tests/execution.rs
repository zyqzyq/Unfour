use super::super::*;
use super::support::service;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn zero_timeout_is_unlimited_and_duration_includes_the_complete_body() {
    let service = service().await;
    let (url, server) = delayed_body_server(Duration::from_millis(90), "complete body");

    let response = service
        .send(request(&url, Some(0)))
        .await
        .expect("zero timeout request succeeds");

    assert_eq!(response.body, "complete body");
    assert!(
        response.duration_ms >= 70,
        "duration was {}ms",
        response.duration_ms
    );
    server.join().expect("server thread completes");
}

#[tokio::test]
async fn positive_timeout_is_classified_as_api_timeout() {
    let service = service().await;
    let (url, server) = delayed_body_server(Duration::from_millis(160), "too late");

    let error = service
        .send(request(&url, Some(35)))
        .await
        .expect_err("request should time out");

    assert!(matches!(error, AppError::ApiTimeout(_)));
    assert_eq!(error.code(), "API_TIMEOUT");
    server.join().expect("server thread completes");
}

#[tokio::test]
async fn cancellation_stops_body_receive_and_does_not_write_history() {
    let service = service().await;
    let (url, server) = delayed_body_server(Duration::from_millis(180), "cancelled body");
    let cancellation = CancellationToken::new();
    let send_token = cancellation.clone();
    let send_service = service.clone();
    let task = tokio::spawn(async move {
        send_service
            .send_cancellable(request(&url, Some(0)), send_token)
            .await
    });

    tokio::time::sleep(Duration::from_millis(25)).await;
    cancellation.cancel();
    let error = task
        .await
        .expect("send task joins")
        .expect_err("request should be cancelled");

    assert!(matches!(error, AppError::ApiCancelled(_)));
    assert_eq!(error.code(), "API_CANCELLED");
    let history_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_history")
        .fetch_one(service.db.pool())
        .await
        .expect("count history");
    assert_eq!(history_count, 0);
    server.join().expect("server thread completes");
}

fn delayed_body_server(delay: Duration, body: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local HTTP server");
    let address = listener.local_addr().expect("read local address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(headers.as_bytes())
            .expect("write response headers");
        stream.flush().expect("flush response headers");
        thread::sleep(delay);
        let _ = stream.write_all(body.as_bytes());
    });
    (format!("http://{address}/slow"), handle)
}

fn request(url: &str, timeout_ms: Option<u64>) -> ApiRequestInput {
    ApiRequestInput {
        workspace_id: "workspace-a".to_string(),
        name: Some("execution control".to_string()),
        parent_folder_id: None,
        collection_id: None,
        auth_json: None,
        method: "GET".to_string(),
        url: url.to_string(),
        headers: vec![],
        query: vec![],
        body: None,
        body_kind: "none".to_string(),
        timeout_ms,
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        temporary_variables: vec![],
    }
}
