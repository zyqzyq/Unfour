use super::*;
#[path = "tests/browse.rs"]
mod collection_pages;
use crate::protocol::Reply;
#[path = "tests/mutation.rs"]
mod mutation_tests;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    task::JoinHandle,
};

struct Exchange {
    arguments: Vec<Vec<u8>>,
    reply: Vec<u8>,
}

fn exchange(arguments: &[&[u8]], reply: &[u8]) -> Exchange {
    Exchange {
        arguments: arguments.iter().map(|a| a.to_vec()).collect(),
        reply: reply.to_vec(),
    }
}

async fn scripted(script: Vec<Exchange>) -> (RedisEndpoint, JoinHandle<()>) {
    scripted_with_linger(script, std::time::Duration::ZERO).await
}

async fn scripted_with_linger(
    script: Vec<Exchange>,
    linger: std::time::Duration,
) -> (RedisEndpoint, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        for step in script {
            let mut header = String::new();
            reader.read_line(&mut header).await.unwrap();
            assert_eq!(header, format!("*{}\r\n", step.arguments.len()));
            for expected in step.arguments {
                let Reply::Bulk(Some(argument)) = protocol::read_reply(&mut reader, MAX_KEY_BYTES)
                    .await
                    .unwrap()
                else {
                    panic!("expected a bulk argument");
                };
                assert_eq!(expected, argument);
            }
            reader.get_mut().write_all(&step.reply).await.unwrap();
        }
        if !linger.is_zero() {
            tokio::time::sleep(linger).await;
        }
    });
    (
        RedisEndpoint {
            host: "127.0.0.1".into(),
            port,
            db_index: 0,
            username: None,
            tls: false,
        },
        server,
    )
}

#[tokio::test]
async fn authenticated_named_db_reads_binary_keys_and_values() {
    let key = b"k\0\xff";
    let (mut endpoint, server) = scripted(vec![
        exchange(&[b"AUTH", b"fixture-user", b"fixture-password"], b"+OK\r\n"),
        exchange(&[b"SELECT", b"2"], b"+OK\r\n"),
        exchange(&[b"TYPE", key], b"+string\r\n"),
        exchange(&[b"STRLEN", key], b":2\r\n"),
        exchange(&[b"GETRANGE", key, b"0", b"16384"], b"$2\r\n\xff\0\r\n"),
        exchange(&[b"PTTL", key], b":0\r\n"),
    ])
    .await;
    endpoint.username = Some("fixture-user".into());
    endpoint.db_index = 2;
    let info = inspect_key(&endpoint, Some("fixture-password"), key, None)
        .await
        .unwrap();
    assert_eq!(info.length, Some(2));
    assert_eq!(info.pttl_ms, 0);
    let preview = info.preview.unwrap();
    assert_eq!(preview.base64, "/wA=");
    assert!(!preview.truncated);
    server.await.unwrap();
}

#[tokio::test]
async fn detects_expiry_after_read_and_forgets_preview() {
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"k"], b"+string\r\n"),
        exchange(&[b"STRLEN", b"k"], b":3\r\n"),
        exchange(&[b"GETRANGE", b"k", b"0", b"16384"], b"$3\r\nabc\r\n"),
        exchange(&[b"PTTL", b"k"], b":-2\r\n"),
    ])
    .await;
    let info = inspect_key(&endpoint, None, b"k", None).await.unwrap();
    assert_eq!(info.pttl_ms, -2);
    assert!(!info.exists);
    assert!(info.preview.is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn growing_string_does_not_treat_a_truncated_preview_as_complete() {
    let value = vec![b'x'; PREVIEW_BYTES + 1];
    let mut reply = format!("${}\r\n", value.len()).into_bytes();
    reply.extend(value);
    reply.extend(b"\r\n");
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"k"], b"+string\r\n"),
        exchange(&[b"STRLEN", b"k"], b":1\r\n"),
        exchange(&[b"GETRANGE", b"k", b"0", b"16384"], &reply),
        exchange(&[b"PTTL", b"k"], b":-1\r\n"),
    ])
    .await;
    let preview = inspect_key(&endpoint, None, b"k", None)
        .await
        .unwrap()
        .preview
        .unwrap();
    assert!(preview.truncated);
    assert_eq!(
        STANDARD.decode(preview.base64).unwrap().len(),
        PREVIEW_BYTES
    );
    server.await.unwrap();
}

