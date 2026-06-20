use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, Method, Url};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use unfour_core::models::{
    ApiCollection, ApiEnvironment, ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiResponse,
    ApiSavedRequest, KeyValue,
};
use unfour_core::redaction::{redact_json_body, redact_key_values};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiClientService {
    client: Client,
    db: LocalDb,
}

impl ApiClientService {
    pub fn new(db: LocalDb) -> Self {
        Self {
            client: Client::new(),
            db,
        }
    }

    pub async fn send(&self, input: ApiRequestInput) -> AppResult<ApiResponse> {
        validate_workspace_id(&input.workspace_id)?;
        let method = parse_method(&input.method)?;
        let environment = self
            .active_environment_variables(&input.workspace_id)
            .await?;
        let resolved = resolve_input(input.clone(), &environment)?;
        let url = build_url(&resolved.url, &resolved.query)?;
        let timeout =
            Duration::from_millis(input.timeout_ms.unwrap_or(60_000).clamp(1_000, 300_000));

        let mut builder = self
            .client
            .request(method.clone(), url.clone())
            .timeout(timeout);
        let mut has_content_type = false;

        for header in resolved.headers.iter().filter(|item| item.enabled) {
            if header.key.trim().eq_ignore_ascii_case("content-type") {
                has_content_type = true;
            }
            let name = HeaderName::from_bytes(header.key.trim().as_bytes()).map_err(|_| {
                AppError::Validation(format!("invalid header name: {}", header.key))
            })?;
            let value = HeaderValue::from_str(&header.value).map_err(|_| {
                AppError::Validation(format!("invalid header value for {}", header.key))
            })?;
            builder = builder.header(name, value);
        }

        if let Some(body) = resolved.body.clone().filter(|body| !body.is_empty()) {
            if input.body_kind == "json" && !has_content_type {
                builder = builder.header(CONTENT_TYPE, "application/json");
            }
            if !matches!(method, Method::GET | Method::HEAD) {
                builder = builder.body(body);
            }
        }

        let started = Instant::now();
        let response = builder.send().await?;
        let duration_ms = started.elapsed().as_millis();
        let status = response.status();
        let response_headers = response
            .headers()
            .iter()
            .map(|(key, value)| KeyValue {
                key: key.to_string(),
                value: value.to_str().unwrap_or("<binary>").to_string(),
                enabled: true,
            })
            .collect::<Vec<_>>();
        let body = response.text().await?;
        let history_id = self
            .insert_history(
                &resolved,
                status.as_u16(),
                duration_ms,
                &response_headers,
                &body,
            )
            .await?;

        Ok(ApiResponse {
            history_id,
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("").to_string(),
            headers: response_headers,
            body,
            duration_ms,
        })
    }

    pub async fn list_environments(&self, workspace_id: String) -> AppResult<Vec<ApiEnvironment>> {
        validate_workspace_id(&workspace_id)?;
        let rows = sqlx::query_as::<_, EnvironmentRow>(
            r#"
            SELECT id, workspace_id, name, variables_json, is_active, created_at, updated_at
            FROM api_environments
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY name COLLATE NOCASE
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows.into_iter().map(ApiEnvironment::from).collect())
    }

    pub async fn create_environment(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiEnvironment> {
        validate_workspace_id(&workspace_id)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::Validation(
                "environment name cannot be empty".to_string(),
            ));
        }

