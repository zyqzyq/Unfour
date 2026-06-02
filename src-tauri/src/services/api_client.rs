use crate::app_error::{AppError, AppResult};
use crate::local_db::LocalDb;
use crate::models::{
    ApiHistoryDetail, ApiHistoryItem, ApiRequestInput, ApiResponse, ApiSavedRequest, KeyValue,
};
use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, Method, Url};
use std::time::{Duration, Instant};
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
        let name = input
            .name
            .clone()
            .unwrap_or_else(|| format!("{} {}", input.method.to_uppercase(), input.url));
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO api_requests (
              id, workspace_id, name, method, url, headers_json, query_json, body,
              body_kind, created_at, updated_at, revision, sync_status
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, 1, 'local')
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(name)
        .bind(input.method.to_uppercase())
        .bind(input.url)
        .bind(serde_json::to_string(&redact_headers(&input.headers))?)
        .bind(serde_json::to_string(&input.query)?)
        .bind(input.body)
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
              id, workspace_id, name, method, url, headers_json, query_json, body, body_kind,
              created_at, updated_at, deleted_at, revision, sync_status, remote_id
            FROM api_requests
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY updated_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(items)
    }

    async fn get_saved_request(&self, id: &str) -> AppResult<ApiSavedRequest> {
        let saved = sqlx::query_as::<_, ApiSavedRequest>(
            r#"
            SELECT
              id, workspace_id, name, method, url, headers_json, query_json, body, body_kind,
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
        .bind(&input.body)
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
    headers
        .iter()
        .map(|item| {
            let key = item.key.to_ascii_lowercase();
            let sensitive = matches!(
                key.as_str(),
                "authorization" | "cookie" | "proxy-authorization" | "x-api-key" | "x-auth-token"
            );
            KeyValue {
                key: item.key.clone(),
                value: if sensitive {
                    "<redacted>".to_string()
                } else {
                    item.value.clone()
                },
                enabled: item.enabled,
            }
        })
        .collect()
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
}