#[tokio::test]
async fn tls_endpoint_never_falls_back_to_plaintext() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let (mut socket, _) = listener.accept().await.unwrap();
        // TLS handshake record precedes every Redis command, including authentication.
        assert_eq!(socket.read_u8().await.unwrap(), 0x16);
        socket.write_all(b"+PONG\r\n").await.unwrap();
    });
    let endpoint = RedisEndpoint {
        host: "127.0.0.1".into(),
        port,
        db_index: 0,
        username: None,
        tls: true,
    };
    let failure = test_connection(&endpoint, Some("fixture-password"))
        .await
        .unwrap_err();
    assert_eq!(failure.code(), "REDIS_TLS_FAILED");
    server.await.unwrap();
}

#[tokio::test]
async fn large_hash_field_race_rejects_header_before_reading_payload() {
    let (endpoint, server) = scripted(vec![
        exchange(&[b"TYPE", b"h"], b"+hash\r\n"),
        exchange(&[b"HLEN", b"h"], b":1\r\n"),
        exchange(&[b"HSTRLEN", b"h", b"f"], b":20\r\n"),
        // Data changes after the length check. No payload follows the header.
        exchange(&[b"HGET", b"h", b"f"], b"$1073741824\r\n"),
    ])
    .await;
    let failure = inspect_key(&endpoint, None, b"h", Some(b"f"))
        .await
        .unwrap_err();
    assert_eq!(failure.code(), "REDIS_RESPONSE_TOO_LARGE");
    server.await.unwrap();
}

#[tokio::test]
async fn scan_preserves_empty_pages_and_binary_keys() {
    let (endpoint, server) = scripted(vec![exchange(
        &[b"SCAN", b"0", b"MATCH", b"order:*", b"COUNT", b"100"],
        b"*2\r\n$2\r\n42\r\n*0\r\n",
    )])
    .await;
    let page = scan_keys(&endpoint, None, "0", "order:*").await.unwrap();
    assert_eq!(page.cursor, "42");
    assert!(page.keys.is_empty());
    server.await.unwrap();
    let mut final_reply = &b"*2\r\n$1\r\n0\r\n*2\r\n$1\r\n\xff\r\n$0\r\n\r\n"[..];
    let final_page = protocol::read_scan(&mut final_reply).await.unwrap();
    assert_eq!(final_page.keys, ["/w==", ""]);
    assert_eq!(final_page.cursor, "0");
}

#[tokio::test]
async fn inspecting_a_hash_field_stops_if_the_key_was_replaced() {
    // The fixture accepts only TYPE. A follow-up String read would see a closed
    // socket and return a different error rather than passing this assertion.
    let (endpoint, server) = scripted(vec![exchange(&[b"TYPE", b"product"], b"+string\r\n")]).await;
    let result = inspect_key(&endpoint, None, b"product", Some(b"price")).await;
    assert_eq!(result.unwrap_err().code(), "REDIS_KEY_CHANGED");
    server.await.unwrap();
}

#[tokio::test]
async fn connection_test_accepts_ping_only_acl_denial_and_redacts_other_errors() {
    let (endpoint, server) = scripted(vec![exchange(&[b"PING"], b"-NOPERM disabled\r\n")]).await;
    test_connection(&endpoint, None).await.unwrap();
    server.await.unwrap();
    let (endpoint, server) = scripted(vec![exchange(
        &[b"PING"],
        b"-NOAUTH user@example-secret\r\n",
    )])
    .await;
    let failure = test_connection(&endpoint, None).await.unwrap_err();
    assert_eq!(failure.code(), "REDIS_AUTH_FAILED");
    assert!(!failure.to_string().contains("example-secret"));
    server.await.unwrap();
}

#[tokio::test]
async fn oversized_scan_and_nested_frames_fail_closed() {
    let mut too_many = &b"*2\r\n$1\r\n0\r\n*2147483647\r\n"[..];
    assert_eq!(
        protocol::read_scan(&mut too_many).await.unwrap_err().code(),
        "REDIS_RESPONSE_TOO_LARGE"
    );
    let mut nested = &b"*2\r\n$1\r\n0\r\n*1\r\n*1\r\n"[..];
    assert_eq!(
        protocol::read_scan(&mut nested).await.unwrap_err().code(),
        "REDIS_PROTOCOL_ERROR"
    );
}

#[tokio::test(start_paused = true)]
async fn connection_timeout_includes_partial_replies() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.write_all(b"+").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    });
    let endpoint = RedisEndpoint {
        host: "127.0.0.1".into(),
        port,
        db_index: 0,
        username: None,
        tls: false,
    };
    assert_eq!(
        test_connection(&endpoint, None).await.unwrap_err().code(),
        "REDIS_TIMEOUT"
    );
    task.abort();
}
