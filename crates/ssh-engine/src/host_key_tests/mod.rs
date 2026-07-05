use super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_local_storage::LocalDb;

const WORKSPACE_A: &str = "ws-a";
const WORKSPACE_B: &str = "ws-b";

async fn test_store() -> HostKeyStore {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory");
    let db = LocalDb::from_pool(pool.clone());
    db.migrate().await.expect("run migrations");
    let now = Utc::now().to_rfc3339();
    for workspace_id in [WORKSPACE_A, WORKSPACE_B] {
        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, environment_type, mcp_policy,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?1, 0, 'dev', 'auto', ?2, ?2, 1, 'local')
            "#,
        )
        .bind(workspace_id)
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("insert workspace");
    }
    HostKeyStore::new(pool)
}

#[tokio::test]
async fn host_key_first_connect_records_fingerprint() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:abc123")
        .await
        .expect("first connect should record fingerprint");

    let stored = store
        .get_fingerprint(WORKSPACE_A, "example.com", 22)
        .await
        .expect("lookup fingerprint");
    assert_eq!(stored.as_deref(), Some("SHA256:abc123"));
}

#[tokio::test]
async fn host_key_matching_fingerprint_succeeds() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:abc123")
        .await
        .expect("first connect");

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:abc123")
        .await
        .expect("matching fingerprint should succeed");
}

#[tokio::test]
async fn host_key_mismatch_is_rejected() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:abc123")
        .await
        .expect("first connect");

    let result = store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:different456")
        .await;
    assert!(result.is_err(), "mismatched fingerprint must be rejected");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("host key verification failed"),
        "error should mention host key verification: {}",
        err_msg
    );
}

#[tokio::test]
async fn host_key_different_hosts_are_independent() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "host-a.example.com", 22, "SHA256:aaa")
        .await
        .expect("host a first connect");

    store
        .verify_or_record(WORKSPACE_A, "host-b.example.com", 22, "SHA256:bbb")
        .await
        .expect("host b first connect with different fingerprint");

    store
        .verify_or_record(WORKSPACE_A, "host-a.example.com", 22, "SHA256:aaa")
        .await
        .expect("host a still matches");
}

#[tokio::test]
async fn host_key_different_ports_are_independent() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:port22")
        .await
        .expect("port 22 first connect");

    store
        .verify_or_record(WORKSPACE_A, "example.com", 2222, "SHA256:port2222")
        .await
        .expect("port 2222 first connect with different fingerprint");
}

#[tokio::test]
async fn host_key_same_host_port_is_independent_per_workspace() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:workspace-a")
        .await
        .expect("workspace a first connect");
    store
        .verify_or_record(WORKSPACE_B, "example.com", 22, "SHA256:workspace-b")
        .await
        .expect("workspace b first connect");

    assert_eq!(
        store
            .get_fingerprint(WORKSPACE_A, "example.com", 22)
            .await
            .expect("workspace a fingerprint")
            .as_deref(),
        Some("SHA256:workspace-a")
    );
    assert_eq!(
        store
            .get_fingerprint(WORKSPACE_B, "example.com", 22)
            .await
            .expect("workspace b fingerprint")
            .as_deref(),
        Some("SHA256:workspace-b")
    );
}

#[tokio::test]
async fn host_key_delete_fingerprint_removes_record() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:abc123")
        .await
        .expect("first connect");

    let deleted = store
        .delete_fingerprint(WORKSPACE_A, "example.com", 22)
        .await
        .expect("delete fingerprint");
    assert!(deleted, "should have deleted an existing record");

    let stored = store
        .get_fingerprint(WORKSPACE_A, "example.com", 22)
        .await
        .expect("lookup after delete");
    assert!(stored.is_none(), "fingerprint should be gone");

    // Deleting again should return false (nothing to delete).
    let deleted_again = store
        .delete_fingerprint(WORKSPACE_A, "example.com", 22)
        .await
        .expect("delete again");
    assert!(!deleted_again, "no record to delete");
}

#[tokio::test]
async fn host_key_get_fingerprint_info_returns_metadata() {
    let store = test_store().await;

    // No record yet.
    let info = store
        .get_fingerprint_info(WORKSPACE_A, "example.com", 22)
        .await
        .expect("lookup before any record");
    assert!(info.is_none());

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:abc123")
        .await
        .expect("first connect");

    let info = store
        .get_fingerprint_info(WORKSPACE_A, "example.com", 22)
        .await
        .expect("lookup after record");
    let (fingerprint, created_at) = info.expect("should have fingerprint info");
    assert_eq!(fingerprint, "SHA256:abc123");
    assert!(!created_at.is_empty(), "created_at should be populated");
}

#[tokio::test]
async fn host_key_delete_allows_new_trust() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:old_key")
        .await
        .expect("first connect");

    // Mismatch would be rejected.
    let result = store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:new_key")
        .await;
    assert!(result.is_err(), "mismatch must be rejected");

    // After reset, a new fingerprint is accepted (TOFU).
    store
        .delete_fingerprint(WORKSPACE_A, "example.com", 22)
        .await
        .expect("reset fingerprint");

    store
        .verify_or_record(WORKSPACE_A, "example.com", 22, "SHA256:new_key")
        .await
        .expect("new trust after reset");

    let stored = store
        .get_fingerprint(WORKSPACE_A, "example.com", 22)
        .await
        .expect("lookup");
    assert_eq!(stored.as_deref(), Some("SHA256:new_key"));
}

