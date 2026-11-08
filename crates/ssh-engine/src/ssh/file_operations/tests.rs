use super::*;
use std::io::Write;
use std::process::{Command, Stdio};
use unfour_core::models::SSH_PATCH_MAX_FILE_BYTES;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        // Keep child-process writes within the repository sandbox as well.
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ssh-file-tests");
        std::fs::create_dir_all(&root).unwrap();
        let path = root
            .canonicalize()
            .unwrap()
            .join(format!("unfour-patch-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn input(&self, search: &str, replace: &str) -> SshFilePatchInput {
        SshFilePatchInput {
            workspace_id: Uuid::new_v4().to_string(),
            connection_id: Uuid::new_v4().to_string(),
            path: self.0.join("--文件 'quoted'.txt").to_string_lossy().into(),
            search: search.into(),
            replace: replace.into(),
            allow_multiple: false,
            timeout_ms: None,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn run(input: &SshFilePatchInput) -> std::process::Output {
    run_payload(&patch::payload(input))
}
fn run_payload(payload: &[u8]) -> std::process::Output {
    let executable = if cfg!(windows) { "python" } else { "python3" };
    // Execute the very same -c expression, with no argv payload or '--'.
    let command = patch::command();
    let expression = command
        .strip_prefix("python3 -c '")
        .unwrap()
        .strip_suffix('\'')
        .unwrap()
        .replace("'\\''", "'");
    let mut child = Command::new(executable)
        .args(["-c", &expression])
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "ascii")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python 3 is required for patch integration tests");
    child.stdin.take().unwrap().write_all(payload).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn patch_incomplete_stdin_never_modifies_target() {
    let fixture = Fixture::new();
    let input = fixture.input("original", "replacement");
    std::fs::write(&input.path, "original").unwrap();
    let payload = patch::payload(&input);
    let result = run_payload(&payload[..payload.len() - 1]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(std::fs::read_to_string(&input.path).unwrap(), "original");
}

#[test]
fn file_limits_use_utf8_bytes_and_accept_exactly_four_mib() {
    let ws = Uuid::new_v4().to_string();
    let conn = Uuid::new_v4().to_string();
    assert!(validate_file_input(&ws, &conn, "/tmp/file", SSH_FILE_MAX_CONTENT_BYTES).is_ok());
    assert_eq!(
        validate_file_input(&ws, &conn, "/tmp/file", SSH_FILE_MAX_CONTENT_BYTES + 1)
            .unwrap_err()
            .code(),
        "SSH_FILE_CONTENT_TOO_LARGE"
    );
    assert!(validate_file_input(&ws, &conn, "/tmp/file\n", 1).is_err());
    assert!(validate_file_input("", &conn, "/tmp/file", 1).is_err());
}

#[test]
fn patch_stdin_handles_large_unicode_quotes_crlf_and_preserves_bytes() {
    let fixture = Fixture::new();
    let input = fixture.input("old\r\n", &format!("{}'\"\n", "界".repeat(700_000)));
    std::fs::write(&input.path, b"prefix\r\nold\r\nsuffix\r\n").unwrap();
    let result = run(&input);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&input.path).unwrap(),
        format!("prefix\r\n{}suffix\r\n", input.replace)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("__UNFOUR_PATCHED__ 1"));
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    assert!(!patch::command().contains("old"));
    assert!(patch::command().chars().count() < 4096);
}

#[test]
fn patch_no_match_multiple_matches_and_delete_are_safe() {
    let fixture = Fixture::new();
    let mut input = fixture.input("a", "");
    std::fs::write(&input.path, "a a").unwrap();
    assert_eq!(run(&input).status.code(), Some(3));
    assert_eq!(std::fs::read_to_string(&input.path).unwrap(), "a a");
    input.allow_multiple = true;
    assert!(run(&input).status.success());
    assert_eq!(std::fs::read_to_string(&input.path).unwrap(), " ");
    assert_eq!(run(&input).status.code(), Some(3));
}

#[test]
fn patch_rejects_oversized_target_and_expansion_without_writing() {
    let fixture = Fixture::new();
    let mut input = fixture.input("a", "xx");
    input.allow_multiple = true;
    for size in [SSH_PATCH_MAX_FILE_BYTES, SSH_PATCH_MAX_FILE_BYTES + 1] {
        std::fs::write(&input.path, vec![b'a'; size]).unwrap();
        let result = run(&input);
        assert_eq!(result.status.code(), Some(4));
        assert_eq!(std::fs::metadata(&input.path).unwrap().len(), size as u64);
    }
}

#[test]
fn patch_invalid_utf8_does_not_leak_content_or_truncate_target() {
    let fixture = Fixture::new();
    let input = fixture.input("a", "secret-replacement");
    std::fs::write(&input.path, [255, 97]).unwrap();
    let result = run(&input);
    assert_eq!(result.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&result.stderr).contains("secret-replacement"));
    assert_eq!(std::fs::read(&input.path).unwrap(), [255, 97]);
}
