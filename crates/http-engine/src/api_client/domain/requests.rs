use std::collections::HashSet;

use chrono::Utc;
use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityType, DomainMutation, MutationOperation,
};
use unfour_core::models::{
    ApiRequestInput, ApiRequestSettings, ApiSavedRequest, MAX_API_TIMEOUT_MS,
};
use unfour_core::{AppError, AppResult};

use super::super::helpers::{normalize_collection_id, normalize_entity_id};
use super::super::{DEFAULT_AUTH_JSON, DEFAULT_COLLECTION_NAME};
use super::collections::soft_delete_request_on;
use super::{
    collection_on, effective_parent, folder_on, list_requests_on, mutation, request_on,
    validate_workspace_on, ApiClientService,
};

struct ResolvedLocation {
    collection_id: String,
    parent_folder_id: Option<String>,
    mutations: Vec<DomainMutation>,
}

struct StoredRequestFields {
    name: String,
    location: ResolvedLocation,
    auth_json: String,
    method: String,
    headers_json: String,
    query_json: String,
    settings_json: String,
}

impl ApiClientService {
    pub async fn save_request_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        input: ApiRequestInput,
    ) -> AppResult<DomainCommandResult<ApiSavedRequest>> {
        validate_request_input(&input)?;
        validate_workspace_on(connection, &input.workspace_id).await?;
        let now = Utc::now().to_rfc3339();
        let fields = stored_request_fields_on(connection, context, &input, &now).await?;
        let id = unfour_core::id::new_id();
        sqlx::query(
            r#"
            INSERT INTO api_requests (
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              settings_json, pre_request_script, post_response_script, script_schema_version,
              created_at, updated_at, revision, sync_status
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
              ?14, ?15, ?16, ?17, ?18, ?18, 1, 'local'
            )
            "#,
        )
        .bind(&id)
        .bind(&input.workspace_id)
        .bind(fields.name)
        .bind(&fields.location.collection_id)
        .bind(&fields.location.parent_folder_id)
        .bind(
            next_request_sort_order_on(
                connection,
                &input.workspace_id,
                &fields.location.collection_id,
                fields.location.parent_folder_id.as_deref(),
            )
            .await?,
        )
        .bind(fields.auth_json)
        .bind(fields.method)
        .bind(input.url)
        .bind(fields.headers_json)
        .bind(fields.query_json)
        .bind(input.body)
        .bind(input.body_kind)
        .bind(fields.settings_json)
        .bind(input.pre_request_script)
        .bind(input.post_response_script)
        .bind(input.script_schema_version)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        let parent = effective_parent(
            &fields.location.collection_id,
            fields.location.parent_folder_id.as_deref(),
        );
        let mut mutations = fields.location.mutations;
        mutations.push(mutation(
            context,
            DomainEntityType::ApiRequest,
            MutationOperation::Upsert,
            &input.workspace_id,
            &id,
            Some(parent),
            1,
        ));
        Ok(DomainCommandResult::new(
            request_on(connection, &input.workspace_id, &id, false).await?,
            mutations,
        ))
    }

    pub async fn update_request_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        request_id: String,
        input: ApiRequestInput,
    ) -> AppResult<DomainCommandResult<ApiSavedRequest>> {
        validate_request_input(&input)?;
        if workspace_id != input.workspace_id {
            return Err(AppError::Validation(
                "api request workspace mismatch".to_string(),
            ));
        }
        let current = request_on(connection, &workspace_id, &request_id, false).await?;
        let now = Utc::now().to_rfc3339();
        let fields = stored_request_fields_on(connection, context, &input, &now).await?;
        if current.name == fields.name
            && current.collection_id == fields.location.collection_id
            && current.parent_folder_id == fields.location.parent_folder_id
            && current.auth_json == fields.auth_json
            && current.method == fields.method
            && current.url == input.url
            && current.headers_json == fields.headers_json
            && current.query_json == fields.query_json
            && current.body == input.body
            && current.body_kind == input.body_kind
            && current.settings_json == fields.settings_json
            && current.pre_request_script == input.pre_request_script
            && current.post_response_script == input.post_response_script
            && current.script_schema_version == input.script_schema_version
        {
            return Ok(DomainCommandResult::new(current, fields.location.mutations));
        }
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE api_requests
            SET name = ?1, collection_id = ?2, parent_folder_id = ?3,
                auth_json = ?4, method = ?5, url = ?6, headers_json = ?7,
                query_json = ?8, body = ?9, body_kind = ?10,
                settings_json = ?11, pre_request_script = ?12, post_response_script = ?13,
                script_schema_version = ?14, updated_at = ?15,
                revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?16 AND id = ?17 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(fields.name)
        .bind(&fields.location.collection_id)
        .bind(&fields.location.parent_folder_id)
        .bind(fields.auth_json)
        .bind(fields.method)
        .bind(input.url)
        .bind(fields.headers_json)
        .bind(fields.query_json)
        .bind(input.body)
        .bind(input.body_kind)
        .bind(fields.settings_json)
        .bind(input.pre_request_script)
        .bind(input.post_response_script)
        .bind(input.script_schema_version)
        .bind(now)
        .bind(&workspace_id)
        .bind(&request_id)
        .fetch_one(&mut *connection)
        .await?;
        let parent = effective_parent(
            &fields.location.collection_id,
            fields.location.parent_folder_id.as_deref(),
        );
        let mut mutations = fields.location.mutations;
        mutations.push(mutation(
            context,
            DomainEntityType::ApiRequest,
            MutationOperation::Upsert,
            &workspace_id,
            &request_id,
            Some(parent),
            revision,
        ));
        Ok(DomainCommandResult::new(
            request_on(connection, &workspace_id, &request_id, false).await?,
            mutations,
        ))
    }

    pub async fn duplicate_request_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<DomainCommandResult<ApiSavedRequest>> {
        let source = request_on(connection, &workspace_id, &request_id, false).await?;
        let id = unfour_core::id::new_id();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO api_requests (
              id, workspace_id, name, collection_id, parent_folder_id, sort_order,
              auth_json, method, url, headers_json, query_json, body, body_kind,
              settings_json, pre_request_script, post_response_script, script_schema_version,
              created_at, updated_at, revision, sync_status
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
              ?14, ?15, ?16, ?17, ?18, ?18, 1, 'local'
            )
            "#,
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(duplicate_request_name(&source.name))
        .bind(&source.collection_id)
        .bind(&source.parent_folder_id)
        .bind(source.sort_order)
        .bind(source.auth_json)
        .bind(source.method)
        .bind(source.url)
        .bind(source.headers_json)
        .bind(source.query_json)
        .bind(source.body)
        .bind(source.body_kind)
        .bind(source.settings_json)
        .bind(source.pre_request_script)
        .bind(source.post_response_script)
        .bind(source.script_schema_version)
        .bind(now)
        .execute(&mut *connection)
        .await?;
        let parent = effective_parent(&source.collection_id, source.parent_folder_id.as_deref());
        Ok(DomainCommandResult::new(
            request_on(connection, &workspace_id, &id, false).await?,
            vec![mutation(
                context,
                DomainEntityType::ApiRequest,
                MutationOperation::Upsert,
                &workspace_id,
                &id,
                Some(parent),
                1,
            )],
        ))
    }

    pub async fn delete_request_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        request_id: String,
    ) -> AppResult<DomainCommandResult<Vec<ApiSavedRequest>>> {
        let current = request_on(connection, &workspace_id, &request_id, false).await?;
        let revision = soft_delete_request_on(
            connection,
            &workspace_id,
            &request_id,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        let parent = effective_parent(&current.collection_id, current.parent_folder_id.as_deref());
        Ok(DomainCommandResult::new(
            list_requests_on(connection, &workspace_id).await?,
            vec![mutation(
                context,
                DomainEntityType::ApiRequest,
                MutationOperation::Delete,
                &workspace_id,
                &request_id,
                Some(parent),
                revision,
            )],
        ))
    }

    pub async fn move_request_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        request_id: String,
        collection_id: Option<String>,
        parent_folder_id: Option<String>,
    ) -> AppResult<DomainCommandResult<ApiSavedRequest>> {
        let current = request_on(connection, &workspace_id, &request_id, false).await?;
        let now = Utc::now().to_rfc3339();
        let location = resolve_location_on(
            connection,
            context,
            &workspace_id,
            collection_id,
            parent_folder_id,
            &now,
        )
        .await?;
        if current.collection_id == location.collection_id
            && current.parent_folder_id == location.parent_folder_id
        {
            return Ok(DomainCommandResult::new(current, location.mutations));
        }
        let sort_order = next_request_sort_order_on(
            connection,
            &workspace_id,
            &location.collection_id,
            location.parent_folder_id.as_deref(),
        )
        .await?;
        let revision: i64 = sqlx::query_scalar(
            r#"
            UPDATE api_requests
            SET collection_id = ?1, parent_folder_id = ?2, sort_order = ?3,
                updated_at = ?4, revision = revision + 1, sync_status = 'pending'
            WHERE workspace_id = ?5 AND id = ?6 AND deleted_at IS NULL
            RETURNING revision
            "#,
        )
        .bind(&location.collection_id)
        .bind(&location.parent_folder_id)
        .bind(sort_order)
        .bind(now)
        .bind(&workspace_id)
        .bind(&request_id)
        .fetch_one(&mut *connection)
        .await?;
        let parent = effective_parent(
            &location.collection_id,
            location.parent_folder_id.as_deref(),
        );
        let mut mutations = location.mutations;
        mutations.push(mutation(
            context,
            DomainEntityType::ApiRequest,
            MutationOperation::Upsert,
            &workspace_id,
            &request_id,
            Some(parent),
            revision,
        ));
        Ok(DomainCommandResult::new(
            request_on(connection, &workspace_id, &request_id, false).await?,
            mutations,
        ))
    }

    pub async fn reorder_requests_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        request_ids: Vec<String>,
    ) -> AppResult<DomainCommandResult<Vec<ApiSavedRequest>>> {
        let parent_folder_id = normalize_entity_id(parent_folder_id);
        collection_on(connection, &workspace_id, &collection_id, false).await?;
        if let Some(parent_id) = parent_folder_id.as_deref() {
            let parent = folder_on(connection, &workspace_id, parent_id, false).await?;
            if parent.collection_id != collection_id {
                return Err(AppError::Validation(
                    "request reorder parent must belong to the target collection".to_string(),
                ));
            }
        }
        let current = sibling_requests_on(
            connection,
            &workspace_id,
            &collection_id,
            parent_folder_id.as_deref(),
        )
        .await?;
        validate_request_reorder(
            &request_ids,
            current.iter().map(|request| request.id.as_str()),
        )?;
        let now = Utc::now().to_rfc3339();
        let mut mutations = Vec::new();
        for (index, request_id) in request_ids.iter().enumerate() {
            let sort_order = i64::try_from(index).unwrap_or(i64::MAX);
            let request = current
                .iter()
                .find(|request| request.id == *request_id)
                .expect("validated request reorder id");
            if request.sort_order == sort_order {
                continue;
            }
            let revision: i64 = sqlx::query_scalar(
                r#"
                UPDATE api_requests
                SET sort_order = ?1, updated_at = ?2, revision = revision + 1,
                    sync_status = 'pending'
                WHERE workspace_id = ?3 AND id = ?4 AND deleted_at IS NULL
                RETURNING revision
                "#,
            )
            .bind(sort_order)
            .bind(&now)
            .bind(&workspace_id)
            .bind(request_id)
            .fetch_one(&mut *connection)
            .await?;
            mutations.push(mutation(
                context,
                DomainEntityType::ApiRequest,
                MutationOperation::Upsert,
                &workspace_id,
                request_id,
                Some(effective_parent(
                    &collection_id,
                    parent_folder_id.as_deref(),
                )),
                revision,
            ));
        }
        Ok(DomainCommandResult::new(
            list_requests_on(connection, &workspace_id).await?,
            mutations,
        ))
    }
}

