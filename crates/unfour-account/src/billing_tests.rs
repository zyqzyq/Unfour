use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration as StdDuration;

use time::{Duration, OffsetDateTime};

use super::*;

const SESSION_TOKEN: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde";

fn test_service_at(
    api_base_url: &str,
    secrets: unfour_secret_store::SecretStore,
) -> AccountService {
    AccountService::with_secret_store(api_base_url, "https://web.example.test", false, secrets)
        .expect("valid test service")
}

fn spawn_http_server(
    responses: Vec<(&'static str, &'static str)>,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test HTTP server");
    let address = listener.local_addr().expect("test server address");
    let server = thread::spawn(move || {
        responses
            .into_iter()
            .map(|(status, body)| {
                let (mut stream, _) = listener.accept().expect("accept HTTP request");
                let request = read_http_request(&mut stream);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write HTTP response");
                request
            })
            .collect()
    });
    (format!("http://{address}"), server)
}

fn read_http_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(StdDuration::from_secs(5)))
        .expect("set request read timeout");
    let mut request = Vec::new();
    loop {
        let mut chunk = [0_u8; 4096];
        let count = stream.read(&mut chunk).expect("read HTTP request");
        assert!(count > 0, "connection closed before complete HTTP request");
        request.extend_from_slice(&chunk[..count]);

        let Some(header_offset) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
            continue;
        };
        let body_offset = header_offset + 4;
        let headers = String::from_utf8_lossy(&request[..header_offset]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("valid content length"))
            })
            .unwrap_or(0);
        if request.len() >= body_offset + content_length {
            return String::from_utf8(request).expect("ASCII HTTP request");
        }
    }
}

async fn save_active_session(service: &AccountService) {
    service
        .sessions
        .save(StoredSession {
            session_token: SESSION_TOKEN.into(),
            expires_at: OffsetDateTime::now_utc() + Duration::days(30),
        })
        .await
        .expect("save desktop session");
}

#[tokio::test]
async fn billing_requests_use_the_saved_desktop_session_and_exact_endpoints() {
    let (api_base_url, server) = spawn_http_server(vec![
        (
            "200 OK",
            r#"{"checkout_url":"https://checkout.example.test/session/checkout-1"}"#,
        ),
        (
            "200 OK",
            r#"{"portal_url":"https://billing.example.test/portal/portal-1"}"#,
        ),
    ]);
    let service = test_service_at(
        &api_base_url,
        unfour_secret_store::SecretStore::in_memory("billing-session-test"),
    );
    save_active_session(&service).await;

    let checkout = service
        .create_billing_checkout()
        .await
        .expect("create checkout");
    let portal = service
        .create_billing_portal()
        .await
        .expect("create portal");
    assert_eq!(
        checkout.as_str(),
        "https://checkout.example.test/session/checkout-1"
    );
    assert_eq!(
        portal.as_str(),
        "https://billing.example.test/portal/portal-1"
    );

    let requests = server.join().expect("HTTP test server");
    assert_eq!(requests.len(), 2);
    let checkout_request = requests[0].to_ascii_lowercase();
    assert!(checkout_request.starts_with("post /v1/billing/checkout http/1.1\r\n"));
    assert!(checkout_request.contains(&format!(
        "\r\nx-desktop-session: {}\r\n",
        SESSION_TOKEN.to_ascii_lowercase()
    )));
    assert!(checkout_request.contains(r#"{"plan":"pro_monthly"}"#));
    assert!(!checkout_request.contains("authorization:"));

    let portal_request = requests[1].to_ascii_lowercase();
    assert!(portal_request.starts_with("post /v1/billing/portal http/1.1\r\n"));
    assert!(portal_request.contains(&format!(
        "\r\nx-desktop-session: {}\r\n",
        SESSION_TOKEN.to_ascii_lowercase()
    )));
    assert!(!portal_request.contains("authorization:"));
}

#[tokio::test]
async fn checkout_without_a_saved_session_makes_no_http_request() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind observation socket");
    listener
        .set_nonblocking(true)
        .expect("set observation socket nonblocking");
    let service = test_service_at(
        &format!("http://{}", listener.local_addr().expect("socket address")),
        unfour_secret_store::SecretStore::in_memory("billing-no-session-test"),
    );

    let error = service
        .create_billing_checkout()
        .await
        .expect_err("checkout requires a desktop session");
    assert!(matches!(error, AccountError::SignedOut));
    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
}

#[tokio::test]
async fn expired_api_session_is_deleted_and_reported_as_signed_out() {
    let (api_base_url, server) = spawn_http_server(vec![(
        "401 Unauthorized",
        r#"{"error":{"code":"desktop_session_expired","message":"expired","requestId":"request-1"}}"#,
    )]);
    let service = test_service_at(
        &api_base_url,
        unfour_secret_store::SecretStore::in_memory("billing-expired-session-test"),
    );
    save_active_session(&service).await;

    let error = service
        .create_billing_portal()
        .await
        .expect_err("expired session must fail");
    assert!(matches!(error, AccountError::SignedOut));
    assert!(service
        .sessions
        .load()
        .await
        .expect("load cleared session")
        .is_none());
    assert_eq!(server.join().expect("HTTP test server").len(), 1);
}
