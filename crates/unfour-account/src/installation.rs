use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use unfour_core::AppError;
use unfour_secret_store::SecretStore;

use crate::AccountError;

// Compatibility key: changing this would rotate installation identity for
// users migrating from the former split desktop runtime.
const INSTALLATION_SCOPE: &str = "pro-installation";
const INSTALLATION_KEY: &str = "installation-id";
const INSTALLATION_ID_BYTES: usize = 32;
const INSTALLATION_ID_LENGTH: usize = 43;

/// Persists the app-installation identity independently from account sessions.
/// Account sign-out must never delete this scope/key.
#[derive(Clone)]
pub(crate) struct InstallationStore {
    secrets: SecretStore,
}

impl InstallationStore {
    pub(crate) fn new(secrets: SecretStore) -> Self {
        Self { secrets }
    }

    pub(crate) async fn get_or_create(&self) -> Result<String, AccountError> {
        match self
            .secrets
            .get_named_secret(INSTALLATION_SCOPE, INSTALLATION_KEY)
            .await
        {
            Ok(value) => {
                validate_installation_id(&value)?;
                Ok(value)
            }
            Err(AppError::NotFound(_)) => self.generate_and_save().await,
            Err(_) => Err(AccountError::InstallationIdUnavailable),
        }
    }

    async fn generate_and_save(&self) -> Result<String, AccountError> {
        let mut bytes = [0_u8; INSTALLATION_ID_BYTES];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| AccountError::InstallationIdUnavailable)?;
        let installation_id = URL_SAFE_NO_PAD.encode(bytes);
        validate_installation_id(&installation_id)?;
        self.secrets
            .put_named_secret(INSTALLATION_SCOPE, INSTALLATION_KEY, &installation_id)
            .await
            .map_err(|_| AccountError::InstallationIdUnavailable)?;
        Ok(installation_id)
    }
}

fn validate_installation_id(value: &str) -> Result<(), AccountError> {
    if value.len() != INSTALLATION_ID_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(AccountError::CorruptInstallationId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_id_is_random_base64url_without_padding() {
        let store = InstallationStore::new(SecretStore::in_memory("unfour-test"));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime");

        runtime.block_on(async {
            let value = store.get_or_create().await.expect("installation id");
            assert_eq!(value.len(), INSTALLATION_ID_LENGTH);
            assert!(value.len() <= 128);
            assert!(value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'));
            assert!(!value.contains('='));
            assert_eq!(store.get_or_create().await.expect("stored id"), value);
        });
    }

    #[test]
    fn corrupt_persisted_id_returns_a_stable_error_without_rotating_it() {
        let secrets = SecretStore::in_memory("unfour-test");
        let store = InstallationStore::new(secrets.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime");

        runtime.block_on(async {
            secrets
                .put_named_secret(INSTALLATION_SCOPE, INSTALLATION_KEY, "invalid")
                .await
                .expect("seed invalid id");
            let error = store.get_or_create().await.expect_err("reject invalid id");
            assert!(matches!(error, AccountError::CorruptInstallationId));
            assert_eq!(error.code(), "corrupt_installation_id");
            assert_eq!(
                secrets
                    .get_named_secret(INSTALLATION_SCOPE, INSTALLATION_KEY)
                    .await
                    .expect("invalid id remains available for diagnosis"),
                "invalid"
            );
        });
    }
}
