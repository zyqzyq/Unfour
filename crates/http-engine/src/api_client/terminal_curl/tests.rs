use super::*;

mod loopback;

fn input() -> ApiRequestInput {
    serde_json::from_value(serde_json::json!({
        "workspaceId": "workspace-a", "method": "POST", "url": "http://example.test/v1/embeddings",
        "headers": [{"key":"Authorization","value":"Bearer synthetic-token","enabled":true}],
        "query": [], "bodyKind":"json", "body":"{\"input\":\"北京是中国的首都。\"}", "timeoutMs":20000
    })).unwrap()
}

fn prepared() -> PreparedTerminalCurl {
    prepare_terminal_curl(input(), &[]).unwrap()
}

#[test]
fn terminal_timeout_is_bounded_and_shared_with_remote_curl() {
    for (requested, expected, option) in [
        (None, 30_000, "--max-time 30 --config -"),
        (Some(0), 30_000, "--max-time 30 --config -"),
        (Some(1), 1, "--max-time 1 --config -"),
        (Some(1_234), 1_234, "--max-time 2 --config -"),
        (Some(60_000), 60_000, "--max-time 60 --config -"),
    ] {
        let mut request = input();
        request.timeout_ms = requested;
        let prepared = prepare_terminal_curl(request, &[]).unwrap();
        assert_eq!(prepared.timeout_ms(), Some(expected));
        assert_eq!(prepared.executed.timeout_ms, Some(expected));
        assert!(prepared.command.contains(option));
        assert!(prepared.command.contains("--retry 0"));
    }
    let mut request = input();
    request.timeout_ms = Some(unfour_core::models::MAX_API_TIMEOUT_MS + 1);
    assert_eq!(
        code(prepare_terminal_curl(request, &[]).err().unwrap()),
        "SSH_TERMINAL_HTTP_UNSUPPORTED_REQUEST"
    );
}

fn wire(prepared: &PreparedTerminalCurl, status: u16, headers: &str, body: &[u8]) -> Vec<u8> {
    wire_with_trailers(prepared, status, headers, body, &[])
}

fn wire_with_trailers(
    prepared: &PreparedTerminalCurl,
    status: u16,
    headers: &str,
    body: &[u8],
    trailers: &[u8],
) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status} Test\r\n{headers}\r\n").into_bytes();
    out.extend_from_slice(body);
    out.extend_from_slice(trailers);
    out.extend_from_slice(
        format!(
            "\nUNFOUR_CURL_RESULT_{}:{status}:{}:0.123456:0.200\n",
            prepared.nonce,
            body.len()
        )
        .as_bytes(),
    );
    out
}

fn code(error: AppError) -> String {
    match error {
        AppError::Validation(code) => code,
        other => panic!("unexpected error kind: {}", other.code()),
    }
}

#[test]
fn quoted_config_keeps_shell_and_curl_options_inside_values() {
    let mut request = input();
    request.body = Some(
        "{\"value\":\"$(touch /nope) `whoami` ' \\\"\",\"line\":\"中文\"}\n--output /nope".into(),
    );
    request.headers.push(KeyValue {
        key: "x-check".into(),
        value: "$(whoami) `id` \\\"".into(),
        enabled: true,
    });
    let prepared = prepare_terminal_curl(request, &[]).unwrap();
    let lines: Vec<_> = prepared.command.lines().collect();
    assert!(lines[0].contains(
        "case $(command -v printf):$(LC_ALL=C command -V printf 2>/dev/null) in printf:*builtin*) ;; *) exit 126;; esac;"
    ));
    assert!(!lines[0].contains("synthetic-token"));
    assert!(lines
        .iter()
        .any(|line| line.starts_with("data-binary = \"") && line.contains("\\n--output /nope")));
    assert!(!lines.iter().any(|line| line.starts_with("--output")));
    for required in [
        "--no-location",
        "--no-netrc",
        "--retry 0",
        "--proxy ''",
        "--noproxy '*'",
        "header = \"accept-encoding: identity\"",
    ] {
        assert!(prepared.command.contains(required));
    }
    assert!(!prepared
        .snapshot
        .headers
        .iter()
        .any(|header| header.value.contains("synthetic-token")));
}

