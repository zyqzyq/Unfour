use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, Method};
use std::time::{Duration, Instant};
use unfour_core::models::{
    ApiCollection, ApiCollectionFolder, ApiEnvironment, ApiHistoryDetail, ApiHistoryItem,
    ApiRequestInput, ApiResponse, ApiSavedRequest, KeyValue,
};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

#[path = "helpers.rs"]
mod helpers;
use helpers::{build_url, normalize_entity_id, parse_method, validate_workspace_id, CollectionRow};

mod collections;
mod domain;
mod execution;
mod history;
mod multipart;
mod openapi_export;
mod openapi_import;
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
