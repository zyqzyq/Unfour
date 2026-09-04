use sqlx::SqliteConnection;
use unfour_core::domain::ExternalApiRequestUpsert;
use unfour_core::models::ApiSavedRequest;
use unfour_core::{AppError, AppResult};

use super::helpers::{doomed_orphan_to_skip, validate_external_record, validate_owner};
use crate::api_client::domain::secrets::{
    restore_auth_json, restore_body, restore_key_values, restore_url,
};
use crate::api_client::domain::{collection_on, folder_on};
use crate::api_client::helpers::normalize_entity_id;
use unfour_core::models::{ApiRequestSettings, MAX_API_TIMEOUT_MS};

pub(super) async fn upsert_request(
    connection: &mut SqliteConnection,
    mut record: ExternalApiRequestUpsert,
) -> AppResult<Option<i64>> {
    validate_external_record(
        &record.id,
        &record.workspace_id,
        &record.created_at,
        &record.updated_at,
    )?;
    validate_request_settings_json(&record.settings_json)?;
    validate_owner(connection, "api_requests", &record.id, &record.workspace_id).await?;
    if doomed_orphan_to_skip(
        collection_on(
            connection,
            &record.workspace_id,
            &record.collection_id,
            false,
        )
        .await,
    )?
    .is_none()
    {
        return Ok(None);
    }
    record.parent_folder_id = normalize_entity_id(record.parent_folder_id);
    if let Some(parent_id) = record.parent_folder_id.as_deref() {
        let Some(parent) = doomed_orphan_to_skip(
            folder_on(connection, &record.workspace_id, parent_id, false).await,
        )?
        else {
            return Ok(None);
        };
        if parent.collection_id != record.collection_id {
            return Err(AppError::Validation(
                "external API request parent must belong to its collection".to_string(),
            ));
        }
    }
    crate::script_runtime::validate_script_config(
        record.pre_request_script.as_deref(),
        record.post_response_script.as_deref(),
        record.script_schema_version,
    )?;
    let name = record.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation(
            "external API request name cannot be empty".to_string(),
        ));
    }
    let current = sqlx::query_as::<_, ApiSavedRequest>(
        r#"
        SELECT id, workspace_id, name, collection_id, parent_folder_id,
               sort_order, auth_json, method, url, headers_json, query_json,
               body, body_kind, settings_json, pre_request_script, post_response_script,
               script_schema_version, created_at, updated_at, deleted_at,
               revision, sync_status, remote_id
        FROM api_requests WHERE id = ?1
        "#,
    )
    .bind(&record.id)
    .fetch_optional(&mut *connection)
    .await?;
    let auth_json = restore_auth_json(
        &record.auth_json,
        current.as_ref().map(|request| request.auth_json.as_str()),
    );
    let headers = restore_key_values(
        record.headers,
        current
            .as_ref()
            .map(|request| request.headers_json.as_str()),
    );
    let query = restore_key_values(
        record.query,
        current.as_ref().map(|request| request.query_json.as_str()),
    );
    let body = restore_body(
        record.body,
        current.as_ref().and_then(|request| request.body.as_deref()),
        &record.body_kind,
    );
    let url = restore_url(
        &record.url,
        current.as_ref().map(|request| request.url.as_str()),
    );
    let headers_json = serde_json::to_string(&headers)?;
    let query_json = serde_json::to_string(&query)?;
    let method = record.method.to_uppercase();
    if let Some(current) = current {
        if current.deleted_at.is_none()
            && current.collection_id == record.collection_id
            && current.parent_folder_id == record.parent_folder_id
            && current.name == name
            && current.sort_order == record.sort_order
            && current.auth_json == auth_json
            && current.method == method
            && current.url == url
            && current.headers_json == headers_json
            && current.query_json == query_json
            && current.body == body
            && current.body_kind == record.body_kind
            && current.settings_json == record.settings_json
            && current.pre_request_script == record.pre_request_script
            && current.post_response_script == record.post_response_script
            && current.script_schema_version == record.script_schema_version
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok(None);
        }
        return Ok(Some(
            sqlx::query_scalar(
                r#"
            UPDATE api_requests
            SET collection_id = ?1, parent_folder_id = ?2, name = ?3,
                sort_order = ?4, auth_json = ?5, method = ?6, url = ?7,
                headers_json = ?8, query_json = ?9, body = ?10, body_kind = ?11,
                settings_json = ?12, pre_request_script = ?13, post_response_script = ?14,
                script_schema_version = ?15, created_at = ?16, updated_at = ?17,
                deleted_at = NULL, revision = revision + 1, sync_status = 'local'
            WHERE id = ?18 AND workspace_id = ?19 RETURNING revision
            "#,
            )
            .bind(record.collection_id)
            .bind(record.parent_folder_id)
            .bind(name)
            .bind(record.sort_order)
            .bind(auth_json)
            .bind(method)
            .bind(url)
            .bind(headers_json)
            .bind(query_json)
            .bind(body)
            .bind(record.body_kind)
            .bind(record.settings_json)
            .bind(record.pre_request_script)
            .bind(record.post_response_script)
            .bind(record.script_schema_version)
            .bind(record.created_at)
            .bind(record.updated_at)
            .bind(record.id)
            .bind(record.workspace_id)
            .fetch_one(&mut *connection)
            .await?,
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO api_requests (
          id, workspace_id, name, collection_id, parent_folder_id, sort_order,
          auth_json, method, url, headers_json, query_json, body, body_kind,
          settings_json, pre_request_script, post_response_script, script_schema_version,
          created_at, updated_at, revision, sync_status
        ) VALUES (
          ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
          ?14, ?15, ?16, ?17, ?18, ?19, 1, 'local'
        )
        "#,
    )
    .bind(record.id)
    .bind(record.workspace_id)
    .bind(name)
    .bind(record.collection_id)
    .bind(record.parent_folder_id)
    .bind(record.sort_order)
    .bind(auth_json)
    .bind(method)
    .bind(url)
    .bind(headers_json)
    .bind(query_json)
    .bind(body)
    .bind(record.body_kind)
    .bind(record.settings_json)
    .bind(record.pre_request_script)
    .bind(record.post_response_script)
    .bind(record.script_schema_version)
    .bind(record.created_at)
    .bind(record.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok(Some(1))
}

fn validate_request_settings_json(settings_json: &str) -> AppResult<()> {
    let settings = serde_json::from_str::<ApiRequestSettings>(settings_json).map_err(|_| {
        AppError::Validation("external API request settings are invalid".to_string())
    })?;
    if settings
        .timeout_ms
        .is_some_and(|timeout_ms| timeout_ms > MAX_API_TIMEOUT_MS)
    {
        return Err(AppError::Validation(format!(
            "external API request timeout must be between 0 and {MAX_API_TIMEOUT_MS} milliseconds"
        )));
    }
    Ok(())
}
