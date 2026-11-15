use super::execution::request;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

async fn service() -> super::super::ApiClientService {
    let mut service = super::support::service().await;
    service.client = reqwest::Client::builder().no_proxy().build().unwrap();
    service
}

fn post_server(
    status: u16,
    truncate: bool,
) -> (String, Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/write", listener.local_addr().unwrap());
    let writes = Arc::new(AtomicUsize::new(0));
    let observed = writes.clone();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut wire = Vec::new();
        let mut chunk = [0; 1024];
        while !wire.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let count = socket.read(&mut chunk).unwrap();
            assert!(count > 0);
            wire.extend_from_slice(&chunk[..count]);
        }
        assert!(
            wire.starts_with(b"POST /write"),
            "unexpected request: {}",
            String::from_utf8_lossy(&wire)
        );
        // The fixture's remote side effect commits before it sends a response.
        observed.fetch_add(1, Ordering::SeqCst);
        let length = if truncate { 100 } else { 9 };
        write!(socket, "HTTP/1.1 {status} Result\r\nContent-Length: {length}\r\nConnection: close\r\n\r\ncommitted").unwrap();
    });
    (url, writes, server)
}

#[tokio::test]
async fn received_response_survives_history_failure_without_repeating_write() {
    for status in [201, 500] {
        let service = service().await;
        sqlx::query("CREATE TRIGGER reject_history BEFORE INSERT ON api_history BEGIN SELECT RAISE(FAIL, 'fixture history unavailable'); END")
            .execute(service.db.pool()).await.unwrap();
        let (url, writes, server) = post_server(status, false);
        let mut input = request(&url, Some(2000));
        input.method = "POST".into();
        let response = service
            .send(input)
            .await
            .expect("HTTP result survives local history failure");
        server.join().unwrap();
        assert_eq!(response.status, status);
        assert_eq!(response.body, "committed");
        assert!(response.history_id.is_empty());
        assert_eq!(response.warnings, ["API_HISTORY_RECORD_FAILED"]);
        assert_eq!(writes.load(Ordering::SeqCst), 1);
        assert!(service
            .list_history("workspace-a".into(), None)
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn interrupted_response_after_remote_commit_does_not_repeat_post_or_claim_success() {
    let service = service().await;
    let (url, writes, server) = post_server(201, true);
    let mut input = request(&url, Some(2000));
    input.method = "POST".into();
    let error = service.send(input).await.unwrap_err();
    server.join().unwrap();
    assert!(matches!(error, unfour_core::AppError::Http(_)));
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    assert!(service
        .list_history("workspace-a".into(), None)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn cancellation_while_recording_history_preserves_received_response() {
    let service = service().await;
    let connection = service.db.pool().acquire().await.unwrap();
    let (url, writes, server) = post_server(201, false);
    let cancellation = CancellationToken::new();
    let mut input = request(&url, Some(2000));
    input.method = "POST".into();
    let task_service = service.clone();
    let token = cancellation.clone();
    let mut task = tokio::spawn(async move { task_service.send_cancellable(input, token).await });
    while writes.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    server.join().unwrap();
    // The only pool connection is held: the complete response waits on local history.
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut task)
        .await
        .is_err());
    cancellation.cancel();
    assert!(tokio::time::timeout(Duration::from_millis(50), &mut task)
        .await
        .is_err());
    drop(connection);
    let response = task.await.unwrap().unwrap();
    assert_eq!(response.status, 201);
    assert_eq!(response.body, "committed");
    assert!(!response.history_id.is_empty());
    assert_eq!(writes.load(Ordering::SeqCst), 1);
}