#[test]
fn legacy_json_uses_literal_bytes_and_at_prefixed_text_never_becomes_a_file_read() {
    let prepared = prepared();
    assert!(prepared
        .command
        .contains("\ndata-binary = \"{\\\"input\\\":"));
    assert!(!prepared.command.contains("data-raw ="));
    assert!(prepared
        .command
        .contains("--version </dev/null 2>/dev/null) || exit 125;"));
    for body in ["@/private/file", "@-", "@", "@file\nwith newline"] {
        let mut request = input();
        request.body_kind = "text".into();
        request.body = Some(body.into());
        let prepared = prepare_terminal_curl(request, &[]).unwrap();
        assert!(prepared.command.contains("--data-raw '{}' --version"));
        assert!(prepared.command.contains("|| exit 124; command printf"));
        assert!(prepared.command.contains("\ndata-raw = \"@"));
        assert!(!prepared.command.contains("\ndata-binary ="));
        // History snapshots redact arbitrary text; the execution config above
        // is the wire source, verified byte-for-byte by loopback integration QA.
        assert_eq!(prepared.executed.body.as_deref(), Some("<redacted>"));
    }
}

#[test]
fn chinese_large_request_and_response_preserve_the_entire_payload() {
    let body = serde_json::json!({"embedding":vec![0.12345; 20_000],"text":"中文"}).to_string();
    assert!(body.len() > 64 * 1024);
    let mut input = input();
    input.body = Some(body.clone());
    let prepared = prepare_terminal_curl(input, &[]).unwrap();
    assert!(prepared.command.len() > 4096);
    let response = response::parse(
        &prepared,
        &wire(
            &prepared,
            200,
            "Content-Type: application/json\r\n",
            body.as_bytes(),
        ),
        0,
    )
    .unwrap();
    assert_eq!(response.body, body);
}

#[test]
fn continue_headers_duplicate_headers_and_http_errors_are_preserved() {
    let prepared = prepared();
    let mut bytes =
        b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 103 Early Hints\r\nLink: </foo>\r\n\r\n".to_vec();
    bytes.extend(wire(&prepared, 500, "Content-Type: application/json\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\nTransfer-Encoding: chunked\r\n", b"{\"error\":\"keep\"}"));
    let response = response::parse(&prepared, &bytes, 0).unwrap();
    assert_eq!(response.status, 500);
    assert_eq!(
        response
            .headers
            .iter()
            .filter(|header| header.key == "set-cookie")
            .count(),
        2
    );
    assert_eq!(response.body, "{\"error\":\"keep\"}");
}

#[test]
fn incomplete_or_uncorrelated_response_never_becomes_success() {
    let prepared = prepared();
    for bytes in [
        b"login:".to_vec(),
        wire(&prepared, 200, "", b"hello")[..20].to_vec(),
        wire(&prepared, 200, "", b"hello")
            .into_iter()
            .rev()
            .collect(),
    ] {
        assert_eq!(
            code(response::parse(&prepared, &bytes, 0).err().unwrap()),
            "SSH_TERMINAL_HTTP_INVALID_RESPONSE"
        );
    }
    let mut bytes = wire(&prepared, 200, "", b"hello");
    bytes.extend_from_slice(b"unsolicited tail");
    assert_eq!(
        code(response::parse(&prepared, &bytes, 0).err().unwrap()),
        "SSH_TERMINAL_HTTP_INVALID_RESPONSE"
    );
    let bytes = String::from_utf8(wire(&prepared, 200, "", b"hello"))
        .unwrap()
        .replace(":200:5:", ":200:6:");
    assert_eq!(
        code(
            response::parse(&prepared, bytes.as_bytes(), 0)
                .err()
                .unwrap()
        ),
        "SSH_TERMINAL_HTTP_INVALID_RESPONSE"
    );
}