async fn stored_request_fields_on(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    input: &ApiRequestInput,
    now: &str,
) -> AppResult<StoredRequestFields> {
    let location = resolve_location_on(
        connection,
        context,
        &input.workspace_id,
        input.collection_id.clone(),
        input.parent_folder_id.clone(),
        now,
    )
    .await?;
    let name = stored_request_name(input.name.as_deref(), &input.method, &input.url)?;
    Ok(StoredRequestFields {
        name,
        location,
        auth_json: input
            .auth_json
            .clone()
            .unwrap_or_else(|| DEFAULT_AUTH_JSON.to_string()),
        method: input.method.to_uppercase(),
        headers_json: serde_json::to_string(&input.headers)?,
        query_json: serde_json::to_string(&input.query)?,
        settings_json: serde_json::to_string(&ApiRequestSettings {
            timeout_ms: input.timeout_ms,
        })?,
    })
}

/// Duplicating a request appends " Copy"; truncate the source name so the
/// result stays within the 120-character cap instead of producing a name the
/// sync server would reject.
fn duplicate_request_name(source: &str) -> String {
    const SUFFIX: &str = " Copy";
    let budget = 120 - SUFFIX.chars().count();
    let base: String = source.trim().chars().take(budget).collect();
    format!("{}{}", base.trim_end(), SUFFIX)
}

