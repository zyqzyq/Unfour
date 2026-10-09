use super::*;
use unfour_core::redaction::{
    is_sensitive_key, redact_json_body, redact_sensitive_lines, REDACTED_VALUE,
};

pub(super) struct SafeHistory {
    pub name: Option<String>,
    pub url: String,
    pub headers: String,
    pub query: String,
    pub body: Option<String>,
    pub response_headers: String,
    pub response_body: String,
}

pub(super) fn sanitize(
    input: &ApiRequestInput,
    response_headers: &[KeyValue],
    response_body: &str,
) -> AppResult<SafeHistory> {
    sanitize_with_secrets(input, response_headers, response_body, &[])
}

pub(super) fn sanitize_with_secrets(
    input: &ApiRequestInput,
    response_headers: &[KeyValue],
    response_body: &str,
    variable_secrets: &[String],
) -> AppResult<SafeHistory> {
    let secrets = scrub_secrets(input, variable_secrets)?;
    let scrub = |value: &str| scrub_values(value, &secrets);
    Ok(SafeHistory {
        name: input.name.as_deref().map(&scrub),
        url: scrub(&super::domain::export_url(&input.url)),
        headers: redact_rows(&input.headers, &scrub)?,
        query: redact_rows(&input.query, &scrub)?,
        body: redact_request_body(input, &scrub),
        response_headers: redact_rows(response_headers, &scrub)?,
        // Redact the complete JSON first: truncation can cut a credential or
        // make otherwise valid JSON impossible to sanitize structurally.
        response_body: scrub(&safe_body(response_body))
            .chars()
            .take(20_000)
            .collect(),
    })
}

fn scrub_secrets(input: &ApiRequestInput, variable_secrets: &[String]) -> AppResult<Vec<String>> {
    let mut secrets = runtime_request_secret_values(input)?;
    secrets.extend(variable_secrets.iter().cloned());
    secrets.retain(|value| !value.is_empty() && value != REDACTED_VALUE);
    Ok(secrets)
}

pub(super) fn scrub_values(value: &str, secrets: &[String]) -> String {
    let mut variants = Vec::new();
    for secret in secrets
        .iter()
        .filter(|secret| !secret.is_empty() && secret.as_str() != REDACTED_VALUE)
    {
        let escaped = serde_json::to_string(secret).unwrap_or_default();
        let encoded = reqwest::Url::parse_with_params("http://localhost/", &[("v", secret)])
            .ok()
            .and_then(|url| url.query().map(|q| q.trim_start_matches("v=").to_owned()))
            .unwrap_or_default();
        variants.extend([
            secret.clone(),
            escaped.trim_matches('"').to_owned(),
            encoded.clone(),
            encoded.replace('+', "%20"),
        ]);
    }
    variants.retain(|value| !value.is_empty());
    variants.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    variants.dedup();
    let mut value = value.to_owned();
    for secret in variants {
        // Preserve markers inserted by earlier replacements, including when a
        // one-character secret appears in the marker itself.
        value = value
            .split(REDACTED_VALUE)
            .map(|part| part.replace(&secret, REDACTED_VALUE))
            .collect::<Vec<_>>()
            .join(REDACTED_VALUE);
    }
    value
}

fn redact_rows(items: &[KeyValue], scrub: &impl Fn(&str) -> String) -> AppResult<String> {
    let safe = items
        .iter()
        .map(|item| KeyValue {
            key: item.key.clone(),
            enabled: item.enabled,
            value: if is_sensitive_key(&item.key)
                || item.key.trim().eq_ignore_ascii_case("set-cookie")
            {
                REDACTED_VALUE.into()
            } else {
                scrub(&item.value)
            },
        })
        .collect::<Vec<_>>();
    Ok(serde_json::to_string(&safe)?)
}

fn redact_request_body(input: &ApiRequestInput, scrub: &impl Fn(&str) -> String) -> Option<String> {
    let exported = super::domain::export_body(input.body.as_deref(), &input.body_kind)?;
    let original = input.body.as_deref().unwrap_or("");
    let source = if json_values_equal(original, &exported) {
        original
    } else {
        exported.as_str()
    };
    // Form and JSON exports are already structural. Line redaction would erase
    // an entire `password=...&note=keep` body because the line contains "password".
    let structured = input.body_kind.eq_ignore_ascii_case("form-urlencoded")
        || serde_json::from_str::<serde_json::Value>(source).is_ok();
    let safe = if structured {
        source.to_owned()
    } else {
        safe_body(source)
    };
    Some(scrub(&safe))
}

