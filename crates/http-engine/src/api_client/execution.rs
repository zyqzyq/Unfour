use super::*;
use tokio_util::sync::CancellationToken;

impl ApiClientService {
    pub async fn send(&self, input: ApiRequestInput) -> AppResult<ApiResponse> {
        self.send_cancellable(input, CancellationToken::new()).await
    }

    pub async fn send_cancellable(
        &self,
        input: ApiRequestInput,
        cancellation: CancellationToken,
    ) -> AppResult<ApiResponse> {
        self.send_cancellable_with_secrets(input, cancellation, &[])
            .await
    }

    pub async fn send_cancellable_with_secrets(
        &self,
        input: ApiRequestInput,
        cancellation: CancellationToken,
        secrets: &[String],
    ) -> AppResult<ApiResponse> {
        self.send_with_client(&self.client, input, cancellation, secrets, Duration::ZERO)
            .await
    }

    /// The caller owns the authenticated loopback SOCKS lease until the body is consumed.
    /// All redirects use the same proxy and the destination name is resolved over SSH.
    pub async fn send_via_ssh_proxy(
        &self,
        input: ApiRequestInput,
        cancellation: CancellationToken,
        secrets: &[String],
        address: std::net::SocketAddr,
        username: &str,
        password: &str,
        route_started: Instant,
    ) -> AppResult<ApiResponse> {
        if !address.ip().is_loopback() || username.is_empty() || password.is_empty() {
            return Err(AppError::Validation("SSH_TUNNEL_PROXY_FAILED".into()));
        }
        let proxy = reqwest::Proxy::all(format!("socks5h://{address}"))
            .map_err(classify_http_error)?
            .basic_auth(username, password);
        let client = Client::builder()
            .no_proxy()
            .proxy(proxy)
            .build()
            .map_err(classify_http_error)?;
        self.send_with_client(
            &client,
            input,
            cancellation,
            secrets,
            route_started.elapsed(),
        )
        .await
    }

    async fn send_with_client(
        &self,
        client: &Client,
        input: ApiRequestInput,
        cancellation: CancellationToken,
        secrets: &[String],
        setup_time: Duration,
    ) -> AppResult<ApiResponse> {
        validate_workspace_id(&input.workspace_id)?;
        super::execution_snapshot::validate_replay_values(&input)?;
        let mut runtime_secrets = runtime_request_secret_values(&input)?;
        runtime_secrets.extend(secrets.iter().cloned());
        if input.body_kind == unfour_core::models::MULTIPART_BODY_KIND {
            unfour_core::models::parse_multipart_definition(input.body.as_deref())?;
        }
        let method = parse_method(&input.method).map_err(|error| match error {
            AppError::Validation(message) => AppError::Validation(
                super::history_redaction::scrub_values(&message, &runtime_secrets),
            ),
            other => other,
        })?;
        let url = build_url(&input.url, &input.query)?;
        retain_canonical_secret_values(&input, &url, &mut runtime_secrets);
        let secrets = runtime_secrets.as_slice();
        // Bound network execution, including SSH setup. Recording an already received
        // response in local history is outside the network deadline.
        let timeout = request_timeout_duration(input.timeout_ms)
            .map(|budget| {
                budget
                    .checked_sub(setup_time)
                    .filter(|remaining| !remaining.is_zero())
                    .ok_or_else(|| AppError::Validation("SSH_TUNNEL_TIMEOUT".into()))
            })
            .transpose()?;
        let request_id = unfour_diag::new_request_id();
        let request_fields = serde_json::json!({
            "request_id": request_id.as_str(),
            "method": super::history_redaction::scrub_values(method.as_str(), secrets),
            "host": super::history_redaction::scrub_values(url.host_str().unwrap_or(""), secrets),
            "path": super::history_redaction::scrub_values(url.path(), secrets),
        });

        let mut builder = client.request(method.clone(), url.clone());
        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }
        let mut has_content_type = false;

        for header in input.headers.iter().filter(|item| item.enabled) {
            if header.key.trim().eq_ignore_ascii_case("content-type") {
                if input.body_kind == unfour_core::models::MULTIPART_BODY_KIND {
                    continue;
                }
                has_content_type = true;
            }
            let name = HeaderName::from_bytes(header.key.trim().as_bytes())
                .map_err(|_| AppError::Validation("API_HEADER_NAME_INVALID".into()))?;
            let value = HeaderValue::from_str(&header.value)
                .map_err(|_| AppError::Validation("API_HEADER_VALUE_INVALID".into()))?;
            builder = builder.header(name, value);
        }

