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

#[test]
fn history_redaction_keeps_form_context_and_scrubs_short_credential_echoes() {
    let mut form = request("https://example.test", None);
    form.body_kind = "form-urlencoded".into();
    form.body = Some("password=body-secret&note=keep".into());
    let form = super::super::history_redaction::sanitize(&form, &[], "note=keep").unwrap();
    let form_body = form.body.unwrap();
    assert!(form_body.contains("note=keep"), "{form_body}");
    assert!(!form_body.contains("body-secret"), "{form_body}");

    let mut json = request("https://example.test", None);
    json.body_kind = "json".into();
    json.body = Some(r#"{"password":"ab","note":"ab"}"#.into());
    let json = super::super::history_redaction::sanitize(&json, &[], r#"{"ok":true}"#).unwrap();
    let json_body = json.body.unwrap();
    assert!(!json_body.contains("\"note\":\"ab\""), "{json_body}");
    assert!(!json_body.contains("\"password\":\"ab\""), "{json_body}");
    assert!(
        json.response_body.contains("true"),
        "{}",
        json.response_body
    );
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
        multipart_parts: vec![],
        temporary_variables: vec![],
    }
}

#[tokio::test]
async fn history_redacts_credentials_without_changing_runtime_payloads() {
    let service = service().await;
    let (url, server) =
        delayed_body_server(Duration::ZERO, r#"{"token":"response-secret","ok":true}"#);
    let mut input = request(&format!("{url}?access_token=url-secret"), Some(5000));
    input.method = "POST".into();
    input.headers = vec![KeyValue {
        key: "Authorization".into(),
        value: "Bearer header-secret".into(),
        enabled: true,
    }];
    input.body_kind = "json".into();
    input.body = Some(r#"{"password":"body-secret","ordinary":"keep"}"#.into());
    let response = service.send(input).await.unwrap();
    server.join().unwrap();
    assert!(response.body.contains("response-secret"));
    let stored: (String, String, String, String, i64) = sqlx::query_as("SELECT url,request_headers_json,request_body,response_body_preview,redaction_version FROM api_history WHERE id=?")
        .bind(&response.history_id).fetch_one(service.db.pool()).await.unwrap();
    for value in [&stored.0, &stored.1, &stored.2, &stored.3] {
        assert!(!value.contains("-secret"), "{value}");
    }
    assert!(stored.2.contains("keep"));
    assert!(stored.3.contains("true"));
    assert_eq!(stored.4, 1);
}

#[tokio::test]
async fn legacy_history_is_redacted_on_read_and_repaired_idempotently() {
    let service = service().await;
    sqlx::query("INSERT INTO api_history (id,workspace_id,method,url,request_headers_json,request_body,response_body_preview,created_at,updated_at) VALUES ('legacy','workspace-a','POST','https://example.test?token=url-secret',?,? ,?,'now','now')")
        .bind(r#"[{"key":"Cookie","value":"session=header-secret","enabled":true}]"#)
        .bind(r#"{"password":"body-secret","ordinary":"keep"}"#)
        .bind(r#"{"token":"truncated-secret"#)
        .execute(service.db.pool()).await.unwrap();
    let detail = service
        .history_detail("workspace-a".into(), "legacy".into())
        .await
        .unwrap();
    assert!(!serde_json::to_string(&detail).unwrap().contains("-secret"));
    let version: i64 =
        sqlx::query_scalar("SELECT redaction_version FROM api_history WHERE id='legacy'")
            .fetch_one(service.db.pool())
            .await
            .unwrap();
    assert_eq!(version, 0, "reading legacy history must remain read-only");
    service.redact_legacy_history().await.unwrap();
    service.redact_legacy_history().await.unwrap();
    let row: (String, String, i64) = sqlx::query_as("SELECT request_body,response_body_preview,redaction_version FROM api_history WHERE id='legacy'").fetch_one(service.db.pool()).await.unwrap();
    assert!(!row.0.contains("body-secret"));
    assert!(row.0.contains("keep"));
    assert_eq!(row.1, "<redacted>");
    assert_eq!(row.2, 1);
}

#[tokio::test]
async fn oversized_declared_and_chunked_bodies_are_rejected_without_history() {
    for chunked in [false, true] {
        let service = service().await;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.read(&mut [0; 4096]);
            if chunked {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                );
                let chunk = vec![b'x'; 64 * 1024];
                for _ in 0..161 {
                    if stream
                        .write_all(b"10000\r\n")
                        .and_then(|_| stream.write_all(&chunk))
                        .and_then(|_| stream.write_all(b"\r\n"))
                        .is_err()
                    {
                        break;
                    }
                }
                let _ = stream.write_all(b"0\r\n\r\n");
            } else {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 10485761\r\nConnection: close\r\n\r\n",
                );
            }
        });
        let error = service
            .send(request(&format!("http://{address}"), Some(5000)))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "API_RESPONSE_TOO_LARGE");
        server.join().unwrap();
        assert!(service
            .list_history("workspace-a".into(), None)
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn read_only_history_repair_leaves_rows_for_a_writable_open() {
    let path = std::env::temp_dir().join(format!(
        "unfour-history-redaction-{}.db",
        unfour_core::id::new_id()
    ));
    let db = LocalDb::connect_path(&path).await.unwrap();
    db.migrate().await.unwrap();
    super::support::seed_workspace(&db, "workspace-a").await;
    sqlx::query("INSERT INTO api_history (id,workspace_id,method,url,request_headers_json,request_body,response_body_preview,created_at,updated_at) VALUES ('legacy','workspace-a','POST','https://example.test?token=url-secret',?,? ,?,'now','now')")
        .bind(r#"[{"key":"Cookie","value":"session=header-secret","enabled":true}]"#)
        .bind(r#"{"password":"body-secret"}"#)
        .bind(r#"{"token":"response-secret"}"#)
        .execute(db.pool())
        .await
        .unwrap();
    drop(db);

    let readonly = LocalDb::connect_existing_read_only_path(&path)
        .await
        .unwrap();
    ApiClientService::new(readonly)
        .redact_legacy_history()
        .await
        .unwrap();

    let db = LocalDb::connect_existing_path(&path).await.unwrap();
    let version: i64 =
        sqlx::query_scalar("SELECT redaction_version FROM api_history WHERE id='legacy'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(version, 0);
    ApiClientService::new(db.clone())
        .redact_legacy_history()
        .await
        .unwrap();
    let version: i64 =
        sqlx::query_scalar("SELECT redaction_version FROM api_history WHERE id='legacy'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(version, 1);
    drop(db);
    let _ = std::fs::remove_file(path);
}
