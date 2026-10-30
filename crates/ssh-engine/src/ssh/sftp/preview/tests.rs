use super::*;

#[tokio::test]
async fn previews_utf8_bom_empty_and_plain_text() {
    for input in ["", "hello\r\n世界\t{}", "\u{feff}name=value\n"] {
        let result = read_bounded_text(&mut input.as_bytes()).await.unwrap();
        assert_eq!(result.content, input.trim_start_matches('\u{feff}'));
        assert_eq!(result.size, input.len() as u64);
    }
}

#[tokio::test]
async fn enforces_limit_on_actual_reads_even_when_file_grows() {
    let bytes = vec![b'x'; MAX_PREVIEW_BYTES * 3];
    let mut reader = bytes.as_slice();
    let error = read_bounded_text(&mut reader).await.err().unwrap();
    assert_eq!(error.code(), "SFTP_PREVIEW_TOO_LARGE");
    assert_eq!(bytes.len() - reader.len(), MAX_PREVIEW_BYTES + 1);
    let exact = read_bounded_text(&mut &bytes[..MAX_PREVIEW_BYTES])
        .await
        .unwrap();
    assert_eq!(exact.size, MAX_PREVIEW_BYTES as u64);
    assert!(check_size(MAX_PREVIEW_BYTES as u64 + 1).is_err());
}

#[tokio::test]
async fn rejects_binary_and_non_utf8_without_echoing_content() {
    for (bytes, code) in [
        (&b"secret\0text"[..], "SFTP_PREVIEW_BINARY"),
        (&b"\x89PNG\r\n\x1a\n"[..], "SFTP_PREVIEW_BINARY"),
        (&b"secret\xff"[..], "SFTP_PREVIEW_ENCODING"),
        (&b"\xff\xfeA\0"[..], "SFTP_PREVIEW_ENCODING"),
        (&b"\xfe\xff\0A"[..], "SFTP_PREVIEW_ENCODING"),
    ] {
        let error = read_bounded_text(&mut &bytes[..]).await.err().unwrap();
        assert_eq!(error.code(), code);
        assert!(!serde_json::to_string(&error).unwrap().contains("secret"));
    }
}

#[test]
fn classifies_permission_disconnect_and_timeout_without_remote_details() {
    use std::io::{Error, ErrorKind};
    for (error, code) in [
        (
            Error::new(ErrorKind::PermissionDenied, "secret"),
            "SFTP_PREVIEW_PERMISSION_DENIED",
        ),
        (
            Error::other("Permission denied: secret"),
            "SFTP_PREVIEW_PERMISSION_DENIED",
        ),
        (
            Error::new(ErrorKind::ConnectionReset, "secret"),
            "SFTP_PREVIEW_DISCONNECTED",
        ),
        (
            Error::other("RecvError: secret"),
            "SFTP_PREVIEW_DISCONNECTED",
        ),
        (Error::other("Timeout"), "SFTP_PREVIEW_TIMEOUT"),
        (Error::other("secret"), "SFTP_PREVIEW_READ_FAILED"),
    ] {
        let error = read_error(error);
        assert_eq!(error.code(), code);
        assert!(!serde_json::to_string(&error).unwrap().contains("secret"));
    }
}
