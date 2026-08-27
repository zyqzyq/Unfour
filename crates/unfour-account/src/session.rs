use unfour_core::AppError;
use unfour_secret_store::SecretStore;

use crate::types::StoredSession;
use crate::AccountError;

pub(crate) const KEYCHAIN_SERVICE: &str = "unfour";
// Compatibility key: changing this would orphan sessions written by the
// former split desktop runtime.
const SESSION_SCOPE: &str = "pro-auth";
const SESSION_KEY: &str = "login-key";

#[derive(Clone)]
pub(crate) struct SessionStore {
    secrets: SecretStore,
}

impl SessionStore {
    pub(crate) fn from_secret_store(secrets: SecretStore) -> Self {
        Self { secrets }
    }

    pub(crate) async fn load(&self) -> Result<Option<StoredSession>, AccountError> {
        let serialized = match self
            .secrets
            .get_named_secret(SESSION_SCOPE, SESSION_KEY)
            .await
        {
            Ok(value) => value,
            Err(AppError::NotFound(_)) => return Ok(None),
            Err(_) => return Err(AccountError::KeychainUnavailable),
        };
        let session = serde_json::from_str::<StoredSession>(&serialized)
            .map_err(|_| AccountError::CorruptStoredSession)?;
        session
            .validate()
            .map_err(|_| AccountError::CorruptStoredSession)?;
        Ok(Some(session))
    }

    pub(crate) async fn save(&self, session: StoredSession) -> Result<(), AccountError> {
        session.validate()?;
        if session.is_expired() {
            return Err(AccountError::InvalidApiResponse);
        }
        let serialized =
            serde_json::to_string(&session).map_err(|_| AccountError::InvalidApiResponse)?;
        self.secrets
            .put_named_secret(SESSION_SCOPE, SESSION_KEY, &serialized)
            .await
            .map_err(|_| AccountError::KeychainUnavailable)
    }

    pub(crate) async fn delete(&self) -> Result<(), AccountError> {
        match self
            .secrets
            .delete_named_secret(SESSION_SCOPE, SESSION_KEY)
            .await
        {
            Ok(()) => Ok(()),
            Err(AppError::NotFound(_)) => Ok(()),
            Err(_) => Err(AccountError::KeychainUnavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Duration, OffsetDateTime};

    #[test]
    fn serialized_session_contains_only_the_opaque_token_and_expiration() {
        let session = StoredSession {
            session_token: "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde".to_string(),
            expires_at: OffsetDateTime::now_utc() + Duration::days(30),
        };
        let serialized = serde_json::to_string(&session).expect("serialize session");
        let value: serde_json::Value = serde_json::from_str(&serialized).expect("JSON payload");
        let restored: StoredSession = serde_json::from_str(&serialized).expect("restore session");
        assert_eq!(restored.session_token, session.session_token);
        assert_eq!(value.as_object().expect("session object").len(), 2);
        assert!(value.get("sessionToken").is_some());
        assert!(value.get("expiresAt").is_some());
        assert!(value.get("accessToken").is_none());
        assert!(value.get("refreshToken").is_none());
        assert!(value.get("account").is_none());
        assert_eq!(KEYCHAIN_SERVICE, "unfour");
        assert_eq!(SESSION_SCOPE, "pro-auth");
        assert_eq!(SESSION_KEY, "login-key");
    }

    #[test]
    fn session_round_trips_through_the_core_named_secret_store() {
        let store = SessionStore {
            secrets: SecretStore::in_memory(KEYCHAIN_SERVICE),
        };
        let session = StoredSession {
            session_token: "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde".to_string(),
            expires_at: OffsetDateTime::now_utc() + Duration::days(30),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime");

        runtime.block_on(async {
            assert!(store.load().await.expect("empty store").is_none());
            store.save(session.clone()).await.expect("save session");
            let restored = store
                .load()
                .await
                .expect("load session")
                .expect("stored session");
            assert_eq!(restored.session_token, session.session_token);
            assert_eq!(restored.expires_at, session.expires_at);
            store.delete().await.expect("delete session");
            assert!(store.load().await.expect("deleted store").is_none());
            store.delete().await.expect("idempotent delete");
        });
    }
}
