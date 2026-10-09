//! Pure helpers for the API client service: HTTP method parsing, URL building,
//! environment template resolution, validation, and the sqlx row types used by
//! the storage layer. Extracted from `api_client.rs` to keep the service module
//! focused on orchestration; visibility is `pub(super)` so only the
//! `api_client` module (and its test submodules) can reach these items.

use reqwest::{Method, Url};
use unfour_core::models::{ApiCollection, KeyValue};
use unfour_core::{AppError, AppResult};

pub(super) fn parse_method(method: &str) -> AppResult<Method> {
    Method::from_bytes(method.trim().to_uppercase().as_bytes())
        .map_err(|_| AppError::Validation(format!("invalid HTTP method: {}", method)))
}

#[derive(sqlx::FromRow)]
pub(super) struct CollectionRow {
    pub(super) id: String,
    pub(super) workspace_id: String,
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
}

impl From<CollectionRow> for ApiCollection {
    fn from(row: CollectionRow) -> Self {
        ApiCollection {
            id: row.id,
            workspace_id: row.workspace_id,
            name: row.name,
            description: row.description,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(super) fn normalize_collection_id(value: Option<String>) -> Option<String> {
    normalize_entity_id(value)
}

pub(super) fn normalize_entity_id(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn normalize_folder_name(value: String) -> AppResult<String> {
    let name = value.trim();
    if name.is_empty() {
        return Err(AppError::Validation(
            "folder name cannot be empty".to_string(),
        ));
    }
    if name.chars().count() > 120 {
        return Err(AppError::Validation(
            "folder name must be 120 characters or fewer".to_string(),
        ));
    }
    if name
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | '"' | '|'))
    {
        return Err(AppError::Validation(format!(
            "invalid folder name: {}",
            value
        )));
    }
    Ok(name.to_string())
}

pub(super) fn build_url(raw_url: &str, query: &[KeyValue]) -> AppResult<Url> {
    let mut url =
        Url::parse(raw_url.trim()).map_err(|_| AppError::Validation("API_URL_INVALID".into()))?;

    {
        let mut pairs = url.query_pairs_mut();
        for item in query
            .iter()
            .filter(|item| item.enabled && !item.key.is_empty())
        {
            pairs.append_pair(&item.key, &item.value);
        }
    }

    Ok(url)
}

pub(super) fn validate_workspace_id(workspace_id: &str) -> AppResult<()> {
    if workspace_id.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
        ));
    }
    Ok(())
}
