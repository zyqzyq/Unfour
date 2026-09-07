use super::support::service;
use std::io::{Read, Write};
use unfour_core::models::{ApiMultipartRuntimePart, ApiRequestInput};

fn input(url: &str) -> ApiRequestInput {
    serde_json::from_value(serde_json::json!({
        "workspaceId": "workspace-a", "name": "multipart", "method": "POST", "url": url,
        "headers": [{"enabled": true, "key": "cOnTeNt-TyPe", "value": "application/json"}],
        "query": [], "bodyKind": "multipart-form-data", "timeoutMs": 3000,
        "body": serde_json::json!([
            {"id":"t1","enabled":true,"key":"same","type":"text","value":"hello"},
            {"id":"t2","enabled":true,"key":"same","type":"text","value":""},
            {"id":"f","enabled":true,"key":"avatar","type":"file","fileName":"avatar.png"},
            {"id":"off","enabled":false,"key":"disabled","type":"file","fileName":null}
        ]).to_string()
    }))
    .unwrap()
}

fn server() -> (String, std::thread::JoinHandle<(String, Vec<u8>)>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buf = [0u8; 4096];
        let header_end = loop {
            let count = stream.read(&mut buf).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buf[..count]);
            if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                break end + 4;
            }
        };
        let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
        let length: usize = headers
            .lines()
            .find_map(|line| {
                line.to_lowercase()
                    .strip_prefix("content-length:")
                    .map(|value| value.trim().parse().unwrap())
            })
            .unwrap_or(0);
        while bytes.len() < header_end + length {
            let count = stream.read(&mut buf).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buf[..count]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        (headers, bytes[header_end..].to_vec())
    });
    (format!("http://{address}/upload"), handle)
}

