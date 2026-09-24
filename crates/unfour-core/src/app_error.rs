use serde::ser::{SerializeStruct, Serializer};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP response status: {0}")]
    HttpStatus(u16),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("read-only: {0}")]
    ReadOnly(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("API request timed out: {0}")]
    ApiTimeout(String),
    #[error("API network error: {0}")]
    ApiNetwork(String),
    #[error("API request cancelled: {0}")]
    ApiCancelled(String),
    #[error("confirmation required: {message}")]
    ConfirmationRequired {
        message: String,
        details: serde_json::Value,
    },
    #[error("row conflict: {0}")]
    RowConflict(String),
    #[error(
        "SSH task cannot run because its latest remote state requires a newer compatible client"
    )]
    SshTaskIncompleteRemoteState,
}

impl AppError {
    /// Safe diagnostics for a database-engine failure.
    ///
    /// `sqlState` and `databaseMessage` are null when the driver does not
    /// provide them. This never returns `Display`/`to_string()`, which can
    /// embed DSNs and other connection material.
    pub fn database_error_details(&self) -> Option<serde_json::Value> {
        let AppError::Database(error) = self else {
            return None;
        };
        let (sql_state, database_message) = match error.as_database_error() {
            Some(database_error) => (
                sql_state_of(database_error),
                sanitize_database_message(database_error.message()),
            ),
            None => (None, None),
        };
        Some(serde_json::json!({
            "sqlState": sql_state,
            "databaseMessage": database_message,
        }))
    }

    /// Stable, safe error classification code. Contains no dynamic detail, so it
    /// is safe to surface to external consumers (e.g. the MCP/LLM boundary).
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Config(_) => "CONFIG_ERROR",
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::Http(_) => "HTTP_ERROR",
            AppError::HttpStatus(_) => "HTTP_STATUS_FAILED",
            AppError::Io(_) => "IO_ERROR",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Serialization(_) => "SERIALIZATION_ERROR",
            AppError::Unsupported(_) => "UNSUPPORTED_OPERATION",
            AppError::Validation(_) => "VALIDATION_ERROR",
            AppError::ReadOnly(_) => "READ_ONLY_CONNECTION",
            AppError::Timeout(_) => "QUERY_TIMEOUT",
            AppError::ApiTimeout(_) => "API_TIMEOUT",
            AppError::ApiNetwork(_) => "NETWORK_ERROR",
            AppError::ApiCancelled(_) => "API_CANCELLED",
            AppError::ConfirmationRequired { .. } => "CONFIRMATION_REQUIRED",
            AppError::RowConflict(_) => "ROW_CONFLICT",
            AppError::SshTaskIncompleteRemoteState => "SSH_TASK_INCOMPLETE_REMOTE_STATE",
        }
    }
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 3)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        if let AppError::ConfirmationRequired { details, .. } = self {
            state.serialize_field("details", details)?;
        }
        state.end()
    }
}

fn sql_state_of(error: &dyn sqlx::error::DatabaseError) -> Option<String> {
    if let Some(postgres) = error.try_downcast_ref::<sqlx::postgres::PgDatabaseError>() {
        return sql_state_token(postgres.code());
    }
    if let Some(mysql) = error.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>() {
        return mysql.code().and_then(sql_state_token);
    }
    None
}

fn sql_state_token(code: &str) -> Option<String> {
    let valid = code.len() == 5
        && code
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_uppercase());
    valid.then(|| code.to_string())
}

fn sanitize_database_message(message: &str) -> Option<String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return None;
    }
    let redacted = redact_sensitive_assignments(&redact_urls(
        &crate::redaction::redact_connection_string(trimmed),
    ));
    let redacted = redacted.trim();
    if redacted.is_empty() {
        None
    } else {
        Some(truncate_chars(redacted, 500))
    }
}

fn redact_urls(value: &str) -> String {
    value
        .split_inclusive(char::is_whitespace)
        .map(|token| {
            let trimmed = token.trim();
            if trimmed.contains("://") {
                token.replace(trimmed, crate::redaction::REDACTED_VALUE)
            } else {
                token.to_string()
            }
        })
        .collect()
}

fn redact_sensitive_assignments(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.find('=') {
        let key_start = rest[..index]
            .rfind(|ch: char| ch.is_whitespace() || ch == ';' || ch == ',' || ch == '&')
            .map(|found| found + 1)
            .unwrap_or(0);
        let key = rest[key_start..index].trim();
        output.push_str(&rest[..key_start]);
        if crate::redaction::is_sensitive_key(key) {
            output.push_str(key);
            output.push('=');
            output.push_str(crate::redaction::REDACTED_VALUE);
            let after = &rest[index + 1..];
            let value_end = after
                .find(|ch: char| ch.is_whitespace() || ch == ';' || ch == ',' || ch == '&')
                .unwrap_or(after.len());
            rest = &after[value_end..];
        } else {
            output.push_str(&rest[key_start..=index]);
            rest = &rest[index + 1..];
        }
    }
    output.push_str(rest);
    output
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_database_errors_have_no_database_details() {
        assert!(AppError::Validation("table name cannot be empty".into())
            .database_error_details()
            .is_none());
    }

    #[test]
    fn protocol_database_errors_leave_sqlstate_and_message_empty() {
        let error = AppError::Database(sqlx::Error::Protocol(
            "postgres://app:super-secret@db.internal:5432/app".into(),
        ));
        let details = error.database_error_details().expect("database error");
        assert!(details["sqlState"].is_null());
        assert!(details["databaseMessage"].is_null());
        assert!(!error.to_string().is_empty());
        assert_eq!(details["sqlState"], serde_json::Value::Null);
    }

    #[test]
    fn database_messages_drop_urls_and_secret_assignments() {
        let message = sanitize_database_message(
            "failed postgres://app:super-secret@db.internal/app password=hunter2 token=abc",
        )
        .expect("message");
        assert!(!message.contains("super-secret"));
        assert!(!message.contains("db.internal"));
        assert!(!message.contains("hunter2"));
        assert!(!message.contains("abc"));
        assert!(message.contains("failed"));
        assert!(message.contains("<redacted>"));
        assert!(sanitize_database_message("   ").is_none());
    }

    #[test]
    fn sql_state_token_keeps_five_character_ascii_codes() {
        assert_eq!(sql_state_token("42P01").as_deref(), Some("42P01"));
        assert_eq!(sql_state_token("23000").as_deref(), Some("23000"));
        assert_eq!(sql_state_token("P0001").as_deref(), Some("P0001"));
        assert_eq!(sql_state_token("XX000").as_deref(), Some("XX000"));
        assert!(sql_state_token("1").is_none());
        assert!(sql_state_token("2067").is_none());
        assert!(sql_state_token("42P011").is_none());
        assert!(sql_state_token("").is_none());
        assert!(sql_state_token("nope!").is_none());
        assert!(sql_state_token("42p01").is_none());
        assert!(sql_state_token("P000!").is_none());
    }
}