#[test]
fn only_complete_chunked_trailer_fields_fit_after_the_declared_body() {
    let prepared = prepared();
    let chunked = "Transfer-Encoding: chunked\r\n";
    let with_tail =
        |headers: &str, tail: &[u8]| wire_with_trailers(&prepared, 200, headers, b"{}", tail);
    let valid = with_tail(chunked, b"X-Result: ok\r\nX-Other: 2\r\n");
    let response = response::parse(&prepared, &valid, 0).unwrap();
    assert_eq!(response.body, "{}");
    assert_eq!(response.headers.len(), 1);
    for (headers, tail) in [
        ("", b"X-Result: ok\r\n".as_slice()),
        (chunked, b"X-Result: incomplete"),
        (chunked, b"invalid field: x\r\n"),
        (chunked, b"X-Result: x\x00\r\n"),
        (chunked, b"X-Result: ok\r\n\r\nunsolicited\r\n"),
    ] {
        assert_eq!(
            code(
                response::parse(&prepared, &with_tail(headers, tail), 0)
                    .err()
                    .unwrap()
            ),
            "SSH_TERMINAL_HTTP_INVALID_RESPONSE"
        );
    }
    let large = format!("X-Pad: {}\r\n", "x".repeat(MAX_HEADER_BYTES - 16));
    assert_eq!(
        code(
            response::parse(&prepared, &with_tail(chunked, large.as_bytes()), 0)
                .err()
                .unwrap()
        ),
        "SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE"
    );
}

#[test]
fn supported_scope_is_validated_before_terminal_use() {
    let mut cases = Vec::new();
    let mut value = input();
    value.url = "file:///etc/passwd".into();
    cases.push(value);
    let mut value = input();
    value.url = "http://user:secret@example.test/".into();
    cases.push(value);
    let mut value = input();
    value.method = "CONNECT".into();
    cases.push(value);
    let mut value = input();
    value.body_kind = "multipart".into();
    cases.push(value);
    for (key, value) in [
        ("Accept", "text/event-stream"),
        ("Accept-Encoding", "gzip"),
        ("Content-Length", "0"),
        ("Transfer-Encoding", "chunked"),
        ("Upgrade", "websocket"),
    ] {
        let mut item = input();
        item.headers.push(KeyValue {
            key: key.into(),
            value: value.into(),
            enabled: true,
        });
        cases.push(item);
    }
    let mut value = input();
    value.body = Some("\0".into());
    cases.push(value);
    for value in cases {
        assert_eq!(
            code(prepare_terminal_curl(value, &[]).err().unwrap()),
            "SSH_TERMINAL_HTTP_UNSUPPORTED_REQUEST"
        );
    }
    let mut value = input();
    value.body = Some("a".repeat(MAX_REQUEST_BYTES + 1));
    assert_eq!(
        code(prepare_terminal_curl(value, &[]).err().unwrap()),
        "SSH_TERMINAL_HTTP_REQUEST_TOO_LARGE"
    );
}

#[test]
fn binary_sse_compression_and_oversized_responses_are_rejected() {
    let prepared = prepared();
    for (headers, body) in [
        (
            "Content-Type: text/event-stream\r\n",
            b"event: hello".as_slice(),
        ),
        ("Content-Type: application/octet-stream\r\n", b"abc"),
        ("Content-Encoding: gzip\r\n", b"abc"),
        ("", b"\xff"),
        ("", b"nul\0body"),
    ] {
        assert_eq!(
            code(
                response::parse(&prepared, &wire(&prepared, 200, headers, body), 0)
                    .err()
                    .unwrap()
            ),
            "SSH_TERMINAL_HTTP_UNSUPPORTED_RESPONSE"
        );
    }
    let body = vec![b'a'; MAX_RESPONSE_BYTES + 1];
    assert_eq!(
        code(
            response::parse(&prepared, &wire(&prepared, 200, "", &body), 0)
                .err()
                .unwrap()
        ),
        "SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE"
    );
}

