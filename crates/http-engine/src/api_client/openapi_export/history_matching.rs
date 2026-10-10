use super::super::history_redaction;
use unfour_core::models::{ApiHistoryDetail, ApiRequestInput, ApiSavedRequest};
use unfour_core::AppResult;

pub(super) fn matching_histories(
    client: &reqwest::Client,
    request: &ApiSavedRequest,
    histories: &[ApiHistoryDetail],
) -> AppResult<Vec<ApiHistoryDetail>> {
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
    let input = super::super::auth::materialize_auth(client, input)?;
    let safe = history_redaction::sanitize(&input, &[], "")?;
    let mut matches = Vec::new();
    for history in histories {
        if history.workspace_id != request.workspace_id
            || !history.method.eq_ignore_ascii_case(&request.method)
        {
            continue;
        }
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
            matches.push(history);
        }
    }
    Ok(matches)
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
