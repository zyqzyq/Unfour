use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use unfour_core::{AppError, AppResult};
use unfour_secret_store::SecretStore;

pub(crate) const TELEMETRY_INSTALLATION_SCOPE: &str = "telemetry";
pub(crate) const TELEMETRY_INSTALLATION_KEY: &str = "anonymous-installation-id";
pub(crate) const INSTALLATION_ID_LENGTH: usize = 43;
const INSTALLATION_ID_BYTES: usize = 32;

/// Keeps telemetry identity separate from Account installation and session
/// scopes. Account sign-in, sign-out, and entitlement changes never touch it.
#[derive(Clone)]
pub(crate) struct TelemetryInstallationStore {
    secrets: SecretStore,
}

impl TelemetryInstallationStore {
    pub(crate) fn new(secrets: SecretStore) -> Self {
        Self { secrets }
    }

    pub(crate) async fn get_or_create(&self) -> AppResult<String> {
        match self
            .secrets
            .get_named_secret(TELEMETRY_INSTALLATION_SCOPE, TELEMETRY_INSTALLATION_KEY)
            .await
        {
            Ok(value) => {
                validate_installation_id(&value)?;
                Ok(value)
            }
            Err(AppError::NotFound(_)) => self.generate_and_save().await,
            Err(error) => Err(error),
        }
    }

    async fn generate_and_save(&self) -> AppResult<String> {
        let mut bytes = [0_u8; INSTALLATION_ID_BYTES];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| AppError::Config("telemetry installation id unavailable".to_string()))?;
        let installation_id = URL_SAFE_NO_PAD.encode(bytes);
        validate_installation_id(&installation_id)?;
        self.secrets
            .put_named_secret(
                TELEMETRY_INSTALLATION_SCOPE,
                TELEMETRY_INSTALLATION_KEY,
                &installation_id,
            )
            .await?;
        Ok(installation_id)
    }
}

fn validate_installation_id(value: &str) -> AppResult<()> {
    if value.len() != INSTALLATION_ID_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(AppError::Config(
            "stored telemetry installation id is invalid".to_string(),
        ));
    }
    Ok(())
}