        if input.body_kind == unfour_core::models::MULTIPART_BODY_KIND {
            if !matches!(method, Method::GET | Method::HEAD) {
                let form = tokio::select! {
                    biased;
                    form = super::multipart::build_form(&input, secrets) => form?,
                    _ = cancellation.cancelled() => return Err(cancelled_error()),
                };
                builder = builder.multipart(form);
            }
        } else if let Some(body) = input.body.clone().filter(|body| !body.is_empty()) {
            if input.body_kind == "json" && !has_content_type {
                builder = builder.header(CONTENT_TYPE, "application/json");
            }
            if !matches!(method, Method::GET | Method::HEAD) {
                builder = builder.body(body);
            }
        }

        let request = builder.build().map_err(classify_http_error)?;
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let executed = super::execution_snapshot::executed_input(&input, &request);
        let mut executed_secrets = secrets.to_vec();
        executed_secrets.extend(runtime_request_secret_values(&executed)?);
        let secrets = executed_secrets.as_slice();
        let request_snapshot =
            super::execution_snapshot::snapshot(&executed, request.url(), secrets)?;
        let started = Instant::now();
        unfour_diag::log_operation_event(
            "api_request_started",
            "api_client",
            "send",
            "started",
            None,
            None,
            request_fields.clone(),
        );
        let response_result = tokio::select! {
            biased;
            response = client.execute(request) => response,
            _ = cancellation.cancelled() => return Err(cancelled_error()),
        };
        let mut response = match response_result {
            Ok(response) => response,
            Err(error) => {
                let error = classify_http_error(error);
                unfour_diag::log_operation_event(
                    "api_request_failed",
                    "api_client",
                    "send",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(error.code()),
                    request_fields,
                );
                return Err(error);
            }
        };
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
        let body_result = tokio::select! {
            biased;
            body = read_bounded_body(&mut response) => body,
            _ = cancellation.cancelled() => return Err(cancelled_error()),
        };
        let body = match body_result {
            Ok(body) => body,
            Err(error) => {
                unfour_diag::log_operation_event(
                    "api_request_failed",
                    "api_client",
                    "send",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(error.code()),
                    request_fields,
                );
                return Err(error);
            }
        };
        let duration_ms = started.elapsed().as_millis();
        // Once the complete HTTP response is available, cancellation and local
        // persistence cannot undo it or justify resending the remote operation.
        let history_result = self
            .insert_history(
                &executed,
                status.as_u16(),
                duration_ms,
                &response_headers,
                &body,
                secrets,
            )
            .await;
        let (history_id, warnings) = match history_result {
            Ok(history_id) => (history_id, Vec::new()),
            Err(error) => {
                unfour_diag::log_operation_event(
                    "api_history_record_failed",
                    "api_client",
                    "record_history",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(unfour_diag::app_error_kind(&error)),
                    serde_json::json!({
                        "request_id": request_id.as_str(),
                        "method": super::history_redaction::scrub_values(method.as_str(), secrets),
                        "host": super::history_redaction::scrub_values(url.host_str().unwrap_or(""), secrets),
                        "path": super::history_redaction::scrub_values(url.path(), secrets),
                        "status_code": status.as_u16(),
                    }),
                );
                (String::new(), vec!["API_HISTORY_RECORD_FAILED".to_string()])
            }
        };
        unfour_diag::log_operation_event(
            "api_request_completed",
            "api_client",
            "send",
            "ok",
            Some(duration_ms),
            None,
            serde_json::json!({
                "request_id": request_id.as_str(),
                "method": super::history_redaction::scrub_values(method.as_str(), secrets),
                "host": super::history_redaction::scrub_values(url.host_str().unwrap_or(""), secrets),
                "path": super::history_redaction::scrub_values(url.path(), secrets),
                "status_code": status.as_u16(),
            }),
        );

        Ok(ApiResponse {
            diagnostic_projection: Some(super::history_redaction::diagnostic_response(
                &response_headers,
                &body,
                secrets,
            )),
            warnings,
            request_snapshot: Some(request_snapshot),
            history_id,
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("").to_string(),
            headers: response_headers,
            body,
            duration_ms,
        })
    }

    async fn insert_history(
        &self,
        input: &ApiRequestInput,
        status: u16,
        duration_ms: u128,
        response_headers: &[KeyValue],
        response_body: &str,
        secrets: &[String],
    ) -> AppResult<String> {
        let now = Utc::now().to_rfc3339();
        let id = unfour_core::id::new_id();
        let safe = super::history_redaction::sanitize_with_secrets(
            input,
            response_headers,
            response_body,
            secrets,
        )?;

        sqlx::query(
            r#"
            INSERT INTO api_history (
              id, workspace_id, name, method, url, request_headers_json, request_query_json,
              request_body, status, duration_ms, response_headers_json, response_body_preview,
              created_at, updated_at, request_body_kind, redaction_version
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, ?14, 1)
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(safe.name)
        .bind(super::history_redaction::scrub_values(
            &input.method.to_uppercase(),
            secrets,
        ))
        .bind(safe.url)
        .bind(safe.headers)
        .bind(safe.query)
        .bind(safe.body)
        .bind(i64::from(status))
        .bind(i64::try_from(duration_ms).unwrap_or(i64::MAX))
        .bind(safe.response_headers)
        .bind(safe.response_body)
        .bind(now)
        .bind(&input.body_kind)
        .execute(self.db.pool())
        .await?;

        Ok(id)
    }
}

