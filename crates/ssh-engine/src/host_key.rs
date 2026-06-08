use chrono::Utc;
use sqlx::SqlitePool;
use unfour_core::{AppError, AppResult};

/// Host-key verification using trust-on-first-use (TOFU).
///
/// On first connection to a host, the server's key fingerprint is recorded.
/// On subsequent connections, the stored fingerprint must match.
/// A mismatch is always rejected with a clear error.
#[derive(Clone)]
pub struct HostKeyStore {
    pool: SqlitePool,
}

impl HostKeyStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record the fingerprint for a host on first connection.
    pub async fn record_fingerprint(
        &self,
        host: &str,
        port: u16,
        fingerprint: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO ssh_host_keys (host, port, fingerprint, created_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(host)
        .bind(port as i64)
        .bind(fingerprint)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Look up the stored fingerprint for a host, if any.
    pub async fn get_fingerprint(&self, host: &str, port: u16) -> AppResult<Option<String>> {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT fingerprint FROM ssh_host_keys WHERE host = ?1 AND port = ?2",
        )
        .bind(host)
        .bind(port as i64)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    /// Verify or record a host key fingerprint.
    ///
    /// - If no fingerprint is stored, record the given one (first connection).
    /// - If a fingerprint is stored and matches, return Ok.
    /// - If a fingerprint is stored and does NOT match, return an error.
    pub async fn verify_or_record(
        &self,
        host: &str,
        port: u16,
        fingerprint: &str,
    ) -> AppResult<()> {
        match self.get_fingerprint(host, port).await? {
            None => {
                self.record_fingerprint(host, port, fingerprint).await?;
                Ok(())
            }
            Some(stored) if stored == fingerprint => Ok(()),
            Some(_) => Err(AppError::Config(format!(
                "SSH host key verification failed for {}:{}: \
                 server key fingerprint does not match the previously recorded \
                 fingerprint. This could indicate a man-in-the-middle attack.",
                host, port
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use unfour_local_storage::LocalDb;

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
        HostKeyStore::new(pool)
    }

    #[tokio::test]
    async fn host_key_first_connect_records_fingerprint() {
        let store = test_store().await;

        store
            .verify_or_record("example.com", 22, "SHA256:abc123")
            .await
            .expect("first connect should record fingerprint");

        let stored = store
            .get_fingerprint("example.com", 22)
            .await
            .expect("lookup fingerprint");
        assert_eq!(stored.as_deref(), Some("SHA256:abc123"));
    }

    #[tokio::test]
    async fn host_key_matching_fingerprint_succeeds() {
        let store = test_store().await;

        store
            .verify_or_record("example.com", 22, "SHA256:abc123")
            .await
            .expect("first connect");

        store
            .verify_or_record("example.com", 22, "SHA256:abc123")
            .await
            .expect("matching fingerprint should succeed");
    }

    #[tokio::test]
    async fn host_key_mismatch_is_rejected() {
        let store = test_store().await;

        store
            .verify_or_record("example.com", 22, "SHA256:abc123")
            .await
            .expect("first connect");

        let result = store
            .verify_or_record("example.com", 22, "SHA256:different456")
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
            .verify_or_record("host-a.example.com", 22, "SHA256:aaa")
            .await
            .expect("host a first connect");

        store
            .verify_or_record("host-b.example.com", 22, "SHA256:bbb")
            .await
            .expect("host b first connect with different fingerprint");

        store
            .verify_or_record("host-a.example.com", 22, "SHA256:aaa")
            .await
            .expect("host a still matches");
    }

    #[tokio::test]
    async fn host_key_different_ports_are_independent() {
        let store = test_store().await;

        store
            .verify_or_record("example.com", 22, "SHA256:port22")
            .await
            .expect("port 22 first connect");

        store
            .verify_or_record("example.com", 2222, "SHA256:port2222")
            .await
            .expect("port 2222 first connect with different fingerprint");
    }
}
