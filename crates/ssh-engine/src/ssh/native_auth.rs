//! Credential resolution and authentication, independent of SSH channel use.
use super::*;

impl SshService {
    #[cfg(feature = "ssh-native")]
    pub(super) async fn authenticate_native(
        &self,
        handle: &mut russh::client::Handle<SshClientHandler>,
        connection: &SshConnection,
        override_secret: Option<&str>,
    ) -> AppResult<()> {
        match connection.auth_kind.as_str() {
            "password" => {
                // A transient override (e.g. testing a not-yet-saved password)
                // takes precedence over the stored keychain credential and is
                // never persisted.
                let password = match override_secret {
                    Some(secret) => secret.to_string(),
                    None => {
                        let credential_ref =
                            connection.credential_ref.as_deref().ok_or_else(|| {
                                AppError::Validation(
                                    "password auth requires a credential reference".to_string(),
                                )
                            })?;
                        self.secret_store
                            .read_secret(
                                connection.workspace_id.clone(),
                                credential_ref.to_string(),
                            )
                            .await
                            .map_err(|error| {
                                // Surface the underlying cause (e.g. credential
                                // not found) so the user knows the stored secret
                                // is missing rather than facing an opaque
                                // failure. The credential for a password
                                // connection must be created via the dialog's
                                // "create credential" action before connecting.
                                AppError::Config(format!(
                                    "failed to read ssh credential from secret store \
                                     (create the credential before connecting): {}",
                                    error
                                ))
                            })?
                    }
                };
                let result = handle
                    .authenticate_password(connection.username.clone(), password)
                    .await
                    .map_err(|_| AppError::Config("ssh authentication failed".to_string()))?;
                if !result.success() {
                    return Err(AppError::Config(
                        "ssh authentication failed: invalid credentials".to_string(),
                    ));
                }
            }
            "private-key" => {
                let key_path = connection.key_path.as_deref().ok_or_else(|| {
                    AppError::Validation("private-key auth requires a key path".to_string())
                })?;
                let path = std::path::Path::new(key_path);
                if !path.exists() {
                    return Err(AppError::Config(format!(
                        "ssh private key file not found: {}",
                        key_path
                    )));
                }
                let key_bytes = zeroize::Zeroizing::new(tokio::fs::read(path).await?);
                let mut private_key = ssh_key::PrivateKey::from_openssh(&key_bytes)
                    .map_err(|_| AppError::Config("failed to read ssh private key".into()))?;
                if private_key.is_encrypted() {
                    let passphrase = zeroize::Zeroizing::new(match override_secret {
                        Some(secret) => secret.to_owned(),
                        None => {
                            let reference = connection.credential_ref.clone().ok_or_else(|| {
                                AppError::Validation(
                                    "encrypted SSH key requires a passphrase".into(),
                                )
                            })?;
                            self.secret_store
                                .read_secret(connection.workspace_id.clone(), reference)
                                .await?
                        }
                    });
                    private_key = tokio::task::spawn_blocking(move || {
                        private_key.decrypt(passphrase.as_bytes())
                    })
                    .await
                    .map_err(|_| AppError::Config("failed to load ssh private key".into()))?
                    .map_err(|_| AppError::Config("failed to decrypt ssh private key".into()))?;
                }
                let key = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(private_key), None);
                let result = handle
                    .authenticate_publickey(connection.username.clone(), key)
                    .await
                    .map_err(|_| {
                        AppError::Config("ssh public key authentication failed".to_string())
                    })?;
                if !result.success() {
                    return Err(AppError::Config(
                        "ssh public key authentication failed: key rejected by server".to_string(),
                    ));
                }
            }
            "none" => {
                let result = handle
                    .authenticate_none(connection.username.clone())
                    .await
                    .map_err(|_| AppError::Config("ssh authentication failed".to_string()))?;
                if !result.success() {
                    return Err(AppError::Config(
                        "ssh authentication failed: server rejected unauthenticated access"
                            .to_string(),
                    ));
                }
            }
            _ => {
                return Err(AppError::Validation(format!(
                    "unsupported ssh auth kind: {}",
                    connection.auth_kind
                )));
            }
        }
        Ok(())
    }
}
