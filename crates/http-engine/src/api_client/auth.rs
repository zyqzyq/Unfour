use super::*;

impl ApiClientService {
    /// Materialize saved auth for non-UI callers. Explicit enabled request
    /// headers/query entries retain precedence, matching the frontend adapter.
    pub fn materialize_auth(&self, mut input: ApiRequestInput) -> AppResult<ApiRequestInput> {
        let auth: serde_json::Value =
            serde_json::from_str(input.auth_json.as_deref().unwrap_or(DEFAULT_AUTH_JSON))?;
        let field = |key: &str| -> AppResult<&str> {
            auth[key]
                .as_str()
                .ok_or_else(|| AppError::Validation("API_AUTH_INVALID".into()))
        };
        let (target, key, value) = match auth["type"].as_str().unwrap_or("none") {
            "none" => return Ok(input),
            "bearer" => (
                "header",
                "Authorization".to_string(),
                format!("Bearer {}", field("token")?),
            ),
            "basic" => {
                // Use reqwest's auth encoding without executing a request.
                let request = self
                    .client
                    .get("http://localhost/")
                    .basic_auth(field("username")?, Some(field("password")?))
                    .build()?;
                let value = request
                    .headers()
                    .get(reqwest::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| AppError::Validation("API_AUTH_INVALID".into()))?
                    .to_string();
                ("header", "Authorization".to_string(), value)
            }
            "api-key" => (
                auth["addTo"].as_str().unwrap_or("header"),
                field("key")?.to_string(),
                field("value")?.to_string(),
            ),
            _ => return Err(AppError::Validation("API_AUTH_UNSUPPORTED".into())),
        };
        let entries = match target {
            "header" => &mut input.headers,
            "query" => &mut input.query,
            _ => return Err(AppError::Validation("API_AUTH_INVALID".into())),
        };
        if !entries.iter().any(|item| {
            item.enabled
                && if target == "header" {
                    item.key.eq_ignore_ascii_case(&key)
                } else {
                    item.key == key
                }
        }) {
            entries.push(KeyValue {
                key,
                value,
                enabled: true,
            });
        }
        Ok(input)
    }
}
