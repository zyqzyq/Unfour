use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, Method, Url};
use std::time::{Duration, Instant};
use unfour_core::models::{
    ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiResponse, ApiSavedRequest, KeyValue,
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

    pub async fn send(
        &self,
        input: ApiRequestInput,
        environment: &[KeyValue],
    ) -> AppResult<ApiResponse> {
        validate_workspace_id(&input.workspace_id)?;
        let method = parse_method(&input.method)?;
        let resolved = resolve_input(input.clone(), environment)?;
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
        let folder_path = normalize_folder_path(input.folder_path.clone())?;
        let name = input
            .name
            .clone()
            .unwrap_or_else(|| format!("{} {}", input.method.to_uppercase(), input.url));
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO api_requests (
              id, workspace_id, name, folder_path, method, url, headers_json, query_json, body,
              body_kind, created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(name)
        .bind(folder_path)
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

    pub async fn list_saved_requests(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        validate_workspace_id(&workspace_id)?;

        let items = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, folder_path, method, url, headers_json, query_json, body,
              body_kind, created_at, updated_at, deleted_at, revision, sync_status, remote_id
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
              id, workspace_id, name, folder_path, method, url, headers_json, query_json, body,
              body_kind, created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(name)
        .bind(source.folder_path)
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

    pub async fn get_saved_request(&self, id: &str) -> AppResult<ApiSavedRequest> {
        let saved = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, folder_path, method, url, headers_json, query_json, body,
              body_kind, created_at, updated_at, deleted_at, revision, sync_status, remote_id
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
              id, workspace_id, name, folder_path, method, url, headers_json, query_json, body,
              body_kind, created_at, updated_at, deleted_at, revision, sync_status, remote_id
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
              workspace_id, layout_json, env_json, created_at, updated_at,
              revision, sync_status
            )
            VALUES (?1, '{}', '[]', ?2, ?2, 1, 'local')
            "#,
        )
        .bind(workspace_id)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("insert workspace settings");
    }

    #[tokio::test]
    async fn save_request_redacts_sensitive_headers() {
        let service = service().await;

        let saved = service
            .save_request(ApiRequestInput {
                workspace_id: "workspace-a".to_string(),
                name: Some("Secret request".to_string()),
                folder_path: None,
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
}
