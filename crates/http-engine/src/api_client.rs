use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, Method, Url};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use sqlx::{Sqlite, Transaction};
use unfour_core::models::{
    ApiCollection, ApiEnvironment, ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiResponse,
    ApiCollectionFolder, ApiSavedRequest, KeyValue,
};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use uuid::Uuid;

const DEFAULT_AUTH_JSON: &str = r#"{"type":"none"}"#;
const DEFAULT_COLLECTION_NAME: &str = "My Collection";

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
        self.ensure_environment_name_available(&workspace_id, &name, None)
            .await?;

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
        self.get_environment(&workspace_id, &environment_id).await?;
        self.ensure_environment_name_available(&workspace_id, &name, Some(&environment_id))
            .await?;
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

    async fn ensure_environment_name_available(
        &self,
        workspace_id: &str,
        name: &str,
        exclude_id: Option<&str>,
    ) -> AppResult<()> {
        let existing: Option<(String,)> = match exclude_id {
            Some(environment_id) => {
                sqlx::query_as(
                    r#"
                    SELECT id
                    FROM api_environments
                    WHERE workspace_id = ?1
                      AND id != ?2
                      AND name COLLATE NOCASE = ?3
                      AND deleted_at IS NULL
                    LIMIT 1
                    "#,
                )
                .bind(workspace_id)
                .bind(environment_id)
                .bind(name)
                .fetch_optional(self.db.pool())
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"
                    SELECT id
                    FROM api_environments
                    WHERE workspace_id = ?1
                      AND name COLLATE NOCASE = ?2
                      AND deleted_at IS NULL
                    LIMIT 1
                    "#,
                )
                .bind(workspace_id)
                .bind(name)
                .fetch_optional(self.db.pool())
                .await?
            }
        };

        if existing.is_some() {
            return Err(AppError::Validation(format!(
                "environment name already exists in this workspace: {name}"
            )));
        }

        Ok(())
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
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let mut tx = self.db.pool().begin().await?;
        let (name, collection_id, parent_folder_id, sort_order) =
            self.saved_request_fields(&mut tx, &input, &now).await?;

        sqlx::query(
            r#"
            INSERT INTO api_requests (
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(name)
        .bind(collection_id)
        .bind(parent_folder_id)
        .bind(sort_order)
        .bind(input.auth_json.as_deref().unwrap_or(DEFAULT_AUTH_JSON))
        .bind(input.method.to_uppercase())
        .bind(input.url)
        .bind(serde_json::to_string(&input.headers)?)
        .bind(serde_json::to_string(&input.query)?)
        .bind(input.body.clone())
        .bind(input.body_kind)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
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

        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.pool().begin().await?;
        let (name, collection_id, parent_folder_id, _sort_order) =
            self.saved_request_fields(&mut tx, &input, &now).await?;

        let result = sqlx::query(
            r#"
            UPDATE api_requests
            SET name = ?1, collection_id = ?2, parent_folder_id = ?3, auth_json = ?4,
                method = ?5, url = ?6, headers_json = ?7, query_json = ?8,
                body = ?9, body_kind = ?10, updated_at = ?11,
                revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?12 AND id = ?13 AND deleted_at IS NULL
            "#,
        )
        .bind(name)
        .bind(collection_id)
        .bind(parent_folder_id)
        .bind(input.auth_json.as_deref().unwrap_or(DEFAULT_AUTH_JSON))
        .bind(input.method.to_uppercase())
        .bind(input.url)
        .bind(serde_json::to_string(&input.headers)?)
        .bind(serde_json::to_string(&input.query)?)
        .bind(input.body.clone())
        .bind(input.body_kind)
        .bind(now)
        .bind(&workspace_id)
        .bind(&request_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api request".to_string()));
        }

        tx.commit().await?;
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
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
            FROM api_requests
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY collection_id, COALESCE(parent_folder_id, ''), sort_order, updated_at DESC
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
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(name)
        .bind(source.collection_id)
        .bind(source.parent_folder_id)
        .bind(source.sort_order)
        .bind(source.auth_json)
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
            SELECT id, workspace_id, name, description, created_at, updated_at
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

    pub async fn list_collection_folders(
        &self,
        workspace_id: String,
        collection_id: Option<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        validate_workspace_id(&workspace_id)?;
        let collection_id = normalize_entity_id(collection_id);
        let rows = match collection_id {
            Some(collection_id) => {
                self.get_collection(&workspace_id, &collection_id).await?;
                sqlx::query_as::<_, ApiCollectionFolder>(
                    r#"
                    SELECT id, workspace_id, collection_id, parent_folder_id, name,
                           sort_order, created_at, updated_at, deleted_at
                    FROM api_collection_folders
                    WHERE workspace_id = ?1 AND collection_id = ?2 AND deleted_at IS NULL
                    ORDER BY COALESCE(parent_folder_id, ''), sort_order, name COLLATE NOCASE
                    "#,
                )
                .bind(&workspace_id)
                .bind(collection_id)
                .fetch_all(self.db.pool())
                .await?
            }
            None => {
                sqlx::query_as::<_, ApiCollectionFolder>(
                    r#"
                    SELECT id, workspace_id, collection_id, parent_folder_id, name,
                           sort_order, created_at, updated_at, deleted_at
                    FROM api_collection_folders
                    WHERE workspace_id = ?1 AND deleted_at IS NULL
                    ORDER BY collection_id, COALESCE(parent_folder_id, ''), sort_order, name COLLATE NOCASE
                    "#,
                )
                .bind(&workspace_id)
                .fetch_all(self.db.pool())
                .await?
            }
        };

        Ok(rows)
    }

    pub async fn create_collection_folder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        validate_workspace_id(&workspace_id)?;
        let name = normalize_folder_name(name)?;
        let parent_folder_id = normalize_entity_id(parent_folder_id);
        let mut tx = self.db.pool().begin().await?;
        self.ensure_collection_exists_tx(&mut tx, &workspace_id, &collection_id)
            .await?;
        if let Some(parent_id) = &parent_folder_id {
            let parent = self
                .get_collection_folder_tx(&mut tx, &workspace_id, parent_id)
                .await?;
            if parent.collection_id != collection_id {
                return Err(AppError::Validation(
                    "parent folder must belong to the target collection".to_string(),
                ));
            }
        }
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let sort_order = self
            .next_folder_sort_order_tx(&mut tx, &workspace_id, &collection_id, parent_folder_id.as_deref())
            .await?;

        sqlx::query(
            r#"
            INSERT INTO api_collection_folders (
              id, workspace_id, collection_id, parent_folder_id, name, sort_order,
              created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(&collection_id)
        .bind(&parent_folder_id)
        .bind(&name)
        .bind(sort_order)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        self.get_collection_folder(&workspace_id, &id).await
    }

    pub async fn rename_collection_folder(
        &self,
        workspace_id: String,
        folder_id: String,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        validate_workspace_id(&workspace_id)?;
        let name = normalize_folder_name(name)?;
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE api_collection_folders
            SET name = ?1, updated_at = ?2
            WHERE workspace_id = ?3 AND id = ?4 AND deleted_at IS NULL
            "#,
        )
        .bind(&name)
        .bind(&now)
        .bind(&workspace_id)
        .bind(&folder_id)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api collection folder".to_string()));
        }

        self.get_collection_folder(&workspace_id, &folder_id).await
    }

    pub async fn delete_collection_folder(
        &self,
        workspace_id: String,
        folder_id: String,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        validate_workspace_id(&workspace_id)?;
        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.pool().begin().await?;
        let folder = self
            .get_collection_folder_tx(&mut tx, &workspace_id, &folder_id)
            .await?;

        sqlx::query(
            r#"
            WITH RECURSIVE folder_tree(id) AS (
              SELECT id
              FROM api_collection_folders
              WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
              UNION ALL
              SELECT child.id
              FROM api_collection_folders child
              JOIN folder_tree parent ON child.parent_folder_id = parent.id
              WHERE child.workspace_id = ?2 AND child.deleted_at IS NULL
            )
            UPDATE api_collection_folders
            SET deleted_at = ?1, updated_at = ?1
            WHERE id IN (SELECT id FROM folder_tree)
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&folder_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            WITH RECURSIVE folder_tree(id) AS (
              SELECT id
              FROM api_collection_folders
              WHERE workspace_id = ?2 AND id = ?3
              UNION ALL
              SELECT child.id
              FROM api_collection_folders child
              JOIN folder_tree parent ON child.parent_folder_id = parent.id
              WHERE child.workspace_id = ?2
            )
            UPDATE api_requests
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1, sync_status = 'deleted'
            WHERE workspace_id = ?2
              AND parent_folder_id IN (SELECT id FROM folder_tree)
              AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&folder_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        self.list_collection_folders(workspace_id, Some(folder.collection_id))
            .await
    }

    pub async fn move_collection_folder(
        &self,
        workspace_id: String,
        folder_id: String,
        target_parent_folder_id: Option<String>,
    ) -> AppResult<ApiCollectionFolder> {
        validate_workspace_id(&workspace_id)?;
        let target_parent_folder_id = normalize_entity_id(target_parent_folder_id);
        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.pool().begin().await?;
        let folder = self
            .get_collection_folder_tx(&mut tx, &workspace_id, &folder_id)
            .await?;

        if target_parent_folder_id.as_deref() == Some(folder.id.as_str()) {
            return Err(AppError::Validation(
                "moving folder would create a cycle".to_string(),
            ));
        }
        if let Some(parent_id) = &target_parent_folder_id {
            let parent = self
                .get_collection_folder_tx(&mut tx, &workspace_id, parent_id)
                .await?;
            if parent.collection_id != folder.collection_id {
                return Err(AppError::Validation(
                    "target parent folder must belong to the same collection".to_string(),
                ));
            }
            if self
                .folder_contains_descendant_tx(&mut tx, &workspace_id, &folder.id, parent_id)
                .await?
            {
                return Err(AppError::Validation(
                    "moving folder would create a cycle".to_string(),
                ));
            }
        }

        let sort_order = self
            .next_folder_sort_order_tx(
                &mut tx,
                &workspace_id,
                &folder.collection_id,
                target_parent_folder_id.as_deref(),
            )
            .await?;

        let result = sqlx::query(
            r#"
            UPDATE api_collection_folders
            SET parent_folder_id = ?1, sort_order = ?2, updated_at = ?3
            WHERE workspace_id = ?4 AND id = ?5 AND deleted_at IS NULL
            "#,
        )
        .bind(&target_parent_folder_id)
        .bind(sort_order)
        .bind(&now)
        .bind(&workspace_id)
        .bind(&folder_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api collection folder".to_string()));
        }

        tx.commit().await?;
        self.get_collection_folder(&workspace_id, &folder_id).await
    }

    pub async fn reorder_collection_folders(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        folder_ids: Vec<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        validate_workspace_id(&workspace_id)?;
        let parent_folder_id = normalize_entity_id(parent_folder_id);
        let mut tx = self.db.pool().begin().await?;
        self.ensure_collection_exists_tx(&mut tx, &workspace_id, &collection_id)
            .await?;
        for (index, folder_id) in folder_ids.iter().enumerate() {
            let folder = self
                .get_collection_folder_tx(&mut tx, &workspace_id, folder_id)
                .await?;
            if folder.collection_id != collection_id || folder.parent_folder_id != parent_folder_id
            {
                return Err(AppError::Validation(
                    "folder reorder ids must be siblings in the target collection".to_string(),
                ));
            }
            sqlx::query(
                r#"
                UPDATE api_collection_folders
                SET sort_order = ?1
                WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
                "#,
            )
            .bind(i64::try_from(index).unwrap_or(i64::MAX))
            .bind(&workspace_id)
            .bind(folder_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.list_collection_folders(workspace_id, Some(collection_id)).await
    }

    async fn get_collection_folder(
        &self,
        workspace_id: &str,
        folder_id: &str,
    ) -> AppResult<ApiCollectionFolder> {
        let row = sqlx::query_as::<_, ApiCollectionFolder>(
            r#"
            SELECT id, workspace_id, collection_id, parent_folder_id, name,
                   sort_order, created_at, updated_at, deleted_at
            FROM api_collection_folders
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(folder_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.ok_or_else(|| AppError::NotFound("api collection folder".to_string()))
    }

    async fn get_collection_folder_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        workspace_id: &str,
        folder_id: &str,
    ) -> AppResult<ApiCollectionFolder> {
        let row = sqlx::query_as::<_, ApiCollectionFolder>(
            r#"
            SELECT id, workspace_id, collection_id, parent_folder_id, name,
                   sort_order, created_at, updated_at, deleted_at
            FROM api_collection_folders
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(folder_id)
        .fetch_optional(&mut **tx)
        .await?;

        row.ok_or_else(|| AppError::NotFound("api collection folder".to_string()))
    }

    async fn ensure_collection_exists_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        workspace_id: &str,
        collection_id: &str,
    ) -> AppResult<()> {
        let exists: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM api_collections
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(collection_id)
        .fetch_optional(&mut **tx)
        .await?;

        if exists.is_none() {
            return Err(AppError::NotFound("api collection".to_string()));
        }
        Ok(())
    }

    async fn first_or_create_collection_id_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        workspace_id: &str,
        now: &str,
    ) -> AppResult<String> {
        let existing: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM api_collections
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY name COLLATE NOCASE
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some((id,)) = existing {
            return Ok(id);
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO api_collections (
              id, workspace_id, name, created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?4, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(DEFAULT_COLLECTION_NAME)
        .bind(now)
        .execute(&mut **tx)
        .await?;

        Ok(id)
    }

    async fn next_folder_sort_order_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        workspace_id: &str,
        collection_id: &str,
        parent_folder_id: Option<&str>,
    ) -> AppResult<i64> {
        let max_order: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT MAX(sort_order)
            FROM api_collection_folders
            WHERE workspace_id = ?1
              AND collection_id = ?2
              AND parent_folder_id IS ?3
              AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(collection_id)
        .bind(parent_folder_id)
        .fetch_one(&mut **tx)
        .await?;

        Ok(max_order.unwrap_or(-1) + 1)
    }

    async fn next_request_sort_order_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        workspace_id: &str,
        collection_id: &str,
        parent_folder_id: Option<&str>,
    ) -> AppResult<i64> {
        let max_order: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT MAX(sort_order)
            FROM api_requests
            WHERE workspace_id = ?1
              AND collection_id = ?2
              AND parent_folder_id IS ?3
              AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(collection_id)
        .bind(parent_folder_id)
        .fetch_one(&mut **tx)
        .await?;

        Ok(max_order.unwrap_or(-1) + 1)
    }

    async fn folder_contains_descendant_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        workspace_id: &str,
        folder_id: &str,
        candidate_descendant_id: &str,
    ) -> AppResult<bool> {
        let found: Option<(String,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE folder_tree(id) AS (
              SELECT id
              FROM api_collection_folders
              WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
              UNION ALL
              SELECT child.id
              FROM api_collection_folders child
              JOIN folder_tree parent ON child.parent_folder_id = parent.id
              WHERE child.workspace_id = ?1 AND child.deleted_at IS NULL
            )
            SELECT id FROM folder_tree WHERE id = ?3 LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .bind(folder_id)
        .bind(candidate_descendant_id)
        .fetch_optional(&mut **tx)
        .await?;

        Ok(found.is_some())
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
            SET deleted_at = ?1, updated_at = ?1, revision = revision + 1,
                sync_status = 'deleted'
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
            UPDATE api_collection_folders
            SET deleted_at = ?1, updated_at = ?1
            WHERE workspace_id = ?2 AND collection_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&collection_id)
        .execute(&mut *tx)
        .await?;

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

    /// Reassign a saved request to a different collection and/or folder.
    pub async fn move_request(
        &self,
        workspace_id: String,
        request_id: String,
        collection_id: Option<String>,
        parent_folder_id: Option<String>,
    ) -> AppResult<ApiSavedRequest> {
        validate_workspace_id(&workspace_id)?;
        if request_id.trim().is_empty() {
            return Err(AppError::Validation(
                "api request id cannot be empty".to_string(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.pool().begin().await?;
        let (collection_id, parent_folder_id) = self
            .resolve_request_location_tx(
                &mut tx,
                &workspace_id,
                collection_id,
                parent_folder_id,
                &now,
            )
            .await?;
        let sort_order = self
            .next_request_sort_order_tx(
                &mut tx,
                &workspace_id,
                &collection_id,
                parent_folder_id.as_deref(),
            )
            .await?;

        let result = sqlx::query(
            r#"
            UPDATE api_requests
            SET collection_id = ?1, parent_folder_id = ?2, sort_order = ?3, updated_at = ?4,
                revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?5 AND id = ?6 AND deleted_at IS NULL
            "#,
        )
        .bind(&collection_id)
        .bind(&parent_folder_id)
        .bind(sort_order)
        .bind(&now)
        .bind(&workspace_id)
        .bind(&request_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("api request".to_string()));
        }

        tx.commit().await?;
        self.get_saved_request_for_workspace(&workspace_id, &request_id)
            .await
    }

    pub async fn reorder_requests(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        request_ids: Vec<String>,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        validate_workspace_id(&workspace_id)?;
        let parent_folder_id = normalize_entity_id(parent_folder_id);
        let mut tx = self.db.pool().begin().await?;
        self.ensure_collection_exists_tx(&mut tx, &workspace_id, &collection_id)
            .await?;
        if let Some(parent_id) = &parent_folder_id {
            let parent = self
                .get_collection_folder_tx(&mut tx, &workspace_id, parent_id)
                .await?;
            if parent.collection_id != collection_id {
                return Err(AppError::Validation(
                    "request reorder parent must belong to the target collection".to_string(),
                ));
            }
        }

        for (index, request_id) in request_ids.iter().enumerate() {
            let request = self
                .get_saved_request_for_workspace_tx(&mut tx, &workspace_id, request_id)
                .await?;
            if request.collection_id != collection_id || request.parent_folder_id != parent_folder_id
            {
                return Err(AppError::Validation(
                    "request reorder ids must be siblings in the target collection".to_string(),
                ));
            }
            sqlx::query(
                r#"
                UPDATE api_requests
                SET sort_order = ?1, revision = revision + 1, sync_status = 'pending'
                WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
                "#,
            )
            .bind(i64::try_from(index).unwrap_or(i64::MAX))
            .bind(&workspace_id)
            .bind(request_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.list_saved_requests(workspace_id).await
    }

    async fn get_collection(
        &self,
        workspace_id: &str,
        collection_id: &str,
    ) -> AppResult<ApiCollection> {
        let row = sqlx::query_as::<_, CollectionRow>(
            r#"
            SELECT id, workspace_id, name, description, created_at, updated_at
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
        tx: &mut Transaction<'_, Sqlite>,
        input: &ApiRequestInput,
        now: &str,
    ) -> AppResult<(String, String, Option<String>, i64)> {
        let (collection_id, parent_folder_id) = self
            .resolve_request_location_tx(
                tx,
                &input.workspace_id,
                input.collection_id.clone(),
                input.parent_folder_id.clone(),
                now,
            )
            .await?;
        let sort_order = self
            .next_request_sort_order_tx(
                tx,
                &input.workspace_id,
                &collection_id,
                parent_folder_id.as_deref(),
            )
            .await?;
        let name = input
            .name
            .clone()
            .unwrap_or_else(|| format!("{} {}", input.method.to_uppercase(), input.url));

        Ok((name, collection_id, parent_folder_id, sort_order))
    }

    async fn resolve_request_location_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        workspace_id: &str,
        collection_id: Option<String>,
        parent_folder_id: Option<String>,
        now: &str,
    ) -> AppResult<(String, Option<String>)> {
        let parent_folder_id = normalize_entity_id(parent_folder_id);
        let collection_id = normalize_collection_id(collection_id);

        if let Some(parent_id) = &parent_folder_id {
            let parent = self
                .get_collection_folder_tx(tx, workspace_id, parent_id)
                .await?;
            if let Some(collection_id) = collection_id {
                if collection_id != parent.collection_id {
                    return Err(AppError::Validation(
                        "parent folder must belong to the target collection".to_string(),
                    ));
                }
                return Ok((collection_id, Some(parent_id.clone())));
            }
            return Ok((parent.collection_id, Some(parent_id.clone())));
        }

        let collection_id = match collection_id {
            Some(collection_id) => {
                self.ensure_collection_exists_tx(tx, workspace_id, &collection_id)
                    .await?;
                collection_id
            }
            None => {
                self.first_or_create_collection_id_tx(tx, workspace_id, now)
                    .await?
            }
        };

        Ok((collection_id, None))
    }

    pub async fn get_saved_request(&self, id: &str) -> AppResult<ApiSavedRequest> {
        let saved = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
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
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
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

    async fn get_saved_request_for_workspace_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        workspace_id: &str,
        id: &str,
    ) -> AppResult<ApiSavedRequest> {
        let saved = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
            FROM api_requests
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(&mut **tx)
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
        .bind(serde_json::to_string(&input.headers)?)
        .bind(serde_json::to_string(&input.query)?)
        .bind(input.body.clone())
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
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn normalize_collection_id(value: Option<String>) -> Option<String> {
    normalize_entity_id(value)
}

fn normalize_entity_id(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn normalize_folder_name(value: String) -> AppResult<String> {
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

fn validate_workspace_id(workspace_id: &str) -> AppResult<()> {
    if workspace_id.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
        ));
    }
    Ok(())
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
    async fn environment_names_are_unique_within_workspace() {
        let service = service().await;
        let dev = service
            .create_environment("workspace-a".to_string(), "Dev".to_string())
            .await
            .expect("create dev");
        let prod = service
            .create_environment("workspace-a".to_string(), "Prod".to_string())
            .await
            .expect("create prod");

        let duplicate_create = service
            .create_environment("workspace-a".to_string(), "dev".to_string())
            .await;
        assert!(matches!(
            duplicate_create,
            Err(AppError::Validation(message)) if message.contains("already exists")
        ));

        let same_name_other_workspace = service
            .create_environment("workspace-b".to_string(), "dev".to_string())
            .await
            .expect("same name in another workspace");
        assert_eq!(same_name_other_workspace.workspace_id, "workspace-b");

        let duplicate_update = service
            .update_environment(
                "workspace-a".to_string(),
                prod.id.clone(),
                "DEV".to_string(),
                vec![],
            )
            .await;
        assert!(matches!(
            duplicate_update,
            Err(AppError::Validation(message)) if message.contains("already exists")
        ));

        let own_name_update = service
            .update_environment("workspace-a".to_string(), dev.id, "dev".to_string(), vec![])
            .await
            .expect("same environment can keep its name");
        assert_eq!(own_name_update.name, "dev");
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
    async fn save_request_preserves_non_json_body_unchanged() {
        let service = service().await;

        let plain_text = "plain text body with no json structure";
        let saved = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Plain text body".to_string()),
                parent_folder_id: None,
                collection_id: None,
                auth_json: None,
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
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
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
            parent_folder_id: None,
            collection_id: None,
            auth_json: None,
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
                parent_folder_id: None,
                collection_id: None,
                auth_json: Some(r#"{"type":"bearer","token":"{{api_token}}"}"#.to_string()),
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
                parent_folder_id: None,
                collection_id: None,
                auth_json: None,
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
        assert_eq!(workspace_a[0].parent_folder_id, None);
        assert_eq!(workspace_a[0].name, "POST https://example.test/users");
        assert_eq!(
            workspace_a[0].auth_json,
            r#"{"type":"bearer","token":"{{api_token}}"}"#
        );
    }

    #[tokio::test]
    async fn duplicate_request_copies_template_inside_workspace() {
        let service = service().await;
        let saved = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Create user".to_string()),
                parent_folder_id: None,
                collection_id: None,
                auth_json: None,
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
        assert_eq!(duplicate.parent_folder_id, saved.parent_folder_id);
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
                parent_folder_id: None,
                collection_id: None,
                auth_json: None,
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
                parent_folder_id: None,
                collection_id: None,
                auth_json: None,
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
                parent_folder_id: None,
                collection_id,
                auth_json: None,
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
        let collection_a = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection A");
        let collection_b = service
            .create_collection("workspace-a".to_string(), "Other".to_string())
            .await
            .expect("create collection B");

        let in_collection_a = save_in_collection(
            &service,
            "workspace-a",
            "Inside A",
            Some(collection_a.id.clone()),
        )
        .await;
        assert_eq!(in_collection_a.collection_id, collection_a.id);
        let in_collection_b = save_in_collection(
            &service,
            "workspace-a",
            "Inside B",
            Some(collection_b.id.clone()),
        )
        .await;
        assert_eq!(in_collection_b.collection_id, collection_b.id);

        let renamed = service
            .rename_collection(
                "workspace-a".to_string(),
                collection_a.id.clone(),
                "Public APIs".to_string(),
            )
            .await
            .expect("rename collection");
        assert_eq!(renamed.name, "Public APIs");

        let remaining_collections = service
            .delete_collection("workspace-a".to_string(), collection_a.id.clone())
            .await
            .expect("delete collection");
        assert_eq!(remaining_collections.len(), 1);
        assert_eq!(remaining_collections[0].id, collection_b.id);

        // The request inside the deleted collection was cascade soft-deleted;
        // the request in the other collection survives.
        let saved = service
            .list_saved_requests("workspace-a".to_string())
            .await
            .expect("list saved");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, in_collection_b.id);

        let deleted_again = service
            .delete_collection("workspace-a".to_string(), collection_a.id)
            .await;
        assert!(matches!(deleted_again, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn folders_and_parent_folder_requests_drive_collection_tree() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");

        let root_request = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Root".to_string()),
                parent_folder_id: None,
                collection_id: Some(collection.id.clone()),
                auth_json: None,
                method: "GET".to_string(),
                url: "https://example.test/root".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save root request");
        assert_eq!(root_request.parent_folder_id, None);
        assert_eq!(root_request.collection_id, collection.id);

        let auth = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                None,
                "Auth".to_string(),
            )
            .await
            .expect("create folder");
        let tokens = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                Some(auth.id.clone()),
                "Tokens".to_string(),
            )
            .await
            .expect("create child folder");
        let folders = service
            .list_collection_folders("workspace-a".to_string(), Some(collection.id.clone()))
            .await
            .expect("list folders");
        assert_eq!(folders.len(), 2);
        assert!(folders.iter().any(|folder| folder.id == auth.id));
        assert!(folders.iter().any(|folder| folder.id == tokens.id));

        let child_request = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Token".to_string()),
                parent_folder_id: Some(tokens.id.clone()),
                collection_id: Some(collection.id.clone()),
                auth_json: None,
                method: "GET".to_string(),
                url: "https://example.test/token".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save child request");
        assert_eq!(child_request.parent_folder_id.as_deref(), Some(tokens.id.as_str()));
        assert_eq!(child_request.collection_id, collection.id);

        let renamed = service
            .rename_collection_folder(
                "workspace-a".to_string(),
                tokens.id.clone(),
                "Session tokens".to_string(),
            )
            .await
            .expect("rename folder");
        assert_eq!(renamed.name, "Session tokens");
        let after_rename = service
            .get_saved_request(&child_request.id)
            .await
            .expect("child request remains after folder rename");
        assert_eq!(
            after_rename.parent_folder_id.as_deref(),
            Some(tokens.id.as_str())
        );

        let moved = service
            .move_collection_folder("workspace-a".to_string(), tokens.id.clone(), None)
            .await
            .expect("move folder to root");
        assert_eq!(moved.parent_folder_id, None);
        let after_move = service
            .get_saved_request(&child_request.id)
            .await
            .expect("child request remains after folder move");
        assert_eq!(after_move.parent_folder_id.as_deref(), Some(tokens.id.as_str()));
    }

    #[tokio::test]
    async fn folder_delete_recursively_soft_deletes_descendants_and_requests() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");
        let parent = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                None,
                "Parent".to_string(),
            )
            .await
            .expect("create parent folder");
        let child = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                Some(parent.id.clone()),
                "Child".to_string(),
            )
            .await
            .expect("create child folder");
        let request = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Nested".to_string()),
                parent_folder_id: Some(child.id.clone()),
                collection_id: Some(collection.id.clone()),
                auth_json: None,
                method: "GET".to_string(),
                url: "https://example.test/nested".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save nested request");

        service
            .delete_collection_folder("workspace-a".to_string(), parent.id.clone())
            .await
            .expect("delete parent recursively");

        let active_folders = service
            .list_collection_folders("workspace-a".to_string(), Some(collection.id))
            .await
            .expect("list active folders");
        let active_requests = service
            .list_saved_requests("workspace-a".to_string())
            .await
            .expect("list active requests");
        assert!(active_folders.is_empty());
        assert!(active_requests.iter().all(|saved| saved.id != request.id));

        let (deleted_folder_count,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM api_collection_folders
            WHERE id IN (?1, ?2) AND deleted_at IS NOT NULL
            "#,
        )
        .bind(&parent.id)
        .bind(&child.id)
        .fetch_one(service.db.pool())
        .await
        .expect("count soft-deleted folders");
        let (deleted_request_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM api_requests WHERE id = ?1 AND deleted_at IS NOT NULL",
        )
        .bind(&request.id)
        .fetch_one(service.db.pool())
        .await
        .expect("count soft-deleted request");

        assert_eq!(deleted_folder_count, 2);
        assert_eq!(deleted_request_count, 1);
    }

    #[tokio::test]
    async fn collection_delete_soft_deletes_folders_and_requests() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");
        let folder = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                None,
                "Auth".to_string(),
            )
            .await
            .expect("create folder");
        let request = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Nested".to_string()),
                parent_folder_id: Some(folder.id.clone()),
                collection_id: Some(collection.id.clone()),
                auth_json: None,
                method: "GET".to_string(),
                url: "https://example.test/nested".to_string(),
                headers: vec![],
                query: vec![],
                body: None,
                body_kind: "json".to_string(),
                timeout_ms: None,
            })
            .await
            .expect("save request");

        service
            .delete_collection("workspace-a".to_string(), collection.id.clone())
            .await
            .expect("delete collection");

        let (deleted_folder_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM api_collection_folders WHERE id = ?1 AND deleted_at IS NOT NULL",
        )
        .bind(&folder.id)
        .fetch_one(service.db.pool())
        .await
        .expect("count soft-deleted folder");
        let (deleted_request_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM api_requests WHERE id = ?1 AND deleted_at IS NOT NULL",
        )
        .bind(&request.id)
        .fetch_one(service.db.pool())
        .await
        .expect("count soft-deleted request");

        assert_eq!(deleted_folder_count, 1);
        assert_eq!(deleted_request_count, 1);
    }

    #[tokio::test]
    async fn folder_and_request_reorder_use_separate_sibling_sort_orders() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");
        let folder_b = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                None,
                "B".to_string(),
            )
            .await
            .expect("create folder b");
        let folder_a = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                None,
                "A".to_string(),
            )
            .await
            .expect("create folder a");
        let request_b = save_in_collection(
            &service,
            "workspace-a",
            "B Request",
            Some(collection.id.clone()),
        )
        .await;
        let request_a = save_in_collection(
            &service,
            "workspace-a",
            "A Request",
            Some(collection.id.clone()),
        )
        .await;

        let folders = service
            .reorder_collection_folders(
                "workspace-a".to_string(),
                collection.id.clone(),
                None,
                vec![folder_a.id.clone(), folder_b.id.clone()],
            )
            .await
            .expect("reorder folders");
        let requests = service
            .reorder_requests(
                "workspace-a".to_string(),
                collection.id.clone(),
                None,
                vec![request_a.id.clone(), request_b.id.clone()],
            )
            .await
            .expect("reorder requests");

        assert_eq!(
            folders.iter().map(|folder| folder.id.as_str()).collect::<Vec<_>>(),
            vec![folder_a.id.as_str(), folder_b.id.as_str()]
        );
        assert_eq!(
            requests
                .iter()
                .map(|request| request.id.as_str())
                .collect::<Vec<_>>(),
            vec![request_a.id.as_str(), request_b.id.as_str()]
        );
    }

    #[tokio::test]
    async fn moving_folder_to_self_or_descendant_is_rejected() {
        let service = service().await;
        let collection = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection");
        let auth = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id.clone(),
                None,
                "Auth".to_string(),
            )
            .await
            .expect("create folder");
        let tokens = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection.id,
                Some(auth.id.clone()),
                "Tokens".to_string(),
            )
            .await
            .expect("create child folder");

        let into_self = service
            .move_collection_folder(
                "workspace-a".to_string(),
                auth.id.clone(),
                Some(auth.id.clone()),
            )
            .await;
        let into_child = service
            .move_collection_folder(
                "workspace-a".to_string(),
                auth.id,
                Some(tokens.id),
            )
            .await;

        assert!(matches!(into_self, Err(AppError::Validation(message)) if message.contains("cycle")));
        assert!(matches!(into_child, Err(AppError::Validation(message)) if message.contains("cycle")));
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
                parent_folder_id: None,
                collection_id: Some(collection.id),
                auth_json: None,
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
        let collection_a = service
            .create_collection("workspace-a".to_string(), "APIs".to_string())
            .await
            .expect("create collection A");
        let collection_b = service
            .create_collection("workspace-a".to_string(), "Other".to_string())
            .await
            .expect("create collection B");
        let request =
            save_in_collection(&service, "workspace-a", "Movable", Some(collection_a.id.clone()))
                .await;
        assert_eq!(request.collection_id, collection_a.id);

        let target_folder = service
            .create_collection_folder(
                "workspace-a".to_string(),
                collection_b.id.clone(),
                None,
                "Sub".to_string(),
            )
            .await
            .expect("create target folder");

        let moved = service
            .move_request(
                "workspace-a".to_string(),
                request.id.clone(),
                Some(collection_b.id.clone()),
                Some(target_folder.id.clone()),
            )
            .await
            .expect("move into collection B");
        assert_eq!(moved.collection_id, collection_b.id);
        assert_eq!(moved.parent_folder_id.as_deref(), Some(target_folder.id.as_str()));

        // Moving with None moves the request to the first collection.
        let moved_to_first = service
            .move_request("workspace-a".to_string(), request.id.clone(), None, None)
            .await
            .expect("move to first collection");
        assert_eq!(moved_to_first.collection_id, collection_a.id);

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
        let collection_id = collection.id.clone();
        let request =
            save_in_collection(&service, "workspace-a", "Original", Some(collection_id.clone()))
                .await;

        let updated = service
            .update_request(
                "workspace-a".to_string(),
                request.id.clone(),
                ApiRequestInput {
                    workspace_id: "workspace-a".to_string(),
                    name: Some("Updated".to_string()),
                    parent_folder_id: None,
                    collection_id: None,
                    auth_json: None,
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
        assert_eq!(updated.collection_id, collection_id);
        assert_eq!(updated.parent_folder_id, None);

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
                    parent_folder_id: None,
                    collection_id: Some("does-not-exist".to_string()),
                    auth_json: None,
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
