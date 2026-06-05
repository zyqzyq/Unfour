use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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

    #[cfg(test)]
    pub fn in_memory(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            backend: SecretStoreBackend::InMemory(Arc::new(Mutex::new(HashMap::new()))),
        }
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

        let record_id = Uuid::new_v4().to_string();
        let credential_ref = self.make_ref(&workspace_id, &kind, &record_id);
        self.write_secret(&credential_ref, &secret).await?;

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

        self.load_secret(&credential_ref).await
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
        self.write_secret(&metadata.credential_ref, &secret).await?;

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

        self.remove_secret(&credential_ref).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn credentials_are_created_read_and_deleted_by_reference() {
        let store = SecretStore::in_memory("unfour-test");

        let created = store
            .create_credential(
                "workspace-a".to_string(),
                "ssh-password".to_string(),
                "Deploy host password".to_string(),
                "secret-value".to_string(),
            )
            .await
            .expect("create credential");

        assert_eq!(created.workspace_id, "workspace-a");
        assert_eq!(created.kind, "ssh-password");
        assert_eq!(created.label, "Deploy host password");
        assert!(created
            .credential_ref
            .starts_with("unfour-test:workspace-a:ssh-password:"));
        assert!(!created.credential_ref.contains("secret-value"));

        let loaded = store
            .read_secret("workspace-a".to_string(), created.credential_ref.clone())
            .await
            .expect("read credential");
        assert_eq!(loaded, "secret-value");

        store
            .delete_credential("workspace-a".to_string(), created.credential_ref.clone())
            .await
            .expect("delete credential");

        let missing = store
            .read_secret("workspace-a".to_string(), created.credential_ref)
            .await;
        assert!(missing.is_err());
    }

    #[tokio::test]
    async fn credential_refs_cannot_cross_workspace_boundaries() {
        let store = SecretStore::in_memory("unfour-test");

        let created = store
            .create_credential(
                "workspace-a".to_string(),
                "database-password".to_string(),
                "Database password".to_string(),
                "secret-value".to_string(),
            )
            .await
            .expect("create credential");

        let cross_workspace = store
            .read_secret("workspace-b".to_string(), created.credential_ref)
            .await;
        assert!(cross_workspace.is_err());
    }

    #[tokio::test]
    async fn credentials_can_be_rotated_without_changing_reference() {
        let store = SecretStore::in_memory("unfour-test");
        let created = store
            .create_credential(
                "workspace-a".to_string(),
                "ssh-password".to_string(),
                "Deploy password".to_string(),
                "old-secret".to_string(),
            )
            .await
            .expect("create credential");

        let rotated = store
            .rotate_credential(
                "workspace-a".to_string(),
                created.credential_ref.clone(),
                "new-secret".to_string(),
            )
            .await
            .expect("rotate credential");

        assert_eq!(rotated.credential_ref, created.credential_ref);
        assert_eq!(rotated.workspace_id, "workspace-a");
        assert_eq!(rotated.kind, "ssh-password");
        assert_eq!(rotated.label, "Rotated credential");
        let loaded = store
            .read_secret("workspace-a".to_string(), rotated.credential_ref)
            .await
            .expect("read rotated credential");
        assert_eq!(loaded, "new-secret");
    }

    #[tokio::test]
    async fn credential_reference_metadata_is_derived_without_loading_secret() {
        let store = SecretStore::in_memory("unfour-test");
        let created = store
            .create_credential(
                "workspace-a".to_string(),
                "database-password".to_string(),
                "Database password".to_string(),
                "secret-value".to_string(),
            )
            .await
            .expect("create credential");

        let metadata = store
            .inspect_credential("workspace-a".to_string(), created.credential_ref.clone())
            .await
            .expect("inspect credential");
        let wrong_workspace = store
            .inspect_credential("workspace-b".to_string(), created.credential_ref)
            .await;

        assert_eq!(metadata.workspace_id, "workspace-a");
        assert_eq!(metadata.kind, "database-password");
        assert_eq!(metadata.label, "Credential reference");
        assert!(!metadata.credential_ref.contains("secret-value"));
        assert!(wrong_workspace.is_err());
    }
}