#[test]
fn curl_failure_uses_only_fixed_codes_even_with_sensitive_output() {
    let prepared = prepared();
    for (exit, expected) in [
        (127, "CURL_UNAVAILABLE"),
        (2, "CURL_UNSUPPORTED"),
        (125, "CURL_UNSUPPORTED"),
        (124, "CURL_BODY_UNSUPPORTED"),
        (6, "DNS_FAILED"),
        (7, "CONNECT_FAILED"),
        (28, "TIMEOUT"),
        (60, "TLS_FAILED"),
        (63, "RESPONSE_TOO_LARGE"),
        (18, "TRANSFER_FAILED"),
    ] {
        assert_eq!(
            code(
                response::parse(&prepared, b"Authorization: secret\nprivate endpoint", exit)
                    .err()
                    .unwrap()
            ),
            format!("SSH_TERMINAL_HTTP_{expected}")
        );
    }
}

#[test]
fn head_and_empty_custom_headers_have_explicit_semantics() {
    let mut request = input();
    request.method = "HEAD".into();
    request.headers.push(KeyValue {
        key: "x-empty".into(),
        value: String::new(),
        enabled: true,
    });
    let prepared = prepare_terminal_curl(request, &[]).unwrap();
    assert!(prepared.command.contains("\nhead\n"));
    assert!(!prepared.command.contains("data-raw ="));
    assert!(!prepared.command.contains("data-binary ="));
    assert!(prepared.command.contains("header = \"x-empty;\""));
    let response = response::parse(
        &prepared,
        &wire(
            &prepared,
            200,
            "Content-Length: 999\r\nContent-Type: application/octet-stream\r\n",
            b"",
        ),
        0,
    )
    .unwrap();
    assert!(response.body.is_empty());
    let large_headers = format!("X-Pad: {}\r\n", "x".repeat(MAX_HEADER_BYTES));
    assert_eq!(
        code(
            response::parse(&prepared, &wire(&prepared, 200, &large_headers, b""), 0)
                .err()
                .unwrap()
        ),
        "SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE"
    );
}

#[tokio::test]
async fn response_stays_raw_while_history_diagnostics_and_snapshot_are_redacted() {
    let service = super::super::tests::support::service().await;
    let prepared = prepared();
    let body = br#"{"token":"synthetic-token","ok":true}"#;
    let bytes = wire(
        &prepared,
        302,
        "Content-Type: application/json\r\nSet-Cookie: private=cookie\r\n",
        body,
    );
    let response = service
        .complete_terminal_curl(
            prepared,
            bytes,
            0,
            Duration::from_millis(42),
            false,
            Default::default(),
        )
        .await
        .unwrap();
    let timing = response.terminal_timing.as_ref().unwrap();
    assert_eq!(timing.curl_start_transfer_ms, Some(123));
    assert_eq!(timing.curl_total_ms, Some(200));
    assert_eq!(response.status, 302);
    assert_eq!(response.body.as_bytes(), body);
    assert!(!response.history_id.is_empty());
    assert_eq!(
        response.warnings,
        [
            "SSH_TERMINAL_HTTP_REDIRECT_NOT_FOLLOWED",
            "SSH_TERMINAL_HTTP_TERMINAL_RECOVERY_REQUIRED"
        ]
    );
    let diagnostic = response.clone().into_diagnostic_projection();
    assert!(!diagnostic.body.contains("synthetic-token"));
    assert!(!diagnostic
        .headers
        .iter()
        .any(|header| header.value.contains("private=cookie")));
    let row: (String,String,String) = sqlx::query_as("SELECT request_headers_json,response_headers_json,response_body_preview FROM api_history WHERE id=?")
        .bind(&response.history_id).fetch_one(service.db.pool()).await.unwrap();
    for text in [row.0, row.1, row.2] {
        assert!(!text.contains("synthetic-token"));
        assert!(!text.contains("private=cookie"));
    }
}