        // The first environment in a workspace becomes the active one.
        let existing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM api_environments WHERE workspace_id = ?1 AND deleted_at IS NULL",
        )
        .bind(&workspace_id)
        .fetch_one(self.db.pool())
        .await?;
        let is_active = existing == 0;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO api_environments (
              id, workspace_id, name, variables_json, is_active, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?5, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(&name)
        .bind(is_active)
        .bind(&now)
        .execute(self.db.pool())
        .await?;

        self.get_environment(&workspace_id, &id).await
    }

    pub async fn update_environment(
        &self,
        workspace_id: String,
        environment_id: String,
        name: String,
        variables: Vec<KeyValue>,
    ) -> AppResult<ApiEnvironment> {
        validate_workspace_id(&workspace_id)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::Validation(
                "environment name cannot be empty".to_string(),
            ));
        }
        validate_environment(&variables)?;
        let now = Utc::now().to_rfc3339();
        let variables_json = serde_json::to_string(&variables)?;

        let result = sqlx::query(
            r#"
            UPDATE api_environments
            SET name = ?1, variables_json = ?2, updated_at = ?3,
                revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?4 AND id = ?5 AND deleted_at IS NULL
            "#,
        )
        .bind(&name)
        .bind(&variables_json)
        .bind(&now)
        .bind(&workspace_id)
        .bind(&environment_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api environment".to_string()));
        }

        self.get_environment(&workspace_id, &environment_id).await
    }

    pub async fn delete_environment(
        &self,
        workspace_id: String,
        environment_id: String,
    ) -> AppResult<Vec<ApiEnvironment>> {
        validate_workspace_id(&workspace_id)?;
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE api_environments
            SET deleted_at = ?1, updated_at = ?1, is_active = 0,
                revision = revision + 1, sync_status = 'deleted'
            WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&environment_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api environment".to_string()));
        }

        self.list_environments(workspace_id).await
    }

    /// Activate one environment (clearing any other active flag) for the
    /// workspace. Passing `None`/empty deactivates all of them ("No
    /// Environment").
    pub async fn activate_environment(
        &self,
        workspace_id: String,
        environment_id: Option<String>,
    ) -> AppResult<Vec<ApiEnvironment>> {
        validate_workspace_id(&workspace_id)?;
        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.pool().begin().await?;

        let target_id = environment_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());

        if let Some(id) = target_id {
            let result = sqlx::query(
                r#"
                UPDATE api_environments
                SET is_active = 1, updated_at = ?1,
                    revision = revision + 1, sync_status = 'pending'
                WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
                "#,
            )
            .bind(&now)
            .bind(&workspace_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            if result.rows_affected() == 0 {
                // tx is dropped without commit -> rolled back.
                return Err(AppError::NotFound("api environment".to_string()));
            }

            sqlx::query(
                r#"
                UPDATE api_environments
                SET is_active = 0, updated_at = ?1,
                    revision = revision + 1, sync_status = 'pending'
                WHERE workspace_id = ?2 AND id != ?3 AND deleted_at IS NULL AND is_active = 1
                "#,
            )
            .bind(&now)
            .bind(&workspace_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE api_environments
                SET is_active = 0, updated_at = ?1,
                    revision = revision + 1, sync_status = 'pending'
                WHERE workspace_id = ?2 AND deleted_at IS NULL AND is_active = 1
                "#,
            )
            .bind(&now)
            .bind(&workspace_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        self.list_environments(workspace_id).await
    }

    async fn get_environment(
        &self,
        workspace_id: &str,
        environment_id: &str,
    ) -> AppResult<ApiEnvironment> {
        let row = sqlx::query_as::<_, EnvironmentRow>(
            r#"
            SELECT id, workspace_id, name, variables_json, is_active, created_at, updated_at
            FROM api_environments
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(environment_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(ApiEnvironment::from)
            .ok_or_else(|| AppError::NotFound("api environment".to_string()))
    }

    async fn active_environment_variables(&self, workspace_id: &str) -> AppResult<Vec<KeyValue>> {
        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT variables_json
            FROM api_environments
            WHERE workspace_id = ?1 AND is_active = 1 AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row
            .map(|(json,)| serde_json::from_str::<Vec<KeyValue>>(&json).unwrap_or_default())
            .unwrap_or_default())
    }

    pub async fn list_history(
        &self,
        workspace_id: String,
        limit: Option<i64>,
    ) -> AppResult<Vec<ApiHistoryItem>> {
        validate_workspace_id(&workspace_id)?;
        let limit = limit.unwrap_or(50).clamp(1, 200);

        let items = sqlx::query_as::<_, ApiHistoryItem>(
            r#"
            SELECT
              id, workspace_id, name, method, url, status, duration_ms, created_at,
              updated_at, deleted_at, revision, sync_status, remote_id
            FROM api_history
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(items)
    }

    pub async fn history_detail(
        &self,
        workspace_id: String,
        history_id: String,
    ) -> AppResult<ApiHistoryDetail> {
        validate_workspace_id(&workspace_id)?;
        if history_id.trim().is_empty() {
            return Err(AppError::Validation(
                "history id cannot be empty".to_string(),
            ));
        }

        let item = sqlx::query_as::<_, ApiHistoryDetail>(
            r#"
            SELECT
              id, workspace_id, name, method, url, request_headers_json, request_query_json,
              request_body, status, duration_ms, response_headers_json, response_body_preview,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
            FROM api_history
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(history_id)
        .fetch_optional(self.db.pool())
        .await?;

        item.ok_or_else(|| AppError::NotFound("api history".to_string()))
    }

    pub async fn save_request(&self, input: ApiRequestInput) -> AppResult<ApiSavedRequest> {
        validate_workspace_id(&input.workspace_id)?;
        let (name, folder_path, collection_id) = self.saved_request_fields(&input).await?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO api_requests (
              id, workspace_id, name, folder_path, collection_id, method, url, headers_json,
              query_json, body, body_kind, created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(name)
        .bind(folder_path)
        .bind(collection_id)
        .bind(input.method.to_uppercase())
        .bind(input.url)
        .bind(serde_json::to_string(&redact_headers(&input.headers))?)
        .bind(serde_json::to_string(&input.query)?)
        .bind(input.body.as_deref().map(|b| redact_json_body(b).0))
        .bind(input.body_kind)
        .bind(now)
        .execute(self.db.pool())
        .await?;

        self.get_saved_request(&id).await
    }

    pub async fn update_request(
        &self,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
    ) -> AppResult<ApiSavedRequest> {
        validate_workspace_id(&workspace_id)?;
        validate_workspace_id(&input.workspace_id)?;
        if workspace_id != input.workspace_id {
            return Err(AppError::Validation(
                "api request workspace mismatch".to_string(),
            ));
        }
        if request_id.trim().is_empty() {
            return Err(AppError::Validation(
                "api request id cannot be empty".to_string(),
            ));
        }

        let (name, folder_path, collection_id) = self.saved_request_fields(&input).await?;
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE api_requests
            SET name = ?1, folder_path = ?2, collection_id = ?3, method = ?4, url = ?5,
                headers_json = ?6, query_json = ?7, body = ?8, body_kind = ?9,
                updated_at = ?10, revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?11 AND id = ?12 AND deleted_at IS NULL
            "#,
        )
        .bind(name)
        .bind(folder_path)
        .bind(collection_id)
        .bind(input.method.to_uppercase())
        .bind(input.url)
        .bind(serde_json::to_string(&redact_headers(&input.headers))?)
        .bind(serde_json::to_string(&input.query)?)
        .bind(input.body.as_deref().map(|b| redact_json_body(b).0))
        .bind(input.body_kind)
        .bind(now)
        .bind(&workspace_id)
        .bind(&request_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api request".to_string()));
        }

        self.get_saved_request_for_workspace(&workspace_id, &request_id)
            .await
    }

    pub async fn list_saved_requests(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        validate_workspace_id(&workspace_id)?;

        let items = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, folder_path, collection_id, method, url, headers_json,
              query_json, body, body_kind, created_at, updated_at, deleted_at, revision,
              sync_status, remote_id
            FROM api_requests
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY COALESCE(folder_path, ''), updated_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(items)
    }

    pub async fn duplicate_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<ApiSavedRequest> {
        validate_workspace_id(&workspace_id)?;
        if request_id.trim().is_empty() {
            return Err(AppError::Validation(
                "api request id cannot be empty".to_string(),
            ));
        }

        let source = self
            .get_saved_request_for_workspace(&workspace_id, &request_id)
            .await?;
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let name = format!("{} Copy", source.name);

        sqlx::query(
            r#"
            INSERT INTO api_requests (
              id, workspace_id, name, folder_path, collection_id, method, url, headers_json,
              query_json, body, body_kind, created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(name)
        .bind(source.folder_path)
        .bind(source.collection_id)
        .bind(source.method)
        .bind(source.url)
        .bind(source.headers_json)
        .bind(source.query_json)
        .bind(source.body)
        .bind(source.body_kind)
        .bind(now)
        .execute(self.db.pool())
        .await?;

        self.get_saved_request_for_workspace(&workspace_id, &id)
            .await
    }

    pub async fn delete_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        validate_workspace_id(&workspace_id)?;
        if request_id.trim().is_empty() {
            return Err(AppError::Validation(
                "api request id cannot be empty".to_string(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE api_requests
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(&workspace_id)
        .bind(&request_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api request".to_string()));
        }

        self.list_saved_requests(workspace_id).await
    }

    pub async fn list_collections(&self, workspace_id: String) -> AppResult<Vec<ApiCollection>> {
        validate_workspace_id(&workspace_id)?;
        let rows = sqlx::query_as::<_, CollectionRow>(
            r#"
            SELECT id, workspace_id, name, description, folders_json, created_at, updated_at
            FROM api_collections
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY name COLLATE NOCASE
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows.into_iter().map(ApiCollection::from).collect())
    }

    pub async fn create_collection(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        validate_workspace_id(&workspace_id)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::Validation(
                "collection name cannot be empty".to_string(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO api_collections (
              id, workspace_id, name, created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?4, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(&name)
        .bind(&now)
        .execute(self.db.pool())
        .await?;

        self.get_collection(&workspace_id, &id).await
    }

    pub async fn rename_collection(
        &self,
        workspace_id: String,
        collection_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        validate_workspace_id(&workspace_id)?;
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::Validation(
                "collection name cannot be empty".to_string(),
            ));
        }
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE api_collections
            SET name = ?1, updated_at = ?2, revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?3 AND id = ?4 AND deleted_at IS NULL
            "#,
        )
        .bind(&name)
        .bind(&now)
        .bind(&workspace_id)
        .bind(&collection_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api collection".to_string()));
        }

        self.get_collection(&workspace_id, &collection_id).await
    }

    /// Add an (initially empty) folder to a collection. Folders persist on the
    /// collection so they remain visible without any saved request. Idempotent
    /// when the folder path already exists.
    pub async fn add_collection_folder(
        &self,
        workspace_id: String,
        collection_id: String,
        folder_path: String,
    ) -> AppResult<ApiCollection> {
        validate_workspace_id(&workspace_id)?;
        let normalized = normalize_folder_path(Some(folder_path))?
            .ok_or_else(|| AppError::Validation("folder path cannot be empty".to_string()))?;

        let mut collection = self.get_collection(&workspace_id, &collection_id).await?;
        if !collection
            .folders
            .iter()
            .any(|folder| folder == &normalized)
        {
            collection.folders.push(normalized);
            collection.folders.sort();
            let folders_json = serde_json::to_string(&collection.folders)?;
            let now = Utc::now().to_rfc3339();
            sqlx::query(
                r#"
                UPDATE api_collections
                SET folders_json = ?1, updated_at = ?2,
                    revision = revision + 1, sync_status = 'pending'
                WHERE workspace_id = ?3 AND id = ?4 AND deleted_at IS NULL
                "#,
            )
            .bind(&folders_json)
            .bind(&now)
            .bind(&workspace_id)
            .bind(&collection_id)
            .execute(self.db.pool())
            .await?;
        }

        self.get_collection(&workspace_id, &collection_id).await
    }

    /// Soft-delete a collection and cascade soft-delete its saved requests in a
    /// single transaction.
    pub async fn delete_collection(
        &self,
        workspace_id: String,
        collection_id: String,
    ) -> AppResult<Vec<ApiCollection>> {
        validate_workspace_id(&workspace_id)?;
        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.pool().begin().await?;

        let result = sqlx::query(
            r#"
            UPDATE api_collections
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&collection_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            // tx is dropped without commit -> rolled back.
            return Err(AppError::NotFound("api collection".to_string()));
        }

        sqlx::query(
            r#"
            UPDATE api_requests
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE workspace_id = ?2 AND collection_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&collection_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        self.list_collections(workspace_id).await
    }

    /// Reassign a saved request to a different collection and/or folder. Passing
    /// `None` for `collection_id` moves the request back to "Unfiled".
    pub async fn move_request(
        &self,
        workspace_id: String,
        request_id: String,
        collection_id: Option<String>,
        folder_path: Option<String>,
    ) -> AppResult<ApiSavedRequest> {
        validate_workspace_id(&workspace_id)?;
        if request_id.trim().is_empty() {
            return Err(AppError::Validation(
                "api request id cannot be empty".to_string(),
            ));
        }
        let collection_id = normalize_collection_id(collection_id);
        let folder_path = normalize_folder_path(folder_path)?;
        // Reject a move into a collection that does not belong to the workspace.
        if let Some(target) = &collection_id {
            self.get_collection(&workspace_id, target).await?;
        }
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE api_requests
            SET collection_id = ?1, folder_path = ?2, updated_at = ?3,
                revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?4 AND id = ?5 AND deleted_at IS NULL
            "#,
        )
        .bind(collection_id)
        .bind(folder_path)
        .bind(&now)
        .bind(&workspace_id)
        .bind(&request_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api request".to_string()));
        }

        self.get_saved_request_for_workspace(&workspace_id, &request_id)
            .await
    }

    async fn get_collection(
        &self,
        workspace_id: &str,
        collection_id: &str,
    ) -> AppResult<ApiCollection> {
        let row = sqlx::query_as::<_, CollectionRow>(
            r#"
            SELECT id, workspace_id, name, description, folders_json, created_at, updated_at
            FROM api_collections
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(collection_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(ApiCollection::from)
            .ok_or_else(|| AppError::NotFound("api collection".to_string()))
    }

    async fn saved_request_fields(
        &self,
        input: &ApiRequestInput,
    ) -> AppResult<(String, Option<String>, Option<String>)> {
        let folder_path = normalize_folder_path(input.folder_path.clone())?;
        let collection_id = normalize_collection_id(input.collection_id.clone());
        if let Some(target) = &collection_id {
            self.get_collection(&input.workspace_id, target).await?;
        }
        let name = input
            .name
            .clone()
            .unwrap_or_else(|| format!("{} {}", input.method.to_uppercase(), input.url));

        Ok((name, folder_path, collection_id))
    }

    pub async fn get_saved_request(&self, id: &str) -> AppResult<ApiSavedRequest> {
        let saved = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, folder_path, collection_id, method, url, headers_json,
              query_json, body, body_kind, created_at, updated_at, deleted_at, revision,
              sync_status, remote_id
            FROM api_requests
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        saved.ok_or_else(|| AppError::NotFound("api request".to_string()))
    }

    async fn get_saved_request_for_workspace(
        &self,
        workspace_id: &str,
        id: &str,
    ) -> AppResult<ApiSavedRequest> {
        let saved = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, folder_path, collection_id, method, url, headers_json,
              query_json, body, body_kind, created_at, updated_at, deleted_at, revision,
              sync_status, remote_id
            FROM api_requests
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;

        saved.ok_or_else(|| AppError::NotFound("api request".to_string()))
    }

    async fn insert_history(
        &self,
        input: &ApiRequestInput,
        status: u16,
        duration_ms: u128,
        response_headers: &[KeyValue],
        response_body: &str,
    ) -> AppResult<String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let body_preview = response_body.chars().take(20_000).collect::<String>();

        sqlx::query(
            r#"
            INSERT INTO api_history (
              id, workspace_id, name, method, url, request_headers_json, request_query_json,
              request_body, status, duration_ms, response_headers_json, response_body_preview,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(&input.name)
        .bind(input.method.to_uppercase())
        .bind(&input.url)
        .bind(serde_json::to_string(&redact_headers(&input.headers))?)
        .bind(serde_json::to_string(&input.query)?)
        .bind(input.body.as_deref().map(|b| redact_json_body(b).0))
        .bind(i64::from(status))
        .bind(i64::try_from(duration_ms).unwrap_or(i64::MAX))
        .bind(serde_json::to_string(response_headers)?)
        .bind(body_preview)
        .bind(now)
        .execute(self.db.pool())
        .await?;

        Ok(id)
    }
}

fn parse_method(method: &str) -> AppResult<Method> {
    Method::from_bytes(method.trim().to_uppercase().as_bytes())
        .map_err(|_| AppError::Validation(format!("invalid HTTP method: {}", method)))
}

#[derive(sqlx::FromRow)]
struct EnvironmentRow {
    id: String,
    workspace_id: String,
    name: String,
    variables_json: String,
    is_active: bool,
    created_at: String,
    updated_at: String,
}

impl From<EnvironmentRow> for ApiEnvironment {
    fn from(row: EnvironmentRow) -> Self {
        ApiEnvironment {
            id: row.id,
            workspace_id: row.workspace_id,
            name: row.name,
            variables: serde_json::from_str(&row.variables_json).unwrap_or_default(),
            is_active: row.is_active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct CollectionRow {
    id: String,
    workspace_id: String,
    name: String,
    description: Option<String>,
    folders_json: String,
    created_at: String,
    updated_at: String,
}

impl From<CollectionRow> for ApiCollection {
    fn from(row: CollectionRow) -> Self {
        ApiCollection {
            id: row.id,
            workspace_id: row.workspace_id,
            name: row.name,
            description: row.description,
            folders: serde_json::from_str(&row.folders_json).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn normalize_collection_id(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn validate_environment(variables: &[KeyValue]) -> AppResult<()> {
    let mut seen = HashSet::new();
    for variable in variables {
        let key = variable.key.trim();
        if key.is_empty() {
            continue;
        }
        let valid = key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'));
        if !valid {
            return Err(AppError::Validation(format!(
                "invalid environment variable name: {}",
                variable.key
            )));
        }
        if variable.enabled && !seen.insert(key.to_ascii_lowercase()) {
            return Err(AppError::Validation(format!(
                "duplicate environment variable name: {}",
                variable.key
            )));
        }
    }

    Ok(())
}

fn build_url(raw_url: &str, query: &[KeyValue]) -> AppResult<Url> {
    let mut url = Url::parse(raw_url.trim())
        .map_err(|_| AppError::Validation(format!("invalid URL: {}", raw_url)))?;

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

fn redact_headers(headers: &[KeyValue]) -> Vec<KeyValue> {
    redact_key_values(
        headers.to_vec(),
        |item| &item.key,
        |item, value| {
            item.value = value;
        },
    )
}

fn validate_workspace_id(workspace_id: &str) -> AppResult<()> {
    if workspace_id.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn normalize_folder_path(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim().trim_matches('/').trim_matches('\\');
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > 160 {
        return Err(AppError::Validation(
            "api request folder path must be 160 characters or fewer".to_string(),
        ));
    }
    if trimmed
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | '"' | '|'))
    {
        return Err(AppError::Validation(format!(
            "invalid api request folder path: {}",
            value
        )));
    }

    Ok(Some(trimmed.replace('\\', "/")))
}

fn resolve_input(
    mut input: ApiRequestInput,
    environment: &[KeyValue],
) -> AppResult<ApiRequestInput> {
    input.url = resolve_template(&input.url, environment)?;
    input.headers = resolve_key_values(&input.headers, environment)?;
    input.query = resolve_key_values(&input.query, environment)?;
    input.body = input
        .body
        .map(|body| resolve_template(&body, environment))
        .transpose()?;

    Ok(input)
}

fn resolve_key_values(items: &[KeyValue], environment: &[KeyValue]) -> AppResult<Vec<KeyValue>> {
    items
        .iter()
        .map(|item| {
            Ok(KeyValue {
                key: resolve_template(&item.key, environment)?,
                value: resolve_template(&item.value, environment)?,
                enabled: item.enabled,
            })
        })
        .collect()
}

fn resolve_template(value: &str, environment: &[KeyValue]) -> AppResult<String> {
    let mut output = value.to_string();
    for variable in environment
        .iter()
        .filter(|item| item.enabled && !item.key.trim().is_empty())
    {
        let token = format!("{{{{{}}}}}", variable.key.trim());
        output = output.replace(&token, &variable.value);
    }

    if let Some(start) = output.find("{{") {
        if let Some(end) = output[start + 2..].find("}}") {
            let name = &output[start + 2..start + 2 + end];
            return Err(AppError::Validation(format!(
                "missing environment variable: {}",
                name
            )));
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn service() -> ApiClientService {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory sqlite");
        let db = LocalDb::from_pool(pool);
        db.migrate().await.expect("run migrations");
        seed_workspace(&db, "workspace-a").await;
        seed_workspace(&db, "workspace-b").await;
        ApiClientService::new(db)
    }

    async fn seed_workspace(db: &LocalDb, workspace_id: &str) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO workspaces (
              id, name, is_default, last_opened_at, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, ?1, 0, ?2, ?2, ?2, 1, 'local')
            "#,
        )
        .bind(workspace_id)
        .bind(&now)
        .execute(db.pool())
        .await
        .expect("insert workspace");

        sqlx::query(
            r#"
            INSERT INTO workspace_settings (
              workspace_id, layout_json, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, '{}', ?2, ?2, 1, 'local')
            "#,
        )
        .bind(workspace_id)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("insert workspace settings");
    }

    #[tokio::test]
    async fn environment_lifecycle_create_update_activate_delete() {
        let service = service().await;

        // First environment auto-activates.
        let dev = service
            .create_environment("workspace-a".to_string(), "Dev".to_string())
            .await
            .expect("create dev");
        assert!(dev.is_active);

        // Second does not.
        let prod = service
            .create_environment("workspace-a".to_string(), "Prod".to_string())
            .await
            .expect("create prod");
        assert!(!prod.is_active);

        // Update variables on prod.
        let prod = service
            .update_environment(
                "workspace-a".to_string(),
                prod.id.clone(),
                "Prod".to_string(),
                vec![KeyValue {
                    key: "base_url".to_string(),
                    value: "https://api.example.test".to_string(),
                    enabled: true,
                }],
            )
            .await
            .expect("update prod");
        assert_eq!(prod.variables.len(), 1);

        // Activating prod clears dev's active flag (single-active invariant).
        let list = service
            .activate_environment("workspace-a".to_string(), Some(prod.id.clone()))
            .await
            .expect("activate prod");
        assert_eq!(list.iter().filter(|e| e.is_active).count(), 1);
        assert!(list.iter().find(|e| e.id == prod.id).unwrap().is_active);
        assert!(!list.iter().find(|e| e.id == dev.id).unwrap().is_active);

        let meta_rows: Vec<(String, i64, String)> = sqlx::query_as(
            r#"
            SELECT id, revision, sync_status
            FROM api_environments
            WHERE id = ?1 OR id = ?2
            "#,
        )
        .bind(&dev.id)
        .bind(&prod.id)
        .fetch_all(service.db.pool())
        .await
        .expect("environment metadata");
        let dev_meta = meta_rows
            .iter()
            .find(|(id, _, _)| id == &dev.id)
            .expect("dev metadata");
        let prod_meta = meta_rows
            .iter()
            .find(|(id, _, _)| id == &prod.id)
            .expect("prod metadata");
        assert_eq!(dev_meta.1, 2);
        assert_eq!(dev_meta.2, "pending");
        assert_eq!(prod_meta.1, 3);
        assert_eq!(prod_meta.2, "pending");

        // send() should resolve {{base_url}} from the active (prod) environment.
        let resolved = service
            .active_environment_variables("workspace-a")
            .await
            .expect("active vars");
        assert_eq!(resolved[0].value, "https://api.example.test");

        // Deleting the active environment leaves no active env ("No Environment").
        let remaining = service
            .delete_environment("workspace-a".to_string(), prod.id.clone())
            .await
            .expect("delete prod");
        assert!(remaining.iter().all(|e| !e.is_active));
        assert!(!remaining.iter().any(|e| e.id == prod.id));
    }

    #[tokio::test]
    async fn environment_is_scoped_to_workspace() {
        let service = service().await;
        let env_a = service
            .create_environment("workspace-a".to_string(), "Shared".to_string())
            .await
            .expect("create in a");

        let wrong = service
            .activate_environment("workspace-b".to_string(), Some(env_a.id.clone()))
            .await;
        assert!(matches!(wrong, Err(AppError::NotFound(_))));

        let list_b = service
            .list_environments("workspace-b".to_string())
            .await
            .expect("list b");
        assert!(list_b.is_empty());
    }

    #[tokio::test]
    async fn environment_update_rejects_duplicate_enabled_names() {
        let service = service().await;
        let env = service
            .create_environment("workspace-a".to_string(), "Dev".to_string())
            .await
            .expect("create");

        let result = service
            .update_environment(
                "workspace-a".to_string(),
                env.id,
                "Dev".to_string(),
                vec![
                    KeyValue {
                        key: "token".to_string(),
                        value: "a".to_string(),
                        enabled: true,
                    },
                    KeyValue {
                        key: "TOKEN".to_string(),
                        value: "b".to_string(),
                        enabled: true,
                    },
                ],
            )
            .await;
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[tokio::test]
    async fn save_request_redacts_sensitive_headers() {
        let service = service().await;

        let saved = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Secret request".to_string()),
                folder_path: None,
                collection_id: None,
                method: "GET".to_string(),
                url: "https://example.test".to_string(),
                headers: vec![KeyValue {
                    key: "Authorization".to_string(),
                    value: "Bearer secret".to_string(),
                    enabled: true,
                }],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save request");

        assert!(saved.headers_json.contains("<redacted>"));
        assert!(!saved.headers_json.contains("Bearer secret"));
    }

    #[tokio::test]
    async fn save_request_redacts_sensitive_body_fields() {
        let service = service().await;

        let saved = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Body redaction test".to_string()),
                folder_path: None,
                collection_id: None,
                method: "POST".to_string(),
                url: "https://example.test".to_string(),
                headers: vec![],
                query: vec![],
                body: Some(
                    r#"{"user":"alice","Authorization":"Bearer secret","data":"safe"}"#.to_string(),
                ),
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save request");

        assert!(
            saved.body.as_deref().unwrap_or("").contains("<redacted>"),
            "saved body should be redacted"
        );
        assert!(
            !saved
                .body
                .as_deref()
                .unwrap_or("")
                .contains("Bearer secret"),
            "saved body should not contain the secret"
        );
        assert!(
            saved.body.as_deref().unwrap_or("").contains("alice"),
            "non-sensitive values should be preserved"
        );
    }

    #[tokio::test]
    async fn save_request_preserves_non_json_body_unchanged() {
        let service = service().await;

        let plain_text = "plain text body with no json structure";
        let saved = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Plain text body".to_string()),
                folder_path: None,
                collection_id: None,
                method: "POST".to_string(),
                url: "https://example.test".to_string(),
                headers: vec![],
                query: vec![],
                body: Some(plain_text.to_string()),
                body_kind: "text".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save request");

        assert_eq!(saved.body.as_deref(), Some(plain_text));
    }

    #[tokio::test]
    async fn history_detail_is_scoped_to_workspace() {
        let service = service().await;
        sqlx::query(
            r#"
            INSERT INTO api_history (
              id, workspace_id, name, method, url, request_headers_json, request_query_json,
              request_body, status, duration_ms, response_headers_json, response_body_preview,
              created_at, updated_at, revision, sync_status
            )
            VALUES (
              'history-a', 'workspace-a', 'Health', 'GET', 'https://example.test',
              '[]', '[]', NULL, 200, 12, '[]', '{}', ?1, ?1, 1, 'local'
            )
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .execute(service.db.pool())
        .await
        .expect("insert history");

        let detail = service
            .history_detail("workspace-a".to_string(), "history-a".to_string())
            .await
            .expect("load detail");
        let wrong_workspace = service
            .history_detail("workspace-b".to_string(), "history-a".to_string())
            .await;

        assert_eq!(detail.method, "GET");
        assert!(matches!(wrong_workspace, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn resolve_input_applies_environment_across_request_parts() {
        let input = ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: Some("Templated".to_string()),
            folder_path: Some("Users".to_string()),
            collection_id: None,
            method: "POST".to_string(),
            url: "{{base_url}}/users/{{user_id}}".to_string(),
            headers: vec![KeyValue {
                key: "X-Tenant".to_string(),
                value: "{{tenant}}".to_string(),
                enabled: true,
            }],
            query: vec![KeyValue {
                key: "source".to_string(),
                value: "{{source}}".to_string(),
                enabled: true,
            }],
            body: Some("{\"user\":\"{{user_id}}\"}".to_string()),
            body_kind: "json".to_string(),
            timeout_ms: None,
        };

        let resolved = resolve_input(
            input,
            &[
                KeyValue {
                    key: "base_url".to_string(),
                    value: "https://api.example.test".to_string(),
                    enabled: true,
                },
                KeyValue {
                    key: "user_id".to_string(),
                    value: "42".to_string(),
                    enabled: true,
                },
                KeyValue {
                    key: "tenant".to_string(),
                    value: "ops".to_string(),
                    enabled: true,
                },
                KeyValue {
                    key: "source".to_string(),
                    value: "workspace".to_string(),
                    enabled: true,
                },
            ],
        )
        .expect("resolve input");

        assert_eq!(resolved.url, "https://api.example.test/users/42");
        assert_eq!(resolved.headers[0].value, "ops");
        assert_eq!(resolved.query[0].value, "workspace");
        assert_eq!(resolved.body.as_deref(), Some("{\"user\":\"42\"}"));
    }

    #[tokio::test]
    async fn resolve_input_reports_missing_environment_variable() {
        let input = ApiRequestInput {
            workspace_id: "workspace-a".to_string(),
            name: None,
            folder_path: None,
            collection_id: None,
            method: "GET".to_string(),
            url: "https://example.test/{{missing}}".to_string(),
            headers: vec![],
            query: vec![],
            body: None,
            body_kind: "json".to_string(),
            timeout_ms: None,
        };

        let result = resolve_input(input, &[]);

        assert!(
            matches!(result, Err(AppError::Validation(message)) if message.contains("missing"))
        );
    }

    #[tokio::test]
    async fn build_url_appends_enabled_query_pairs_only() {
        let url = build_url(
            "https://example.test/search?existing=true",
            &[
                KeyValue {
                    key: "q".to_string(),
                    value: "hello world".to_string(),
                    enabled: true,
                },
                KeyValue {
                    key: "disabled".to_string(),
                    value: "ignored".to_string(),
                    enabled: false,
                },
                KeyValue {
                    key: "".to_string(),
                    value: "ignored".to_string(),
                    enabled: true,
                },
            ],
        )
        .expect("build url");

        assert_eq!(
            url.as_str(),
            "https://example.test/search?existing=true&q=hello+world"
        );
    }

    #[tokio::test]
    async fn save_request_defaults_name_and_lists_by_workspace() {
        let service = service().await;
        service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: None,
                folder_path: Some("Users".to_string()),
                collection_id: None,
                method: "post".to_string(),
                url: "https://example.test/users".to_string(),
                headers: vec![],
                query: vec![],
                body: Some("{}".to_string()),
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save request");
        service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-b".to_string(),
                name: Some("Other workspace".to_string()),
                folder_path: None,
                collection_id: None,
                method: "GET".to_string(),
                url: "https://other.example.test".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save other request");

        let workspace_a = service
            .list_saved_requests("workspace-a".to_string())
            .await
            .expect("list workspace a");

        assert_eq!(workspace_a.len(), 1);
        assert_eq!(workspace_a[0].workspace_id, "workspace-a");
        assert_eq!(workspace_a[0].folder_path.as_deref(), Some("Users"));
        assert_eq!(workspace_a[0].name, "POST https://example.test/users");
    }

    #[tokio::test]
    async fn duplicate_request_copies_template_inside_workspace() {
        let service = service().await;
        let saved = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Create user".to_string()),
                folder_path: Some("Users/Admin".to_string()),
                collection_id: None,
                method: "POST".to_string(),
                url: "https://example.test/users".to_string(),
                headers: vec![KeyValue {
                    key: "Accept".to_string(),
                    value: "application/json".to_string(),
                    enabled: true,
                }],
                query: vec![],
                body: Some("{}".to_string()),
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save request");

        let duplicate = service
            .duplicate_request("workspace-a".to_string(), saved.id.clone())
            .await
            .expect("duplicate request");
        let wrong_workspace = service
            .duplicate_request("workspace-b".to_string(), saved.id.clone())
            .await;

        assert_ne!(duplicate.id, saved.id);
        assert_eq!(duplicate.name, "Create user Copy");
        assert_eq!(duplicate.folder_path.as_deref(), Some("Users/Admin"));
        assert_eq!(duplicate.url, saved.url);
        assert!(matches!(wrong_workspace, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn delete_request_soft_deletes_and_returns_remaining_workspace_items() {
        let service = service().await;
        let first = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("First".to_string()),
                folder_path: None,
                collection_id: None,
                method: "GET".to_string(),
                url: "https://example.test/first".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save first request");
        let second = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Second".to_string()),
                folder_path: Some("Folder".to_string()),
                collection_id: None,
                method: "GET".to_string(),
                url: "https://example.test/second".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save second request");

        let remaining = service
            .delete_request("workspace-a".to_string(), first.id.clone())
            .await
            .expect("delete request");
        let deleted_again = service
            .delete_request("workspace-a".to_string(), first.id)
            .await;

        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second.id);
        assert!(matches!(deleted_again, Err(AppError::NotFound(_))));
    }

    async fn save_in_collection(
        service: &ApiClientService,
        workspace_id: &str,
        name: &str,
        collection_id: Option<String>,
    ) -> ApiSavedRequest {
        service
            .save_request(ApiRequestInput {
                workspace_id: workspace_id.to_string(),
                name: Some(name.to_string()),
                folder_path: None,
                collection_id,
                method: "GET".to_string(),
                url: "https://example.test".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save request")
    }

    #[tokio::test]
    async fn collection_lifecycle_create_rename_delete_cascades_requests() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");

        let in_collection = save_in_collection(
            &service,
            "workspace-a",
            "Inside",
            Some(collection.id.clone()),
        )
        .await;
        assert_eq!(
            in_collection.collection_id.as_deref(),
            Some(collection.id.as_str())
        );
        let unfiled = save_in_collection(&service, "workspace-a", "Unfiled", None).await;

        let renamed = service
            .rename_collection(
                "workspace-a".to_string(),
                collection.id.clone(),
                "Public APIs".to_string(),
            )
            .await
            .expect("rename collection");
        assert_eq!(renamed.name, "Public APIs");

        let remaining_collections = service
            .delete_collection("workspace-a".to_string(), collection.id.clone())
            .await
            .expect("delete collection");
        assert!(remaining_collections.is_empty());

        // The request inside the collection was cascade soft-deleted; the
        // Unfiled request survives.
        let saved = service
            .list_saved_requests("workspace-a".to_string())
            .await
            .expect("list saved");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, unfiled.id);

        let deleted_again = service
            .delete_collection("workspace-a".to_string(), collection.id)
            .await;
        assert!(matches!(deleted_again, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn add_collection_folder_persists_and_dedupes() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");
        assert!(collection.folders.is_empty());

        let updated = service
            .add_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                "Auth".to_string(),
            )
            .await
            .expect("add folder");
        assert_eq!(updated.folders, vec!["Auth".to_string()]);

        let nested = service
            .add_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                "Auth/Tokens".to_string(),
            )
            .await
            .expect("add nested folder");
        assert_eq!(
            nested.folders,
            vec!["Auth".to_string(), "Auth/Tokens".to_string()]
        );

        // Adding the same folder again is a no-op (no duplicate).
        let again = service
            .add_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                "Auth".to_string(),
            )
            .await
            .expect("add duplicate folder");
        assert_eq!(again.folders.len(), 2);

        // Folders survive a reload via list.
        let listed = service
            .list_collections("workspace-a".to_string())
            .await
            .expect("list collections");
        assert_eq!(listed[0].folders.len(), 2);
    }

    #[tokio::test]
    async fn collection_is_scoped_to_workspace() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "Shared".to_string())
            .await
            .expect("create in a");

        let wrong = service
            .rename_collection(
                "workspace-b".to_string(),
                collection.id.clone(),
                "Renamed".to_string(),
            )
            .await;
        assert!(matches!(wrong, Err(AppError::NotFound(_))));

        let list_b = service
            .list_collections("workspace-b".to_string())
            .await
            .expect("list b");
        assert!(list_b.is_empty());

        let save_wrong_workspace = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-b".to_string(),
                name: Some("Wrong workspace".to_string()),
                folder_path: None,
                collection_id: Some(collection.id),
                method: "GET".to_string(),
                url: "https://example.test".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await;
        assert!(matches!(save_wrong_workspace, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn move_request_reassigns_collection_and_folder() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");
        let request = save_in_collection(&service, "workspace-a", "Movable", None).await;
        assert!(request.collection_id.is_none());

        let moved = service
            .move_request(
                "workspace-a".to_string(),
                request.id.clone(),
                Some(collection.id.clone()),
                Some("Sub".to_string()),
            )
            .await
            .expect("move into collection");
        assert_eq!(moved.collection_id.as_deref(), Some(collection.id.as_str()));
        assert_eq!(moved.folder_path.as_deref(), Some("Sub"));

        // Moving with None returns the request to "Unfiled".
        let unfiled = service
            .move_request("workspace-a".to_string(), request.id.clone(), None, None)
            .await
            .expect("move to unfiled");
        assert!(unfiled.collection_id.is_none());

        // Moving into a collection that does not exist is rejected.
        let missing = service
            .move_request(
                "workspace-a".to_string(),
                request.id,
                Some("does-not-exist".to_string()),
                None,
            )
            .await;
        assert!(matches!(missing, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn update_request_reuses_existing_record_and_validates_collection() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");
        let request =
            save_in_collection(&service, "workspace-a", "Original", Some(collection.id)).await;

        let updated = service
            .update_request(
                "workspace-a".to_string(),
                request.id.clone(),
                ApiRequestInput {
                    workspace_id: "workspace-a".to_string(),
                    name: Some("Updated".to_string()),
                    folder_path: Some("Moved".to_string()),
                    collection_id: None,
                    method: "POST".to_string(),
                    url: "https://example.test/updated".to_string(),
                    headers: vec![],
                    query: vec![],
                    body: Some("{}".to_string()),
                    body_kind: "json".to_string(),
                    timeout_ms: None,
                },
            )
            .await
            .expect("update request");

        assert_eq!(updated.id, request.id);
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.method, "POST");
        assert!(updated.collection_id.is_none());
        assert_eq!(updated.folder_path.as_deref(), Some("Moved"));

        let saved = service
            .list_saved_requests("workspace-a".to_string())
            .await
            .expect("list saved");
        assert_eq!(saved.len(), 1);

        let missing_collection = service
            .update_request(
                "workspace-a".to_string(),
                request.id,
                ApiRequestInput {
                    workspace_id: "workspace-a".to_string(),
                    name: Some("Bad collection".to_string()),
                    folder_path: None,
                    collection_id: Some("does-not-exist".to_string()),
                    method: "GET".to_string(),
                    url: "https://example.test".to_string(),
                    headers: vec![],
                    query: vec![],
                    body: None,
                    body_kind: "json".to_string(),
                    timeout_ms: None,
                },
            )
            .await;
        assert!(matches!(missing_collection, Err(AppError::NotFound(_))));
    }
}
