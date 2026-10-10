use super::super::history_redaction;
use unfour_core::models::{ApiHistoryDetail, ApiRequestInput, ApiSavedRequest, KeyValue};
use unfour_core::redaction::REDACTED_VALUE;
use unfour_core::AppResult;

pub(super) fn matching_histories(
    client: &reqwest::Client,
    request: &ApiSavedRequest,
    histories: &[ApiHistoryDetail],
) -> AppResult<Vec<ApiHistoryDetail>> {
    // Imported/legacy Auth may be malformed. Never match a pre-auth fallback,
    // including shapes that Send treats as an implicit "none" or default header.
    let Ok(auth) = serde_json::from_str::<serde_json::Value>(&request.auth_json) else {
        return Ok(Vec::new());
    };
    if !auth.is_object()
        || auth["type"].as_str().is_none()
        || (auth["type"] == "api-key" && auth.get("addTo").is_some_and(|value| !value.is_string()))
    {
        return Ok(Vec::new());
    }
    // History has no saved-request ID (the execution request_id is diagnostic
    // only). Compare the same redacted projection on both sides, including old
    // plaintext rows, without changing persisted contracts or backup formats.
    let input = ApiRequestInput {
        workspace_id: request.workspace_id.clone(),
        name: Some(request.name.clone()),
        method: request.method.clone(),
        url: request.url.clone(),
        headers: serde_json::from_str(&request.headers_json)?,
        query: serde_json::from_str(&request.query_json)?,
        body: request.body.clone(),
        body_kind: request.body_kind.clone(),
        auth_json: Some(request.auth_json.clone()),
        collection_id: None,
        parent_folder_id: None,
        timeout_ms: None,
        pre_request_script: None,
        post_response_script: None,
        script_schema_version: 1,
        multipart_parts: vec![],
        temporary_variables: vec![],
    };
    // Saved definitions do not contain generated auth rows. Use exactly the
    // execution rules (including explicit enabled overrides) before redaction.
    let Ok(input) = super::super::auth::materialize_auth(client, input) else {
        // Auth cannot be reliably executed, but the sanitized definition can
        // still be exported with the default response. Other requests continue.
        return Ok(Vec::new());
    };
    let safe = history_redaction::sanitize(&input, &[], "")?;
    let mut matches = Vec::new();
    for history in histories {
        if history.workspace_id != request.workspace_id
            || !history.method.eq_ignore_ascii_case(&request.method)
        {
            continue;
        }
        let omit_examples = response_secrets_unavailable(history, &auth);
        let mut history = history.clone();
        // Old plaintext custom API-key rows have no auth metadata of their own.
        // The definition supplies the target slot for redaction, never extra
        // rows: histories missing materialized auth must remain non-matches.
        history_redaction::sanitize_detail_with_auth(&mut history, input.auth_json.as_deref())?;
        if history.name == safe.name
            && normalize_redaction_markers(&history.url) == normalize_redaction_markers(&safe.url)
            && json_text_equal(&history.request_headers_json, &safe.headers)
            && json_text_equal(&history.request_query_json, &safe.query)
            && json_text_equal(
                history.request_body.as_deref().unwrap_or(""),
                safe.body.as_deref().unwrap_or(""),
            )
        {
            if omit_examples {
                // A marker has lost its original value; current credentials are
                // not evidence of a historical echo's value after rotation.
                // Keep association/status, but no response payload can be trusted.
                history.response_headers_json = "[]".into();
                history.response_body_preview = None;
            }
            matches.push(history);
        }
    }
    Ok(matches)
}

fn response_secrets_unavailable(history: &ApiHistoryDetail, auth: &serde_json::Value) -> bool {
    if [
        history.name.as_deref(),
        Some(history.url.as_str()),
        Some(history.request_headers_json.as_str()),
        Some(history.request_query_json.as_str()),
        history.request_body.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        // Imported JSON can spell the same marker with escaped brackets.
        let canonical = serde_json::from_str::<serde_json::Value>(value)
            .map(|value| value.to_string())
            .unwrap_or_else(|_| value.to_owned());
        normalize_redaction_markers(&canonical).contains(REDACTED_VALUE)
    }) {
        return true;
    }
    // History has no Auth metadata or runtime secret provenance. In particular,
    // an old Basic header exposes only the encoded pair, not the password that
    // could have been echoed separately. Do not guess it from today's Auth.
    let Ok(headers) = serde_json::from_str::<Vec<KeyValue>>(&history.request_headers_json) else {
        return true;
    };
    auth["type"] == "basic"
        || serde_json::from_str::<Vec<KeyValue>>(&history.request_query_json).is_err()
        || headers.iter().any(|header| {
            header.enabled
                && matches!(
                    header.key.trim().to_ascii_lowercase().as_str(),
                    "authorization" | "proxy-authorization"
                )
                && header
                    .value
                    .split_whitespace()
                    .next()
                    .is_some_and(|scheme| scheme.eq_ignore_ascii_case("basic"))
        })
}

fn normalize_redaction_markers(value: &str) -> String {
    // The shared URL exporter percent-encodes markers, while provenance
    // scrubbing can insert the literal marker. Neither is a wildcard.
    value
        .replace("%3Credacted%3E", "<redacted>")
        .replace("%3credacted%3e", "<redacted>")
        .replace("%3Credacted%3e", "<redacted>")
        .replace("%3credacted%3E", "<redacted>")
}

fn json_text_equal(left: &str, right: &str) -> bool {
    let left = normalize_redaction_markers(left);
    let right = normalize_redaction_markers(right);
    match (
        serde_json::from_str::<serde_json::Value>(&left),
        serde_json::from_str::<serde_json::Value>(&right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}