#[tokio::test]
async fn list_all_returns_all_stored_fingerprints() {
    let store = test_store().await;

    store
        .verify_or_record(WORKSPACE_A, "host-a", 22, "SHA256:aaa")
        .await
        .expect("record host-a");
    store
        .verify_or_record(WORKSPACE_A, "host-b", 2222, "SHA256:bbb")
        .await
        .expect("record host-b");

    let all = store.list_all(WORKSPACE_A).await.expect("list all");
    assert_eq!(all.len(), 2);
    let hosts: Vec<&str> = all.iter().map(|e| e.host.as_str()).collect();
    assert!(hosts.contains(&"host-a"));
    assert!(hosts.contains(&"host-b"));
}

#[tokio::test]
async fn import_known_hosts_parses_valid_entries() {
    let store = test_store().await;
    // Use a real SSH RSA public key (truncated for test; valid base64).
    let key_data = "AAAAB3NzaC1yc2EAAAADAQABAAABgQC7";
    let content = format!("example.com ssh-rsa {}", key_data);

    let result = store
        .import_known_hosts(WORKSPACE_A, &content)
        .await
        .expect("import known_hosts");

    assert_eq!(result.imported, 1);
    assert_eq!(result.skipped, 0);
    assert!(result.errors.is_empty());

    // Verify it was stored.
    let fp = store
        .get_fingerprint(WORKSPACE_A, "example.com", 22)
        .await
        .expect("get fingerprint");
    assert!(fp.is_some());
    assert!(fp.unwrap().starts_with("SHA256:"));
}

#[tokio::test]
async fn import_known_hosts_skips_duplicates() {
    let store = test_store().await;
    let key_data = "AAAAB3NzaC1yc2EAAAADAQABAAABgQC7";
    let content = format!("example.com ssh-rsa {}", key_data);

    let result1 = store
        .import_known_hosts(WORKSPACE_A, &content)
        .await
        .expect("first import");
    assert_eq!(result1.imported, 1);

    let result2 = store
        .import_known_hosts(WORKSPACE_A, &content)
        .await
        .expect("second import");
    assert_eq!(result2.imported, 0);
    assert_eq!(result2.skipped, 1);
}

#[tokio::test]
async fn import_known_hosts_skips_comments_and_blank_lines() {
    let store = test_store().await;
    let content = "# This is a comment\n\n   \n# Another comment\n";
    let result = store
        .import_known_hosts(WORKSPACE_A, content)
        .await
        .expect("import");
    assert_eq!(result.imported, 0);
    assert_eq!(result.skipped, 0);
}

#[tokio::test]
async fn import_known_hosts_handles_bracketed_host_with_port() {
    let store = test_store().await;
    let key_data = "AAAAB3NzaC1yc2EAAAADAQABAAABgQC7";
    let content = format!("[myhost.com]:2222 ssh-ed25519 {}", key_data);

    let result = store
        .import_known_hosts(WORKSPACE_A, &content)
        .await
        .expect("import");
    assert_eq!(result.imported, 1);

    let fp = store
        .get_fingerprint(WORKSPACE_A, "myhost.com", 2222)
        .await
        .expect("get fingerprint");
    assert!(fp.is_some());
}

#[tokio::test]
async fn export_known_hosts_produces_valid_format() {
    let store = test_store().await;
    let key_data = "AAAAB3NzaC1yc2EAAAADAQABAAABgQC7";
    let content = format!("example.com ssh-rsa {}", key_data);

    store
        .import_known_hosts(WORKSPACE_A, &content)
        .await
        .expect("import");

    let (exported, count) = store.export_known_hosts(WORKSPACE_A).await.expect("export");
    assert_eq!(count, 1);
    assert!(exported.contains("example.com"));
    assert!(exported.contains("ssh-rsa"));
    assert!(exported.contains(key_data));
}

#[tokio::test]
async fn export_entries_without_key_data_are_comments() {
    let store = test_store().await;
    // Record without key data (old-style TOFU entry).
    store
        .record_fingerprint(WORKSPACE_A, "oldhost.com", 22, "SHA256:old")
        .await
        .expect("record");

    let (exported, count) = store.export_known_hosts(WORKSPACE_A).await.expect("export");
    assert_eq!(count, 0);
    assert!(exported.starts_with('#'));
    assert!(exported.contains("SHA256:old"));
}

#[test]
fn parse_known_hosts_line_valid() {
    let line = "example.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQC7";
    let entry = parse_known_hosts_line(line);
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.host, "example.com");
    assert_eq!(entry.port, 22);
    assert_eq!(entry.key_type, "ssh-rsa");
    assert!(entry.fingerprint.starts_with("SHA256:"));
}

#[test]
fn parse_known_hosts_line_bracketed_port() {
    let line = "[myhost.com]:2222 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA";
    let entry = parse_known_hosts_line(line);
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.host, "myhost.com");
    assert_eq!(entry.port, 2222);
}

#[test]
fn parse_known_hosts_line_invalid() {
    assert!(parse_known_hosts_line("# comment").is_none());
    assert!(parse_known_hosts_line("").is_none());
    assert!(parse_known_hosts_line("only one field").is_none());
    assert!(parse_known_hosts_line("host not-a-key-type AAAA").is_none());
}

#[test]
fn base64_roundtrip() {
    let input = b"Hello, World!";
    let encoded = base64_encode_nopad(input);
    let decoded = base64_decode(&encoded).unwrap();
    assert_eq!(decoded, input);
}
