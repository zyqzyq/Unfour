use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use unfour_core::models::{ApiClientPreferences, ApiRequestInput, MAX_API_TIMEOUT_MS};
use unfour_core::{AppError, AppResult};

use crate::CommandBus;

const API_REQUEST_TIMEOUT_SETTING_KEY: &str = "api_request_timeout_ms";
pub(crate) const MCP_DEFAULT_API_TIMEOUT_MS: u64 = 60_000;

#[derive(Clone, Default)]
pub(crate) struct ApiExecutionManager {
    active: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

pub(crate) struct ApiExecutionLease {
    execution_id: String,
    manager: ApiExecutionManager,
    token: CancellationToken,
}

impl ApiExecutionManager {
    pub(crate) fn register(&self, execution_id: &str) -> AppResult<ApiExecutionLease> {
        let execution_id = execution_id.trim();
        if execution_id.is_empty() || execution_id.len() > 128 {
            return Err(AppError::Validation(
                "API execution id must contain between 1 and 128 characters".to_string(),
            ));
        }
        let token = CancellationToken::new();
        let mut active = self
            .active
            .lock()
            .map_err(|_| AppError::Config("API execution registry is unavailable".to_string()))?;
        if active.contains_key(execution_id) {
            return Err(AppError::Validation(
                "API execution id is already active".to_string(),
            ));
        }
        active.insert(execution_id.to_string(), token.clone());
        Ok(ApiExecutionLease {
            execution_id: execution_id.to_string(),
            manager: self.clone(),
            token,
        })
    }

    pub(crate) fn cancel(&self, execution_id: &str) -> bool {
        let token = self
            .active
            .lock()
            .ok()
            .and_then(|active| active.get(execution_id.trim()).cloned());
        token.is_some_and(|token| {
            token.cancel();
            true
        })
    }

    #[cfg(test)]
    pub(crate) fn active_count(&self) -> usize {
        self.active.lock().map(|active| active.len()).unwrap_or(0)
    }
}

impl ApiExecutionLease {
    pub(crate) fn token(&self) -> CancellationToken {
        self.token.clone()
    }
}

impl Drop for ApiExecutionLease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.manager.active.lock() {
            active.remove(&self.execution_id);
        }
    }
}

impl CommandBus {
    pub async fn api_client_preferences(&self) -> AppResult<ApiClientPreferences> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?1")
                .bind(API_REQUEST_TIMEOUT_SETTING_KEY)
                .fetch_optional(self.db.pool())
                .await?;
        let request_timeout_ms = value
            .map(|value| parse_timeout_setting(&value))
            .transpose()?
            .unwrap_or(0);
        Ok(ApiClientPreferences { request_timeout_ms })
    }

    pub async fn update_api_client_preferences(
        &self,
        preferences: ApiClientPreferences,
    ) -> AppResult<ApiClientPreferences> {
        validate_timeout(preferences.request_timeout_ms)?;
        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(API_REQUEST_TIMEOUT_SETTING_KEY)
        .bind(preferences.request_timeout_ms.to_string())
        .execute(self.db.pool())
        .await?;
        Ok(preferences)
    }

    pub fn cancel_api_request(&self, execution_id: &str) -> bool {
        self.api_executions.cancel(execution_id)
    }

    pub(crate) async fn resolve_desktop_api_timeout(
        &self,
        mut input: ApiRequestInput,
    ) -> AppResult<ApiRequestInput> {
        let global = self.api_client_preferences().await?.request_timeout_ms;
        input.timeout_ms = Some(resolve_effective_timeout(input.timeout_ms, global)?);
        Ok(input)
    }
}

pub(crate) fn mcp_timeout(timeout_ms: Option<u64>) -> AppResult<u64> {
    let timeout_ms = timeout_ms.unwrap_or(MCP_DEFAULT_API_TIMEOUT_MS);
    validate_timeout(timeout_ms)?;
    Ok(timeout_ms)
}

fn resolve_effective_timeout(request_timeout_ms: Option<u64>, global: u64) -> AppResult<u64> {
    validate_timeout(global)?;
    if let Some(timeout_ms) = request_timeout_ms {
        validate_timeout(timeout_ms)?;
        Ok(timeout_ms)
    } else {
        Ok(global)
    }
}

fn parse_timeout_setting(value: &str) -> AppResult<u64> {
    value
        .parse::<u64>()
        .map_err(|_| AppError::Config("API request timeout preference is invalid".to_string()))
        .and_then(|value| {
            validate_timeout(value)?;
            Ok(value)
        })
}

fn validate_timeout(timeout_ms: u64) -> AppResult<()> {
    if timeout_ms > MAX_API_TIMEOUT_MS {
        return Err(AppError::Validation(format!(
            "API request timeout must be between 0 and {MAX_API_TIMEOUT_MS} milliseconds"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_timeout_supports_inherit_and_unlimited_override() {
        assert_eq!(resolve_effective_timeout(None, 5_000).unwrap(), 5_000);
        assert_eq!(resolve_effective_timeout(Some(0), 5_000).unwrap(), 0);
        assert_eq!(
            resolve_effective_timeout(Some(120_000), 5_000).unwrap(),
            120_000
        );
    }

    #[test]
    fn mcp_timeout_uses_safety_default_and_preserves_explicit_values() {
        assert_eq!(mcp_timeout(None).unwrap(), 60_000);
        assert_eq!(mcp_timeout(Some(0)).unwrap(), 0);
        assert_eq!(mcp_timeout(Some(120_000)).unwrap(), 120_000);
    }

    #[test]
    fn registry_cancels_only_the_selected_execution_and_cleans_up() {
        let manager = ApiExecutionManager::default();
        let first = manager.register("first").unwrap();
        let second = manager.register("second").unwrap();
        assert_eq!(manager.active_count(), 2);
        assert!(manager.cancel("first"));
        assert!(first.token().is_cancelled());
        assert!(!second.token().is_cancelled());
        assert!(!manager.cancel("unknown"));
        drop(first);
        drop(second);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn duplicate_execution_ids_are_rejected() {
        let manager = ApiExecutionManager::default();
        let _lease = manager.register("same").unwrap();
        assert!(matches!(
            manager.register("same"),
            Err(AppError::Validation(_))
        ));
    }
}
