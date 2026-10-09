use super::*;
use sqlx::SqliteConnection;

pub fn is_bundle_variable_template(value: &str) -> bool {
    let value = value.strip_prefix("Bearer ").unwrap_or(value);
    value
        .strip_prefix("{{")
        .and_then(|v| v.strip_suffix("}}"))
        .is_some_and(|key| {
            !key.trim().is_empty()
                && key
                    .trim()
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        })
}

impl ApiClientService {
    pub async fn bundle_requests_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<ApiSavedRequest>> {
        Ok(
            sqlx::query_as(
                "SELECT * FROM api_requests WHERE workspace_id=? AND deleted_at IS NULL",
            )
            .bind(workspace)
            .fetch_all(&mut *db)
            .await?,
        )
    }
    /// Only a generated keychain reference may cross this local restoration boundary.
    pub async fn restore_bundle_reference_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        id: &str,
        field: &str,
        pointer: &str,
        reference: &str,
    ) -> AppResult<()> {
        self.restore_bundle_token_on(
            db,
            workspace,
            id,
            field,
            pointer,
            format!("{{{{@unfour-secret:{reference}}}}}"),
        )
        .await
    }
    pub async fn restore_bundle_template_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        id: &str,
        field: &str,
        pointer: &str,
        template: &str,
    ) -> AppResult<()> {
        if !is_bundle_variable_template(template) {
            return Err(AppError::Validation("WORKSPACE_BUNDLE_INVALID".into()));
        }
        self.restore_bundle_token_on(db, workspace, id, field, pointer, template.into())
            .await
    }
    async fn restore_bundle_token_on(
        &self,
        db: &mut SqliteConnection,
        workspace: &str,
        id: &str,
        field: &str,
        pointer: &str,
        token: String,
    ) -> AppResult<()> {
        let column = match field {
            "auth" => "auth_json",
            "headers" => "headers_json",
            "query" => "query_json",
            "url" => "url",
            _ => return Err(AppError::Validation("WORKSPACE_BUNDLE_INVALID".into())),
        };
        let mut value: String = sqlx::query_scalar(&format!("SELECT {column} FROM api_requests WHERE workspace_id=? AND id=? AND deleted_at IS NULL"))
            .bind(workspace).bind(id).fetch_one(&mut *db).await?;
        if field == "url" && pointer.is_empty() {
            value = token;
        } else {
            let mut json: serde_json::Value = serde_json::from_str(&value)?;
            let leaf = json
                .pointer_mut(pointer)
                .filter(|v| v.is_string())
                .ok_or_else(|| AppError::Validation("WORKSPACE_BUNDLE_INVALID".into()))?;
            *leaf = serde_json::Value::String(token);
            value = serde_json::to_string(&json)?;
        }
        sqlx::query(&format!("UPDATE api_requests SET {column}=? WHERE workspace_id=? AND id=? AND deleted_at IS NULL"))
            .bind(value).bind(workspace).bind(id).execute(&mut *db).await?;
        Ok(())
    }
}