/// Explicit request names are validated strictly (the sync server enforces
/// the same 120-rune cap at push time); the derived `METHOD url` fallback is
/// truncated instead so saving a request without a name can never fail.
fn stored_request_name(name: Option<&str>, method: &str, url: &str) -> AppResult<String> {
    if let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) {
        if name.chars().count() > 120 {
            return Err(AppError::Validation(
                "request name must be 120 characters or fewer".to_string(),
            ));
        }
        return Ok(name.to_string());
    }
    Ok(format!("{} {}", method.to_uppercase(), url)
        .chars()
        .take(120)
        .collect())
}

async fn resolve_location_on(
    connection: &mut SqliteConnection,
    context: &CommandContext,
    workspace_id: &str,
    collection_id: Option<String>,
    parent_folder_id: Option<String>,
    now: &str,
) -> AppResult<ResolvedLocation> {
    let parent_folder_id = normalize_entity_id(parent_folder_id);
    let collection_id = normalize_collection_id(collection_id);
    if let Some(parent_id) = parent_folder_id.as_deref() {
        let parent = folder_on(connection, workspace_id, parent_id, false).await?;
        if collection_id
            .as_deref()
            .is_some_and(|collection_id| collection_id != parent.collection_id)
        {
            return Err(AppError::Validation(
                "parent folder must belong to the target collection".to_string(),
            ));
        }
        return Ok(ResolvedLocation {
            collection_id: parent.collection_id,
            parent_folder_id,
            mutations: Vec::new(),
        });
    }
    if let Some(collection_id) = collection_id {
        collection_on(connection, workspace_id, &collection_id, false).await?;
        return Ok(ResolvedLocation {
            collection_id,
            parent_folder_id: None,
            mutations: Vec::new(),
        });
    }
    if let Some(collection_id) = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id FROM api_collections
        WHERE workspace_id = ?1 AND deleted_at IS NULL
        ORDER BY name COLLATE NOCASE LIMIT 1
        "#,
    )
    .bind(workspace_id)
    .fetch_optional(&mut *connection)
    .await?
    {
        return Ok(ResolvedLocation {
            collection_id,
            parent_folder_id: None,
            mutations: Vec::new(),
        });
    }
    let collection_id = unfour_core::id::new_id();
    sqlx::query(
        r#"
        INSERT INTO api_collections (
          id, workspace_id, name, created_at, updated_at, revision, sync_status
        ) VALUES (?1, ?2, ?3, ?4, ?4, 1, 'local')
        "#,
    )
    .bind(&collection_id)
    .bind(workspace_id)
    .bind(DEFAULT_COLLECTION_NAME)
    .bind(now)
    .execute(&mut *connection)
    .await?;
    Ok(ResolvedLocation {
        collection_id: collection_id.clone(),
        parent_folder_id: None,
        mutations: vec![mutation(
            context,
            DomainEntityType::ApiCollection,
            MutationOperation::Upsert,
            workspace_id,
            &collection_id,
            None,
            1,
        )],
    })
}

