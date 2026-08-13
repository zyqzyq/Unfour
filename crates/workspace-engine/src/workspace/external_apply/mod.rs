use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityType, ExternalApplyPage, ExternalApplyReport,
    ExternalDelete, ExternalVariableValue, MutationOrigin, SecretMaterialStatus,
};
use unfour_core::{AppError, AppResult};

use super::WorkspaceService;

mod environment;
mod variable;
mod workspace;

impl WorkspaceService {
    pub async fn apply_external_page_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        page: ExternalApplyPage,
    ) -> AppResult<DomainCommandResult<ExternalApplyReport>> {
        if context.origin != MutationOrigin::External {
            return Err(AppError::Config(
                "external apply requires an External command context".to_string(),
            ));
        }
        let mut mutations = Vec::new();
        let mut secret_material_outcomes = Vec::new();
        for change in page.workspaces {
            workspace::apply_workspace(connection, context, change, &mut mutations).await?;
        }
        for change in page.workspace_environments {
            environment::apply_environment(connection, context, change, &mut mutations).await?;
        }
        for change in page.workspace_variables {
            variable::apply_workspace_variable(
                connection,
                context,
                change,
                &mut mutations,
                &mut secret_material_outcomes,
            )
            .await?;
        }
        for change in page.workspace_environment_variables {
            variable::apply_environment_variable(
                connection,
                context,
                change,
                &mut mutations,
                &mut secret_material_outcomes,
            )
            .await?;
        }
        let report = ExternalApplyReport {
            applied_count: mutations.len(),
            mutations: mutations.clone(),
            secret_material_outcomes,
        };
        Ok(DomainCommandResult::new(report, mutations))
    }
}

pub(super) fn external_value(
    is_secret: bool,
    value: &ExternalVariableValue,
    current: Option<&String>,
) -> AppResult<(String, Option<SecretMaterialStatus>)> {
    match (is_secret, value) {
        (true, ExternalVariableValue::Set(_)) => Err(AppError::Validation(
            "secret external values must use PreserveLocal or Clear".to_string(),
        )),
        (true, ExternalVariableValue::PreserveLocal) => {
            let value = current.cloned().unwrap_or_default();
            let status = if current.is_some_and(|value| !value.is_empty()) {
                SecretMaterialStatus::Present
            } else {
                SecretMaterialStatus::Missing
            };
            Ok((value, Some(status)))
        }
        (true, ExternalVariableValue::Clear) => {
            Ok((String::new(), Some(SecretMaterialStatus::Missing)))
        }
        (false, ExternalVariableValue::Clear) => Ok((String::new(), None)),
        (false, ExternalVariableValue::Set(value)) => Ok((value.clone(), None)),
        (false, ExternalVariableValue::PreserveLocal) => Err(AppError::Validation(
            "plain external values must use Set or Clear".to_string(),
        )),
    }
}

pub(super) async fn delete_existing(
    connection: &mut SqliteConnection,
    table: &str,
    workspace_id: &str,
    entity_id: &str,
    deleted_at: &str,
) -> AppResult<Option<i64>> {
    if deleted_at.trim().is_empty() {
        return Err(AppError::Validation(
            "external delete requires deleted_at".to_string(),
        ));
    }
    let sql = match table {
        "workspaces" => {
            "UPDATE workspaces SET deleted_at = ?1, updated_at = ?1, revision = revision + 1 WHERE id = ?2 AND deleted_at IS NULL RETURNING revision"
        }
        "workspace_variables" => {
            "UPDATE workspace_variables SET deleted_at = ?1, updated_at = ?1, revision = revision + 1 WHERE id = ?2 AND workspace_id = ?3 AND deleted_at IS NULL RETURNING revision"
        }
        "workspace_environments" => {
            "UPDATE workspace_environments SET deleted_at = ?1, updated_at = ?1, revision = revision + 1 WHERE id = ?2 AND workspace_id = ?3 AND deleted_at IS NULL RETURNING revision"
        }
        "workspace_environment_variables" => {
            "UPDATE workspace_environment_variables SET deleted_at = ?1, updated_at = ?1, revision = revision + 1 WHERE id = ?2 AND workspace_id = ?3 AND deleted_at IS NULL RETURNING revision"
        }
        _ => return Err(AppError::Config("unsupported external entity table".to_string())),
    };
    let mut query = sqlx::query_scalar(sql).bind(deleted_at).bind(entity_id);
    if table != "workspaces" {
        query = query.bind(workspace_id);
    }
    Ok(query.fetch_optional(&mut *connection).await?)
}

pub(super) fn validate_delete(
    delete: &ExternalDelete,
    expected: DomainEntityType,
) -> AppResult<()> {
    if delete.entity.entity_type != expected {
        return Err(AppError::Validation(
            "external delete entity type does not match its apply collection".to_string(),
        ));
    }
    if delete.entity.workspace_id.trim().is_empty() || delete.entity.entity_id.trim().is_empty() {
        return Err(AppError::Validation(
            "external delete requires non-empty entity ids".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn normalized_key(key: &str) -> AppResult<String> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::Validation(
            "variable key cannot be empty".to_string(),
        ));
    }
    if key.chars().count() > 120 {
        return Err(AppError::Validation(
            "variable key must be 120 characters or fewer".to_string(),
        ));
    }
    Ok(key.to_string())
}