#[tokio::test]
async fn real_multipart_stream_boundary_and_safe_history_saved_definition() {
    let service = service().await;
    let (url, server) = server();
    let path = std::env::temp_dir().join(format!("unfour-multipart-{}", unfour_core::id::new_id()));
    let file_bytes = b"file-content-\x00\xff\r\n";
    std::fs::write(&path, file_bytes).unwrap();
    let mut request = input(&url);
    request.multipart_parts.push(ApiMultipartRuntimePart {
        id: "f".into(),
        file_path: path.to_str().unwrap().into(),
    });
    let serialized = serde_json::to_string(&request).unwrap();
    assert!(!serialized.contains("filePath"));
    assert!(!format!("{request:?}").contains(path.to_str().unwrap()));
    let saved = service.save_request(request.clone()).await.unwrap();
    assert_eq!(saved.body, request.body);
    assert!(!serde_json::to_string(&saved).unwrap().contains("filePath"));
    let response = service.send(request).await.unwrap();
    std::fs::remove_file(&path).unwrap();
    let (headers, body) = server.join().unwrap();
    let content_type = headers
        .lines()
        .find(|line| line.to_lowercase().starts_with("content-type:"))
        .unwrap();
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    assert!(content_type.contains("multipart/form-data; boundary="));
    assert!(!headers.contains("application/json"));
    assert!(body.starts_with(format!("--{boundary}\r\n").as_bytes()));
    assert!(body.ends_with(format!("--{boundary}--\r\n").as_bytes()));
    let text = String::from_utf8_lossy(&body);
    assert_eq!(text.matches("name=\"same\"").count(), 2);
    assert!(text.contains("\r\n\r\nhello\r\n"));
    assert!(text.contains("filename=\"avatar.png\""));
    assert!(body
        .windows(file_bytes.len())
        .any(|window| window == file_bytes));
    assert!(!text.contains("disabled"));
    let history = service
        .history_detail("workspace-a".into(), response.history_id)
        .await
        .unwrap();
    assert_eq!(history.request_body_kind, "multipart-form-data");
    assert_eq!(history.request_body, saved.body);
    let history_json = serde_json::to_string(&history).unwrap();
    assert!(!history_json.contains("filePath"));
    assert!(!history_json.contains(path.to_str().unwrap()));
    // Existing OpenAPI extension fallback must survive import/export.
    let export = service
        .export_collection_openapi(
            "workspace-a".into(),
            saved.collection_id,
            unfour_core::models::ApiCollectionExportFormat::Json,
            vec![],
        )
        .await
        .unwrap();
    assert!(export.content.contains("multipart-form-data"));
    assert!(export.content.contains("avatar.png"));
    let imported = service
        .import_collection_openapi("workspace-b".into(), export.content)
        .await
        .unwrap();
    assert!(imported.imported);
    let reopened = service
        .list_saved_requests("workspace-b".into())
        .await
        .unwrap();
    assert_eq!(reopened[0].body_kind, "multipart-form-data");
    let original: serde_json::Value = serde_json::from_str(saved.body.as_deref().unwrap()).unwrap();
    let imported: serde_json::Value =
        serde_json::from_str(reopened[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(original, imported);
}

#[tokio::test]
async fn missing_binding_deleted_file_and_directory_fail_without_path_disclosure() {
    let service = service().await;
    let mut request = input("http://127.0.0.1:1/upload");
    let error = service.send(request.clone()).await.unwrap_err().to_string();
    assert!(error.contains("requires a local file selection"));
    let path = std::env::temp_dir().join(format!("unfour-deleted-{}", unfour_core::id::new_id()));
    std::fs::write(&path, "deleted").unwrap();
    request.multipart_parts.push(ApiMultipartRuntimePart {
        id: "f".into(),
        file_path: path.to_str().unwrap().into(),
    });
    std::fs::remove_file(&path).unwrap();
    for local_path in [path, std::env::temp_dir()] {
        request.multipart_parts[0].file_path = local_path.to_str().unwrap().into();
        let error = service.send(request.clone()).await.unwrap_err().to_string();
        assert!(error.contains("missing or unreadable"));
        assert!(!error.contains(local_path.to_str().unwrap()));
    }
}

#[tokio::test]
async fn get_head_never_require_bindings_or_send_body() {
    let service = service().await;
    for method in ["GET", "HEAD"] {
        let (url, server) = server();
        let mut request = input(&url);
        request.method = method.into();
        service.send(request).await.unwrap();
        let (headers, body) = server.join().unwrap();
        assert!(body.is_empty());
        assert!(!headers.to_lowercase().contains("content-type:"));
    }
}

#[tokio::test]
async fn unsafe_definition_cannot_be_saved_and_legacy_history_defaults_to_json() {
    let service = service().await;
    let mut request = input("https://example.test");
    request.body = Some(r#"[{"id":"f","enabled":true,"key":"f","type":"file","fileName":"file","filePath":"/private/secret"}]"#.into());
    assert!(service.save_request(request).await.is_err());
    sqlx::query("INSERT INTO api_history (id,workspace_id,method,url,request_headers_json,request_query_json,response_headers_json,created_at,updated_at) VALUES ('legacy','workspace-a','POST','http://example.test','[]','[]','[]','now','now')").execute(service.db.pool()).await.unwrap();
    let history = service
        .history_detail("workspace-a".into(), "legacy".into())
        .await
        .unwrap();
    assert_eq!(history.request_body_kind, "json");
}

#[cfg(windows)]
#[tokio::test]
async fn exclusive_file_is_unreadable_without_leaking_path() {
    use std::os::windows::fs::OpenOptionsExt;
    let service = service().await;
    let path = std::env::temp_dir().join(format!("unfour-locked-{}", unfour_core::id::new_id()));
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let mut request = input("http://127.0.0.1:1");
    request.multipart_parts.push(ApiMultipartRuntimePart {
        id: "f".into(),
        file_path: path.to_str().unwrap().into(),
    });
    let error = service.send(request).await.unwrap_err().to_string();
    drop(file);
    std::fs::remove_file(&path).unwrap();
    assert!(error.contains("missing or unreadable"));
    assert!(!error.contains(path.to_str().unwrap()));
}