const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

fn retain_canonical_secret_values(
    input: &ApiRequestInput,
    url: &reqwest::Url,
    secrets: &mut Vec<String>,
) {
    // HTTP normalizes methods to uppercase, and host/header names to lowercase.
    // Keep those representations only where a known secret participates, so
    // snapshots, logs, history and echoed responses share the same provenance.
    let mut canonical = Vec::new();
    // Keep the complete source: a secret may itself be a URL with whitespace
    // that build_url accepts after trimming.
    let raw_url = input.url.as_str();
    let raw_host = raw_url_host_range(raw_url);
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        if input.method.contains(secret) {
            canonical.push(secret.to_uppercase());
        }
        let lowercase = secret.to_ascii_lowercase();
        if url.host_str().is_some_and(|host| host.contains(&lowercase))
            || input
                .headers
                .iter()
                .any(|header| header.enabled && header.key.contains(secret))
        {
            canonical.push(lowercase);
        }
        // IDNA (and numeric IP normalization) can transform the complete host
        // label, so a secret substring has no standalone canonical equivalent.
        // Retain the parsed host only when a secret overlaps the raw host span;
        // a secret solely in userinfo, port, path or query must not hide it.
        if let (Some(host), Some(span)) = (url.host_str(), raw_host.as_ref()) {
            if !host.contains(&secret.to_ascii_lowercase())
                && raw_url
                    .match_indices(secret.as_str())
                    .any(|(start, _)| start < span.end && start + secret.len() > span.start)
            {
                canonical.push(host.to_owned());
            }
        }
    }
    secrets.extend(canonical);
}

fn raw_url_host_range(raw_url: &str) -> Option<std::ops::Range<usize>> {
    let (_, after_scheme) = raw_url.split_once(':')?;
    // Match special-URL slash normalization without reparsing the host (which
    // would already lose its Unicode/IDNA source representation).
    let authority_suffix = after_scheme.trim_start_matches(['/', '\\']);
    let authority = authority_suffix.split(['/', '\\', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = if host_port.starts_with('[') {
        &host_port[..=host_port.find(']')?]
    } else {
        host_port.split(':').next()?
    };
    if host.is_empty() {
        return None;
    }
    let start = raw_url.len() - authority_suffix.len() + authority.len() - host_port.len();
    Some(start..start + host.len())
}

async fn read_bounded_body(response: &mut reqwest::Response) -> AppResult<String> {
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::Validation("API_RESPONSE_TOO_LARGE".into()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(classify_http_error)? {
        if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
            return Err(AppError::Validation("API_RESPONSE_TOO_LARGE".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&bytes)
        .trim_start_matches('\u{feff}')
        .to_owned())
}

fn request_timeout_duration(timeout_ms: Option<u64>) -> Option<Duration> {
    match timeout_ms.unwrap_or(0) {
        0 => None,
        timeout_ms => Some(Duration::from_millis(timeout_ms)),
    }
}

fn classify_http_error(error: reqwest::Error) -> AppError {
    let error = error.without_url();
    if error.is_timeout() {
        AppError::ApiTimeout(error.to_string())
    } else if error.is_connect() {
        AppError::ApiNetwork(error.to_string())
    } else {
        AppError::Http(error)
    }
}

fn cancelled_error() -> AppError {
    AppError::ApiCancelled("cancelled by user".to_string())
}

#[cfg(test)]
mod timeout_tests {
    use super::*;

    #[test]
    fn zero_and_missing_are_unlimited() {
        assert_eq!(request_timeout_duration(None), None);
        assert_eq!(request_timeout_duration(Some(0)), None);
    }

    #[test]
    fn positive_timeouts_are_exact_and_not_capped() {
        assert_eq!(
            request_timeout_duration(Some(1)),
            Some(Duration::from_millis(1))
        );
        assert_eq!(
            request_timeout_duration(Some(600_001)),
            Some(Duration::from_millis(600_001))
        );
    }
}