fn json_values_equal(left: &str, right: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(left)
        .ok()
        .zip(serde_json::from_str::<serde_json::Value>(right).ok())
        .is_some_and(|(before, after)| before == after)
}

fn safe_body(body: &str) -> String {
    if body.trim_start().starts_with(['{', '['])
        && serde_json::from_str::<serde_json::Value>(body).is_err()
    {
        // Old history previews may be truncated in the middle of a secret.
        return REDACTED_VALUE.into();
    }
    if serde_json::from_str::<serde_json::Value>(body).is_ok() {
        redact_json_body(body).0
    } else {
        redact_sensitive_lines(body).0
    }
}

impl ApiClientService {
    /// Repair old local history in bounded, atomic batches. Does not change
    /// request definitions or runtime payloads, and never opens the keychain.
    /// A read-only attachment cannot rewrite rows; callers still sanitize on read.
    pub async fn redact_legacy_history(&self) -> AppResult<()> {
        match self.repair_legacy_history().await {
            Err(error) if database_is_readonly(&error) => Ok(()),
            result => result,
        }
    }

    async fn repair_legacy_history(&self) -> AppResult<()> {
        loop {
            let mut tx = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
            let rows = sqlx::query_as::<_, ApiHistoryDetail>(
                "SELECT * FROM api_history WHERE redaction_version = 0 LIMIT 100",
            )
            .fetch_all(&mut *tx)
            .await?;
            if rows.is_empty() {
                // Transaction::drop queues rollback on SQLx's worker. Await it
                // before startup can seed through another pooled connection.
                tx.rollback().await?;
                return Ok(());
            }
            for row in rows {
                let safe = safe_detail(&row)?;
                sqlx::query(
                    "UPDATE api_history SET name=?, url=?, request_headers_json=?, request_query_json=?, request_body=?, response_headers_json=?, response_body_preview=?, redaction_version=1 WHERE workspace_id=? AND id=? AND redaction_version=0",
                )
                .bind(safe.name)
                .bind(safe.url)
                .bind(safe.headers)
                .bind(safe.query)
                .bind(safe.body)
                .bind(safe.response_headers)
                .bind(safe.response_body)
                .bind(row.workspace_id)
                .bind(row.id)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
        }
    }
}

fn database_is_readonly(error: &AppError) -> bool {
    let AppError::Database(sqlx::Error::Database(database_error)) = error else {
        return false;
    };
    let message = database_error.message().to_ascii_lowercase();
    message.contains("readonly") || message.contains("read-only")
}

fn safe_detail(row: &ApiHistoryDetail) -> AppResult<SafeHistory> {
    let input = ApiRequestInput {
        workspace_id: row.workspace_id.clone(),
        name: row.name.clone(),
        method: row.method.clone(),
        url: row.url.clone(),
        headers: serde_json::from_str(&row.request_headers_json).unwrap_or_default(),
        query: serde_json::from_str(&row.request_query_json).unwrap_or_default(),
        body: row.request_body.clone(),
        body_kind: row.request_body_kind.clone(),
        auth_json: None,
        collection_id: None,
        parent_folder_id: None,
        timeout_ms: None,
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        multipart_parts: vec![],
        temporary_variables: vec![],
    };
    sanitize(
        &input,
        &serde_json::from_str::<Vec<KeyValue>>(&row.response_headers_json).unwrap_or_default(),
        row.response_body_preview.as_deref().unwrap_or(""),
    )
}

pub(super) fn sanitize_detail(row: &mut ApiHistoryDetail) -> AppResult<()> {
    let safe = safe_detail(row)?;
    row.name = safe.name;
    row.url = safe.url;
    row.request_headers_json = safe.headers;
    row.request_query_json = safe.query;
    row.request_body = safe.body;
    row.response_headers_json = safe.response_headers;
    row.response_body_preview = row
        .response_body_preview
        .as_ref()
        .map(|_| safe.response_body);
    Ok(())
}

#[cfg(test)]
mod storage_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_escaped_and_encoded_secrets_are_scrubbed_without_corrupting_markers() {
        for secret in ["a", "red", "x y", "quote\"slash\\"] {
            let escaped = serde_json::to_string(secret).unwrap();
            let encoded = reqwest::Url::parse_with_params("http://localhost/", &[("v", secret)])
                .unwrap()
                .query()
                .unwrap()
                .trim_start_matches("v=")
                .to_owned();
            let input = format!(
                "{secret}|{}|{encoded}|<redacted>",
                escaped.trim_matches('"')
            );
            let safe = scrub_values(&input, &[secret.into()]);
            assert_eq!(safe, "<redacted>|<redacted>|<redacted>|<redacted>");
        }
    }
}