async fn next_request_sort_order_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<&str>,
) -> AppResult<i64> {
    let value: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT MAX(sort_order) FROM api_requests
        WHERE workspace_id = ?1 AND collection_id = ?2
          AND parent_folder_id IS ?3 AND deleted_at IS NULL
        "#,
    )
    .bind(workspace_id)
    .bind(collection_id)
    .bind(parent_folder_id)
    .fetch_one(&mut *connection)
    .await?;
    Ok(value.unwrap_or(-1) + 1)
}

async fn sibling_requests_on(
    connection: &mut SqliteConnection,
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<&str>,
) -> AppResult<Vec<ApiSavedRequest>> {
    Ok(sqlx::query_as::<_, ApiSavedRequest>(
        r#"
        SELECT id, workspace_id, name, collection_id, parent_folder_id,
               sort_order, auth_json, method, url, headers_json, query_json,
               body, body_kind, settings_json, pre_request_script, post_response_script,
               script_schema_version, created_at, updated_at, deleted_at,
               revision, sync_status, remote_id
        FROM api_requests
        WHERE workspace_id = ?1 AND collection_id = ?2
          AND parent_folder_id IS ?3 AND deleted_at IS NULL
        ORDER BY sort_order, id
        "#,
    )
    .bind(workspace_id)
    .bind(collection_id)
    .bind(parent_folder_id)
    .fetch_all(&mut *connection)
    .await?)
}

