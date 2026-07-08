use chrono::Utc;
use unfour_core::models::{SshSessionEvent, SshSessionSummary};
use unfour_core::{AppError, AppResult};

use crate::LocalDb;

pub const TERMINAL_HISTORY_MAX_BYTES: usize = 256 * 1024;
const TERMINAL_HISTORY_SESSION_LIMIT: i64 = 20;

#[derive(Clone)]
pub struct TerminalHistoryService {
    db: LocalDb,
}

impl TerminalHistoryService {
    pub fn new(db: LocalDb) -> Self {
        Self { db }
    }

    pub async fn save_session(&self, summary: &SshSessionSummary) -> AppResult<()> {
        validate_identity(
            &summary.workspace_id,
            &summary.session_id,
            &summary.connection_id,
        )?;
        sqlx::query(
            r#"
            INSERT INTO ssh_terminal_history (
              workspace_id, session_id, connection_id, status, reconnect_attempt,
              auth_kind, host, username, cols, rows, content, byte_len,
              created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, '', 0, ?11, ?12)
            ON CONFLICT(workspace_id, session_id) DO UPDATE SET
              connection_id = excluded.connection_id,
              status = excluded.status,
              reconnect_attempt = excluded.reconnect_attempt,
              auth_kind = excluded.auth_kind,
              host = excluded.host,
              username = excluded.username,
              cols = excluded.cols,
              rows = excluded.rows,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(&summary.workspace_id)
        .bind(&summary.session_id)
        .bind(&summary.connection_id)
        .bind(&summary.status)
        .bind(summary.reconnect_attempt as i64)
        .bind(&summary.auth_kind)
        .bind(&summary.host)
        .bind(&summary.username)
        .bind(summary.cols as i64)
        .bind(summary.rows as i64)
        .bind(&summary.created_at)
        .bind(&summary.updated_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn append_output(
        &self,
        workspace_id: &str,
        session_id: &str,
        connection_id: &str,
        output: &str,
    ) -> AppResult<()> {
        validate_identity(workspace_id, session_id, connection_id)?;
        if output.is_empty() {
            return Ok(());
        }

        let redacted = redact_terminal_output(output);
        if redacted.is_empty() {
            return Ok(());
        }

        let current: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT content
            FROM ssh_terminal_history
            WHERE workspace_id = ?1 AND session_id = ?2 AND connection_id = ?3
            "#,
        )
        .bind(workspace_id)
        .bind(session_id)
        .bind(connection_id)
        .fetch_optional(self.db.pool())
        .await?;
        let Some((mut content,)) = current else {
            return Err(AppError::NotFound(
                "ssh terminal history session".to_string(),
            ));
        };

        content.push_str(&redacted);
        let content = retain_utf8_tail(&content, TERMINAL_HISTORY_MAX_BYTES);
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE ssh_terminal_history
            SET content = ?1, byte_len = ?2, updated_at = ?3
            WHERE workspace_id = ?4 AND session_id = ?5 AND connection_id = ?6
            "#,
        )
        .bind(&content)
        .bind(content.len() as i64)
        .bind(now)
        .bind(workspace_id)
        .bind(session_id)
        .bind(connection_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn update_session(&self, summary: &SshSessionSummary) -> AppResult<()> {
        validate_identity(
            &summary.workspace_id,
            &summary.session_id,
            &summary.connection_id,
        )?;
        sqlx::query(
            r#"
            UPDATE ssh_terminal_history
            SET status = ?1, reconnect_attempt = ?2, cols = ?3, rows = ?4, updated_at = ?5
            WHERE workspace_id = ?6 AND session_id = ?7 AND connection_id = ?8
            "#,
        )
        .bind(&summary.status)
        .bind(summary.reconnect_attempt as i64)
        .bind(summary.cols as i64)
        .bind(summary.rows as i64)
        .bind(&summary.updated_at)
        .bind(&summary.workspace_id)
        .bind(&summary.session_id)
        .bind(&summary.connection_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    pub async fn list_sessions(&self, workspace_id: &str) -> AppResult<Vec<SshSessionSummary>> {
        validate_workspace_id(workspace_id)?;
        let rows = sqlx::query_as::<_, PersistedSession>(
            r#"
            SELECT
              session_id, workspace_id, connection_id, status, reconnect_attempt,
              auth_kind, host, username, cols, rows, created_at, updated_at
            FROM ssh_terminal_history
            WHERE workspace_id = ?1
            ORDER BY updated_at DESC
            LIMIT ?2
            "#,
        )
        .bind(workspace_id)
        .bind(TERMINAL_HISTORY_SESSION_LIMIT)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SshSessionSummary {
                session_id: row.session_id,
                workspace_id: row.workspace_id,
                connection_id: row.connection_id,
                status: if matches!(
                    row.status.as_str(),
                    "connected" | "degraded" | "reconnecting"
                ) {
                    "disconnected".to_string()
                } else {
                    row.status
                },
                reconnect_attempt: row.reconnect_attempt.clamp(0, u8::MAX as i64) as u8,
                auth_kind: row.auth_kind,
                host: row.host,
                username: row.username,
                cols: row.cols.clamp(0, u16::MAX as i64) as u16,
                rows: row.rows.clamp(0, u16::MAX as i64) as u16,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
            .collect())
    }

    pub async fn hydrate(
        &self,
        workspace_id: &str,
        session_id: &str,
    ) -> AppResult<Vec<SshSessionEvent>> {
        validate_workspace_id(workspace_id)?;
        validate_session_id(session_id)?;
        let row: Option<(String, String)> = sqlx::query_as(
            r#"
            SELECT content, updated_at
            FROM ssh_terminal_history
            WHERE workspace_id = ?1 AND session_id = ?2
            "#,
        )
        .bind(workspace_id)
        .bind(session_id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(match row {
            Some((content, created_at)) if !content.is_empty() => vec![SshSessionEvent {
                session_id: session_id.to_string(),
                kind: "output".to_string(),
                data: content,
                created_at,
            }],
            _ => Vec::new(),
        })
    }

    /// Return a single session summary by id, or `None` if it is not present.
    /// Used to support idempotent repeated closes after the in-memory entry has
    /// been dropped (issue #4): a second close reads the persisted row instead
    /// of erroring.
    pub async fn get_session(
        &self,
        workspace_id: &str,
        session_id: &str,
    ) -> AppResult<Option<SshSessionSummary>> {
        validate_workspace_id(workspace_id)?;
        validate_session_id(session_id)?;
        let sessions = self.list_sessions(workspace_id).await?;
        Ok(sessions
            .into_iter()
            .find(|summary| summary.session_id == session_id))
    }

    pub async fn delete_connection_history(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> AppResult<()> {
        validate_workspace_id(workspace_id)?;
        validate_connection_id(connection_id)?;
        sqlx::query(
            "DELETE FROM ssh_terminal_history WHERE workspace_id = ?1 AND connection_id = ?2",
        )
        .bind(workspace_id)
        .bind(connection_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct PersistedSession {
    session_id: String,
    workspace_id: String,
    connection_id: String,
    status: String,
    reconnect_attempt: i64,
    auth_kind: String,
    host: String,
    username: String,
    cols: i64,
    rows: i64,
    created_at: String,
    updated_at: String,
}

fn retain_utf8_tail(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut start = value.len() - max_bytes;
    while !value.is_char_boundary(start) {
        start += 1;
    }
    value[start..].to_string()
}

fn redact_terminal_output(value: &str) -> String {
    value
        .split_inclusive('\n')
        .map(|segment| {
            let (line, ending) = if let Some(line) = segment.strip_suffix("\r\n") {
                (line, "\r\n")
            } else if let Some(line) = segment.strip_suffix('\n') {
                (line, "\n")
            } else {
                (segment, "")
            };
            let lower = line.to_ascii_lowercase();
            if lower.contains("authorization:")
                || lower.contains("cookie:")
                || lower.contains("proxy-authorization:")
                || lower.contains("x-api-key:")
                || lower.contains("x-auth-token:")
                || lower.contains("password=")
                || lower.contains("passphrase=")
                || lower.contains("credential_ref")
                || lower.contains("credentialref")
                || lower.contains("private key")
                || lower.contains("private-key")
            {
                format!("<redacted>{ending}")
            } else {
                segment.to_string()
            }
        })
        .collect()
}

fn validate_identity(workspace_id: &str, session_id: &str, connection_id: &str) -> AppResult<()> {
    validate_workspace_id(workspace_id)?;
    validate_session_id(session_id)?;
    validate_connection_id(connection_id)
}

fn validate_workspace_id(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_session_id(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::Validation(
            "ssh session id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_connection_id(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::Validation(
            "ssh connection id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "terminal_history_tests/mod.rs"]
mod terminal_history_tests;
