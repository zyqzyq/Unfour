use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, Method};
use sqlx::{Sqlite, Transaction};
use std::time::{Duration, Instant};
use unfour_core::models::{
    ApiCollection, ApiCollectionFolder, ApiEnvironment, ApiHistoryDetail, ApiHistoryItem,
    ApiRequestInput, ApiResponse, ApiSavedRequest, KeyValue,
};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

#[path = "helpers.rs"]
mod helpers;
use helpers::{
    build_url, normalize_collection_id, normalize_entity_id, normalize_folder_name, parse_method,
    resolve_input, validate_environment, validate_workspace_id, CollectionRow, EnvironmentRow,
};

mod collections;
mod environments;
mod execution;
mod history;
mod openapi_export;
mod requests;

const DEFAULT_AUTH_JSON: &str = r#"{"type":"none"}"#;
const DEFAULT_COLLECTION_NAME: &str = "My Collection";

#[derive(Clone)]
pub struct ApiClientService {
    pub(super) client: Client,
    pub(super) db: LocalDb,
}

impl ApiClientService {
    pub fn new(db: LocalDb) -> Self {
        Self {
            client: Client::new(),
            db,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "api_client_tests/mod.rs"]
mod tests;