fn validate_request_input(input: &ApiRequestInput) -> AppResult<()> {
    super::super::helpers::validate_workspace_id(&input.workspace_id)?;
    if input
        .timeout_ms
        .is_some_and(|value| value > MAX_API_TIMEOUT_MS)
    {
        return Err(AppError::Validation(format!(
            "request timeout must be between 0 and {MAX_API_TIMEOUT_MS} milliseconds"
        )));
    }
    crate::script_runtime::validate_script_config(
        input.pre_request_script.as_deref(),
        input.post_response_script.as_deref(),
        input.script_schema_version,
    )
}

fn validate_request_reorder<'a>(
    desired: &[String],
    current: impl Iterator<Item = &'a str>,
) -> AppResult<()> {
    let desired_set = desired.iter().map(String::as_str).collect::<HashSet<_>>();
    let current_set = current.collect::<HashSet<_>>();
    if desired.len() != desired_set.len() || desired_set != current_set {
        return Err(AppError::Validation(
            "request reorder must contain every sibling exactly once".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_reorder_rejects_duplicate_ids() {
        let desired = vec!["request-a".to_string(), "request-a".to_string()];
        let error = validate_request_reorder(&desired, ["request-a"].into_iter()).unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn explicit_request_names_are_capped_and_derived_names_truncate() {
        let error =
            stored_request_name(Some(&"n".repeat(121)), "get", "https://api.test").unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
        assert_eq!(
            stored_request_name(Some(&"n".repeat(120)), "get", "https://api.test")
                .unwrap()
                .chars()
                .count(),
            120
        );
        assert_eq!(
            stored_request_name(Some("  Create user  "), "post", "https://api.test").unwrap(),
            "Create user"
        );

        let long_url = format!("https://api.test/{}", "p".repeat(300));
        let derived = stored_request_name(None, "get", &long_url).unwrap();
        assert_eq!(derived.chars().count(), 120);
        assert!(derived.starts_with("GET https://api.test/"));
    }

    #[test]
    fn duplicate_names_stay_within_the_cap() {
        assert_eq!(
            duplicate_request_name("List accounts"),
            "List accounts Copy"
        );
        let capped = duplicate_request_name(&"n".repeat(120));
        assert_eq!(capped.chars().count(), 120);
        assert!(capped.ends_with(" Copy"));
    }
}
