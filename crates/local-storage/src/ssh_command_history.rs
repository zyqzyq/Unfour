use unfour_core::models::{
    SshCommandHistoryEntry, SshCommandHistoryQuery, SshCommandHistoryRecordInput,
};
use unfour_core::redaction::redact_shell_command;
use unfour_core::{AppError, AppResult};

use crate::LocalDb;

const DEFAULT_HISTORY_LIMIT: i64 = 100;
const MAX_HISTORY_LIMIT: i64 = 200;
const MAX_STORED_PER_CONNECTION: i64 = 200;
const MAX_COMMAND_BYTES: usize = 32 * 1024;

#[derive(Clone)]
pub struct SshCommandHistoryService {
    db: LocalDb,
}

impl SshCommandHistoryService {
    pub fn new(db: LocalDb) -> Self {
        Self { db }
    }

    /// Record a command after its terminating Enter has been accepted by the
    /// SSH PTY. A consecutive duplicate for the same workspace and connection
    /// refreshes the existing row instead of creating noise.
    pub async fn record(
        &self,
        input: SshCommandHistoryRecordInput,
    ) -> AppResult<Option<SshCommandHistoryEntry>> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        if input.duration_ms.is_some_and(|duration| duration < 0) {
            return Err(AppError::Validation(
                "ssh command history duration cannot be negative".to_string(),
            ));
        }

        let command = input.command.trim().to_string();
        if command.is_empty() {
            return Ok(None);
        }
        if command.len() > MAX_COMMAND_BYTES {
            return Err(AppError::Validation(format!(
                "ssh command history command exceeds {MAX_COMMAND_BYTES} bytes"
            )));
        }
        let executed_at = input.executed_at.trim().to_string();
        if executed_at.is_empty() {
            return Err(AppError::Validation(
                "ssh command history executed_at cannot be empty".to_string(),
            ));
        }
        let (command, redacted) = redact_shell_command(&command);
        let cwd = input.cwd.and_then(trim_to_option);
        let session_id = input.session_id.and_then(trim_to_option);

        let mut transaction = self.db.pool().begin().await?;
        let previous: Option<(String, String, bool)> = sqlx::query_as(
            r#"
            SELECT id, command, redacted
            FROM ssh_command_history
            WHERE workspace_id = ?1 AND connection_id = ?2
            ORDER BY executed_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(&input.workspace_id)
        .bind(&input.connection_id)
        .fetch_optional(&mut *transaction)
        .await?;

        let id = match previous {
            Some((id, previous_command, previous_redacted))
                if previous_command == command && previous_redacted == redacted =>
            {
                sqlx::query(
                    r#"
                    UPDATE ssh_command_history
                    SET session_id = ?1, cwd = ?2, exit_code = ?3,
                        duration_ms = ?4, executed_at = ?5
                    WHERE id = ?6 AND workspace_id = ?7 AND connection_id = ?8
                    "#,
                )
                .bind(&session_id)
                .bind(&cwd)
                .bind(input.exit_code)
                .bind(input.duration_ms)
                .bind(&executed_at)
                .bind(&id)
                .bind(&input.workspace_id)
                .bind(&input.connection_id)
                .execute(&mut *transaction)
                .await?;
                id
            }
            _ => {
                let id = unfour_core::id::new_id();
                sqlx::query(
                    r#"
                    INSERT INTO ssh_command_history (
                      id, workspace_id, connection_id, session_id, command, cwd,
                      exit_code, duration_ms, redacted, executed_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    "#,
                )
                .bind(&id)
                .bind(&input.workspace_id)
                .bind(&input.connection_id)
                .bind(&session_id)
                .bind(&command)
                .bind(&cwd)
                .bind(input.exit_code)
                .bind(input.duration_ms)
                .bind(redacted)
                .bind(&executed_at)
                .execute(&mut *transaction)
                .await?;
                id
            }
        };

        let entry = sqlx::query_as::<_, SshCommandHistoryEntry>(
            r#"
            SELECT
              id, workspace_id, connection_id, session_id, command, cwd,
              exit_code, duration_ms, redacted, executed_at
            FROM ssh_command_history
            WHERE id = ?1 AND workspace_id = ?2
            "#,
        )
        .bind(id)
        .bind(&input.workspace_id)
        .fetch_one(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            DELETE FROM ssh_command_history
            WHERE workspace_id = ?1 AND connection_id = ?2
              AND id IN (
                SELECT id FROM ssh_command_history
                WHERE workspace_id = ?1 AND connection_id = ?2
                ORDER BY executed_at DESC, id DESC
                LIMIT -1 OFFSET ?3
              )
            "#,
        )
        .bind(&input.workspace_id)
        .bind(&input.connection_id)
        .bind(MAX_STORED_PER_CONNECTION)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(entry))
    }

    /// Stable workspace-scoped query boundary for desktop UI and future MCP
    /// adapters. Redacted placeholders are excluded unless explicitly asked
    /// for, and a connection filter keeps interactive recall host-specific.
    pub async fn list(
        &self,
        query: SshCommandHistoryQuery,
    ) -> AppResult<Vec<SshCommandHistoryEntry>> {
        validate_workspace_id(&query.workspace_id)?;
        if let Some(connection_id) = query.connection_id.as_deref() {
            validate_connection_id(connection_id)?;
        }
        let limit = query
            .limit
            .unwrap_or(DEFAULT_HISTORY_LIMIT)
            .clamp(1, MAX_HISTORY_LIMIT);
        let search = query.search.and_then(trim_to_option);
        let search_pattern = search.map(|value| format!("%{}%", escape_like(&value)));
        let since = query.since.and_then(trim_to_option);
        let until = query.until.and_then(trim_to_option);

        let rows = sqlx::query_as::<_, SshCommandHistoryEntry>(
            r#"
            SELECT
              id, workspace_id, connection_id, session_id, command, cwd,
              exit_code, duration_ms, redacted, executed_at
            FROM ssh_command_history
            WHERE workspace_id = ?1
              AND (?2 IS NULL OR connection_id = ?2)
              AND (?3 = 1 OR redacted = 0)
              AND (?4 IS NULL OR command LIKE ?4 ESCAPE '\')
              AND (?6 IS NULL OR executed_at >= ?6)
              AND (?7 IS NULL OR executed_at <= ?7)
            ORDER BY executed_at DESC, id DESC
            LIMIT ?5
            "#,
        )
        .bind(query.workspace_id)
        .bind(query.connection_id)
        .bind(query.include_redacted)
        .bind(search_pattern)
        .bind(limit)
        .bind(since)
        .bind(until)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }
}

fn trim_to_option(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn validate_workspace_id(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
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
#[path = "ssh_command_history_tests.rs"]
mod tests;
