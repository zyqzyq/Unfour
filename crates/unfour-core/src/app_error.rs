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
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("read-only: {0}")]
    ReadOnly(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("confirmation required: {message}")]
    ConfirmationRequired {
        message: String,
        details: serde_json::Value,
    },
}

impl AppError {
    /// Stable, safe error classification code. Contains no dynamic detail, so it
    /// is safe to surface to external consumers (e.g. the MCP/LLM boundary).
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Config(_) => "CONFIG_ERROR",
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::Http(_) => "HTTP_ERROR",
            AppError::Io(_) => "IO_ERROR",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Serialization(_) => "SERIALIZATION_ERROR",
            AppError::Tauri(_) => "TAURI_ERROR",
            AppError::Unsupported(_) => "UNSUPPORTED_OPERATION",
            AppError::Validation(_) => "VALIDATION_ERROR",
            AppError::ReadOnly(_) => "READ_ONLY_CONNECTION",
            AppError::Timeout(_) => "QUERY_TIMEOUT",
            AppError::ConfirmationRequired { .. } => "CONFIRMATION_REQUIRED",
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
