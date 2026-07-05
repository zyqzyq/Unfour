use chrono::Utc;
use sqlx::SqlitePool;
use unfour_core::models::SshKnownHostsImportResult;
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
        workspace_id: &str,
        host: &str,
        port: u16,
        fingerprint: &str,
    ) -> AppResult<()> {
        self.record_fingerprint_full(workspace_id, host, port, fingerprint, None, None)
            .await
    }

    /// Record the fingerprint with optional key type and public key data.
    pub async fn record_fingerprint_full(
        &self,
        workspace_id: &str,
        host: &str,
        port: u16,
        fingerprint: &str,
        key_type: Option<&str>,
        public_key_data: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO ssh_host_keys (
              workspace_id, host, port, fingerprint, key_type, public_key_data, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(workspace_id)
        .bind(host)
        .bind(port as i64)
        .bind(fingerprint)
        .bind(key_type)
        .bind(public_key_data)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Look up the stored fingerprint for a host, if any.
    pub async fn get_fingerprint(
        &self,
        workspace_id: &str,
        host: &str,
        port: u16,
    ) -> AppResult<Option<String>> {
        let row = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT fingerprint
            FROM ssh_host_keys
            WHERE workspace_id = ?1 AND host = ?2 AND port = ?3
            "#,
        )
        .bind(workspace_id)
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
        workspace_id: &str,
        host: &str,
        port: u16,
        fingerprint: &str,
    ) -> AppResult<()> {
        match self.get_fingerprint(workspace_id, host, port).await? {
            None => {
                self.record_fingerprint(workspace_id, host, port, fingerprint)
                    .await?;
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

    /// Remove the stored fingerprint for a host:port pair.
    ///
    /// Returns `true` if a record was deleted, `false` if no record existed.
    pub async fn delete_fingerprint(
        &self,
        workspace_id: &str,
        host: &str,
        port: u16,
    ) -> AppResult<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM ssh_host_keys
            WHERE workspace_id = ?1 AND host = ?2 AND port = ?3
            "#,
        )
        .bind(workspace_id)
        .bind(host)
        .bind(port as i64)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Return the stored fingerprint and the timestamp when it was recorded.
    pub async fn get_fingerprint_info(
        &self,
        workspace_id: &str,
        host: &str,
        port: u16,
    ) -> AppResult<Option<(String, String)>> {
        let row = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT fingerprint, created_at
            FROM ssh_host_keys
            WHERE workspace_id = ?1 AND host = ?2 AND port = ?3
            "#,
        )
        .bind(workspace_id)
        .bind(host)
        .bind(port as i64)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all stored host-key fingerprints.
    pub async fn list_all(&self, workspace_id: &str) -> AppResult<Vec<StoredHostKey>> {
        let rows = sqlx::query_as::<_, StoredHostKey>(
            r#"
            SELECT workspace_id, host, port, fingerprint, key_type, public_key_data, created_at
            FROM ssh_host_keys
            WHERE workspace_id = ?1
            ORDER BY created_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Import entries from OpenSSH known_hosts content.
    ///
    /// Parses each line, computes the SHA-256 fingerprint from the public key,
    /// and stores entries that are valid and not already present.
    pub async fn import_known_hosts(
        &self,
        workspace_id: &str,
        content: &str,
    ) -> AppResult<SshKnownHostsImportResult> {
        let mut imported = 0u32;
        let mut skipped = 0u32;
        let mut errors = Vec::new();

        for (line_number, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match parse_known_hosts_line(line) {
                Some(entry) => {
                    let existing = self
                        .get_fingerprint(workspace_id, &entry.host, entry.port)
                        .await?;
                    if existing.is_some() {
                        skipped += 1;
                        continue;
                    }
                    match self
                        .record_fingerprint_full(
                            workspace_id,
                            &entry.host,
                            entry.port,
                            &entry.fingerprint,
                            Some(&entry.key_type),
                            Some(&entry.public_key_data),
                        )
                        .await
                    {
                        Ok(()) => imported += 1,
                        Err(err) => {
                            errors.push(format!(
                                "line {}: failed to store {}: {}",
                                line_number + 1,
                                entry.host,
                                err
                            ));
                        }
                    }
                }
                None => {
                    skipped += 1;
                }
            }
        }

        Ok(SshKnownHostsImportResult {
            imported,
            skipped,
            errors,
        })
    }

    /// Export stored fingerprints to OpenSSH known_hosts format.
    ///
    /// Entries with stored public key data produce full known_hosts lines.
    /// Entries without public key data are exported as comments.
    pub async fn export_known_hosts(&self, workspace_id: &str) -> AppResult<(String, u32)> {
        let entries = self.list_all(workspace_id).await?;
        let mut lines = Vec::new();
        let mut count = 0u32;

        for entry in &entries {
            let host_port = if entry.port == 22 {
                entry.host.clone()
            } else {
                format!("[{}]:{}", entry.host, entry.port)
            };

            if let (Some(key_type), Some(key_data)) = (&entry.key_type, &entry.public_key_data) {
                lines.push(format!("{} {} {}", host_port, key_type, key_data));
                count += 1;
            } else {
                lines.push(format!(
                    "# {} {} (fingerprint only, no key data)",
                    host_port, entry.fingerprint
                ));
            }
        }

        let content = if lines.is_empty() {
            String::new()
        } else {
            let mut s = lines.join("\n");
            s.push('\n');
            s
        };

        Ok((content, count))
    }
}

/// A stored host-key record with all columns.
#[derive(Clone, sqlx::FromRow)]
pub struct StoredHostKey {
    pub workspace_id: String,
    pub host: String,
    pub port: i64,
    pub fingerprint: String,
    pub key_type: Option<String>,
    pub public_key_data: Option<String>,
    pub created_at: String,
}

struct ParsedKnownHostsEntry {
    host: String,
    port: u16,
    key_type: String,
    public_key_data: String,
    fingerprint: String,
}

fn parse_known_hosts_line(line: &str) -> Option<ParsedKnownHostsEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    let host_field = parts[0];
    let key_type = parts[1];
    let key_data = parts[2];

    // Validate key type looks like an SSH key type.
    if !key_type.starts_with("ssh-")
        && !key_type.starts_with("ecdsa-")
        && key_type != "sk-ssh-ed25519@openssh.com"
        && key_type != "sk-ecdsa-sha2-nistp256@openssh.com"
    {
        return None;
    }

    // Parse host:port from bracket notation or plain host.
    let (host, port) = if host_field.starts_with('[') {
        if let Some(bracket_end) = host_field.find(']') {
            let h = &host_field[1..bracket_end];
            let rest = &host_field[bracket_end + 1..];
            let p = if let Some(port_str) = rest.strip_prefix(':') {
                port_str.parse::<u16>().ok()?
            } else {
                22
            };
            (h.to_string(), p)
        } else {
            return None;
        }
    } else if host_field.contains(',') {
        // Skip entries with multiple hosts (hash groups, wildcards).
        return None;
    } else {
        (host_field.to_string(), 22)
    };

    if host.is_empty() {
        return None;
    }

    // Compute SHA-256 fingerprint from the base64 public key data.
    use sha2::{Digest, Sha256};
    let key_bytes = base64_decode(key_data).ok()?;
    let digest = Sha256::digest(&key_bytes);
    let fingerprint = format!("SHA256:{}", base64_encode_nopad(&digest));

    Some(ParsedKnownHostsEntry {
        host,
        port,
        key_type: key_type.to_string(),
        public_key_data: key_data.to_string(),
        fingerprint,
    })
}

fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    // Standard base64 with or without padding.
    let input = input.trim_end_matches('=');
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut buf = Vec::with_capacity(input.len() * 3 / 4);
    let mut accum: u32 = 0;
    let mut bits: u32 = 0;
    for &byte in input.as_bytes() {
        let val = match alphabet.iter().position(|&b| b == byte) {
            Some(v) => v as u32,
            None => return Err(()),
        };
        accum = (accum << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            buf.push((accum >> bits) as u8);
            accum &= (1 << bits) - 1;
        }
    }
    Ok(buf)
}

fn base64_encode_nopad(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() * 4 + 2) / 3);
    let chunks = input.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
#[path = "host_key_tests/mod.rs"]
mod host_key_tests;
