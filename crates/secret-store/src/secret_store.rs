use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use unfour_core::models::CredentialMetadata;
use unfour_core::{AppError, AppResult};
use uuid::Uuid;

#[derive(Clone)]
pub struct SecretStore {
    service_name: String,
    backend: SecretStoreBackend,
}

#[derive(Clone)]
#[allow(dead_code)]
enum SecretStoreBackend {
    OsKeychain,
    InMemory(Arc<Mutex<HashMap<String, String>>>),
}

impl SecretStore {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            backend: SecretStoreBackend::OsKeychain,
        }
    }

    /// Create an in-memory secret store for testing. Avoids OS keychain access.
    pub fn in_memory(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            backend: SecretStoreBackend::InMemory(Arc::new(Mutex::new(HashMap::new()))),
        }
    }

    /// Store a secret under a caller-defined scope and key.
    pub async fn put_named_secret(&self, scope: &str, key: &str, value: &str) -> AppResult<()> {
        let storage_key = named_secret_storage_key(scope, key)?;
        if value.is_empty() {
            return Err(AppError::Validation(
                "named secret value cannot be empty".to_string(),
            ));
        }

        self.write_secret(&storage_key, value).await
    }

    /// Load a secret previously stored under a caller-defined scope and key.
    pub async fn get_named_secret(&self, scope: &str, key: &str) -> AppResult<String> {
        let storage_key = named_secret_storage_key(scope, key)?;
        self.load_secret(&storage_key)
            .await
            .map_err(named_secret_error)
    }

    /// Delete a secret previously stored under a caller-defined scope and key.
    pub async fn delete_named_secret(&self, scope: &str, key: &str) -> AppResult<()> {
        let storage_key = named_secret_storage_key(scope, key)?;
        self.remove_secret(&storage_key)
            .await
            .map_err(named_secret_error)
    }

    #[allow(dead_code)]
    pub fn make_ref(&self, workspace_id: &str, kind: &str, record_id: &str) -> String {
        format!(
            "{}:{}:{}:{}",
            self.service_name, workspace_id, kind, record_id
        )
    }

    pub async fn create_credential(
        &self,
        workspace_id: String,
        kind: String,
        label: String,
        secret: String,
    ) -> AppResult<CredentialMetadata> {
        let workspace_id = normalize_segment(&workspace_id, "workspace id")?;
        let kind = normalize_segment(&kind, "credential kind")?;
        let label = normalize_label(&label)?;
        if secret.is_empty() {
            return Err(AppError::Validation(
                "credential secret cannot be empty".to_string(),
            ));
        }

        let started = Instant::now();
        unfour_diag::log_operation_event(
            "keychain_save_started",
            "keychain",
            "create_credential",
            "started",
            None,
            None,
            serde_json::json!({ "credential_kind": &kind }),
        );
        let record_id = Uuid::new_v4().to_string();
        let credential_ref = self.make_ref(&workspace_id, &kind, &record_id);
        if let Err(error) = self.write_secret(&credential_ref, &secret).await {
            unfour_diag::log_operation_event(
                "keychain_save_failed",
                "keychain",
                "create_credential",
                "error",
                Some(started.elapsed().as_millis()),
                Some(unfour_diag::app_error_kind(&error)),
                serde_json::json!({ "credential_kind": &kind }),
            );
            return Err(error);
        }
        unfour_diag::log_operation_event(
            "keychain_save_completed",
            "keychain",
            "create_credential",
            "ok",
            Some(started.elapsed().as_millis()),
            None,
            serde_json::json!({ "credential_kind": &kind }),
        );

        Ok(CredentialMetadata {
            workspace_id,
            kind,
            label,
            credential_ref,
        })
    }

    #[allow(dead_code)]
    pub async fn read_secret(
        &self,
        workspace_id: String,
        credential_ref: String,
    ) -> AppResult<String> {
        let workspace_id = normalize_segment(&workspace_id, "workspace id")?;
        let parsed = self.parse_ref(&credential_ref)?;
        if parsed.workspace_id != workspace_id {
            return Err(AppError::Validation(
                "credential reference does not belong to the workspace".to_string(),
            ));
        }

        let started = Instant::now();
        unfour_diag::log_operation_event(
            "keychain_load_started",
            "keychain",
            "read_secret",
            "started",
            None,
            None,
            serde_json::json!({ "credential_kind": &parsed.kind }),
        );
        let result = self.load_secret(&credential_ref).await;
        match result {
            Ok(secret) => {
                unfour_diag::log_operation_event(
                    "keychain_load_completed",
                    "keychain",
                    "read_secret",
                    "ok",
                    Some(started.elapsed().as_millis()),
                    None,
                    serde_json::json!({ "credential_kind": &parsed.kind }),
                );
                Ok(secret)
            }
            Err(error) => {
                unfour_diag::log_operation_event(
                    "keychain_load_failed",
                    "keychain",
                    "read_secret",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(unfour_diag::app_error_kind(&error)),
                    serde_json::json!({ "credential_kind": &parsed.kind }),
                );
                Err(error)
            }
        }
    }

    pub async fn inspect_credential(
        &self,
        workspace_id: String,
        credential_ref: String,
    ) -> AppResult<CredentialMetadata> {
        let workspace_id = normalize_segment(&workspace_id, "workspace id")?;
        let parsed = self.parse_ref(&credential_ref)?;
        if parsed.workspace_id != workspace_id {
            return Err(AppError::Validation(
                "credential reference does not belong to the workspace".to_string(),
            ));
        }

        Ok(CredentialMetadata {
            workspace_id,
            kind: parsed.kind,
            label: "Credential reference".to_string(),
            credential_ref,
        })
    }

    pub async fn rotate_credential(
        &self,
        workspace_id: String,
        credential_ref: String,
        secret: String,
    ) -> AppResult<CredentialMetadata> {
        if secret.is_empty() {
            return Err(AppError::Validation(
                "credential secret cannot be empty".to_string(),
            ));
        }
        let metadata = self
            .inspect_credential(workspace_id, credential_ref)
            .await?;
        let started = Instant::now();
        unfour_diag::log_operation_event(
            "keychain_save_started",
            "keychain",
            "rotate_credential",
            "started",
            None,
            None,
            serde_json::json!({ "credential_kind": &metadata.kind }),
        );
        if let Err(error) = self.write_secret(&metadata.credential_ref, &secret).await {
            unfour_diag::log_operation_event(
                "keychain_save_failed",
                "keychain",
                "rotate_credential",
                "error",
                Some(started.elapsed().as_millis()),
                Some(unfour_diag::app_error_kind(&error)),
                serde_json::json!({ "credential_kind": &metadata.kind }),
            );
            return Err(error);
        }
        unfour_diag::log_operation_event(
            "keychain_save_completed",
            "keychain",
            "rotate_credential",
            "ok",
            Some(started.elapsed().as_millis()),
            None,
            serde_json::json!({ "credential_kind": &metadata.kind }),
        );

        Ok(CredentialMetadata {
            label: "Rotated credential".to_string(),
            ..metadata
        })
    }

    pub async fn delete_credential(
        &self,
        workspace_id: String,
        credential_ref: String,
    ) -> AppResult<()> {
        let workspace_id = normalize_segment(&workspace_id, "workspace id")?;
        let parsed = self.parse_ref(&credential_ref)?;
        if parsed.workspace_id != workspace_id {
            return Err(AppError::Validation(
                "credential reference does not belong to the workspace".to_string(),
            ));
        }

        let started = Instant::now();
        unfour_diag::log_operation_event(
            "keychain_delete_started",
            "keychain",
            "delete_credential",
            "started",
            None,
            None,
            serde_json::json!({ "credential_kind": &parsed.kind }),
        );
        let result = self.remove_secret(&credential_ref).await;
        match result {
            Ok(()) => {
                unfour_diag::log_operation_event(
                    "keychain_delete_completed",
                    "keychain",
                    "delete_credential",
                    "ok",
                    Some(started.elapsed().as_millis()),
                    None,
                    serde_json::json!({ "credential_kind": &parsed.kind }),
                );
                Ok(())
            }
            Err(error) => {
                unfour_diag::log_operation_event(
                    "keychain_delete_failed",
                    "keychain",
                    "delete_credential",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(unfour_diag::app_error_kind(&error)),
                    serde_json::json!({ "credential_kind": &parsed.kind }),
                );
                Err(error)
            }
        }
    }

    pub fn capability_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "provider": match self.backend {
                SecretStoreBackend::OsKeychain => "os-keychain",
                SecretStoreBackend::InMemory(_) => "in-memory-test",
            },
            "plainTextStorage": false,
            "refFormat": format!("{}:<workspace>:<kind>:<record>", self.service_name)
        })
    }

    async fn write_secret(&self, credential_ref: &str, secret: &str) -> AppResult<()> {
        match &self.backend {
            SecretStoreBackend::OsKeychain => {
                let entry = keyring::Entry::new(&self.service_name, credential_ref)
                    .map_err(|error| AppError::Config(error.to_string()))?;
                entry
                    .set_password(secret)
                    .map_err(|error| AppError::Config(error.to_string()))
            }
            SecretStoreBackend::InMemory(values) => {
                values
                    .lock()
                    .map_err(|_| AppError::Config("secret store lock poisoned".to_string()))?
                    .insert(credential_ref.to_string(), secret.to_string());
                Ok(())
            }
        }
    }

    async fn load_secret(&self, credential_ref: &str) -> AppResult<String> {
        match &self.backend {
            SecretStoreBackend::OsKeychain => {
                let entry = keyring::Entry::new(&self.service_name, credential_ref)
                    .map_err(|error| AppError::Config(error.to_string()))?;
                entry
                    .get_password()
                    .map_err(|_| AppError::NotFound("credential".to_string()))
            }
            SecretStoreBackend::InMemory(values) => values
                .lock()
                .map_err(|_| AppError::Config("secret store lock poisoned".to_string()))?
                .get(credential_ref)
                .cloned()
                .ok_or_else(|| AppError::NotFound("credential".to_string())),
        }
    }

    async fn remove_secret(&self, credential_ref: &str) -> AppResult<()> {
        match &self.backend {
            SecretStoreBackend::OsKeychain => {
                let entry = keyring::Entry::new(&self.service_name, credential_ref)
                    .map_err(|error| AppError::Config(error.to_string()))?;
                entry
                    .delete_credential()
                    .map_err(|_| AppError::NotFound("credential".to_string()))
            }
            SecretStoreBackend::InMemory(values) => {
                let removed = values
                    .lock()
                    .map_err(|_| AppError::Config("secret store lock poisoned".to_string()))?
                    .remove(credential_ref);
                removed
                    .map(|_| ())
                    .ok_or_else(|| AppError::NotFound("credential".to_string()))
            }
        }
    }

    fn parse_ref(&self, credential_ref: &str) -> AppResult<ParsedCredentialRef> {
        let mut parts = credential_ref.splitn(4, ':');
        let service_name = parts.next().unwrap_or_default();
        let workspace_id = parts.next().unwrap_or_default();
        let kind = parts.next().unwrap_or_default();
        let record_id = parts.next().unwrap_or_default();

        if service_name != self.service_name
            || workspace_id.is_empty()
            || kind.is_empty()
            || record_id.is_empty()
        {
            return Err(AppError::Validation(
                "credential reference is invalid".to_string(),
            ));
        }

        Ok(ParsedCredentialRef {
            workspace_id: workspace_id.to_string(),
            kind: kind.to_string(),
        })
    }
}

struct ParsedCredentialRef {
    workspace_id: String,
    kind: String,
}

fn normalize_segment(value: &str, label: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{} cannot be empty", label)));
    }
    if trimmed.contains(':') || trimmed.chars().any(char::is_control) {
        return Err(AppError::Validation(format!(
            "{} cannot contain separators or control characters",
            label
        )));
    }
    Ok(trimmed.to_string())
}

fn normalize_label(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "credential label cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 120 {
        return Err(AppError::Validation(
            "credential label must be 120 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn named_secret_storage_key(scope: &str, key: &str) -> AppResult<String> {
    let scope = normalize_segment(scope, "named secret scope")?;
    let key = normalize_segment(key, "named secret key")?;
    Ok(format!("named:{scope}:{key}"))
}

fn named_secret_error(error: AppError) -> AppError {
    match error {
        AppError::NotFound(_) => AppError::NotFound("named secret".to_string()),
        error => error,
    }
}

#[cfg(test)]
#[path = "secret_store_tests/mod.rs"]
mod secret_store_tests;
