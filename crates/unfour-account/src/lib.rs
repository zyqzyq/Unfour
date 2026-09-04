#[cfg(test)]
mod billing_tests;
mod client;
mod installation;
mod pkce;
#[cfg(test)]
mod service_tests;
mod session;
mod types;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use client::AccountClient;
use installation::InstallationStore;
use pkce::PendingAuthorization;
use session::{SessionStore, KEYCHAIN_SERVICE};
use thiserror::Error;
use types::{AuthCallback, StoredSession};
use url::Url;

pub use types::{
    AccountState, AccountSummary, ApiErrorCode, BillingUrl, DeviceSummary, EntitlementStatus,
    EntitlementSummary, AUTH_CALLBACK_URI,
};

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("invalid account service configuration")]
    InvalidConfiguration,
    #[error("invalid account callback: {0}")]
    InvalidDeepLink(&'static str),
    #[error("there is no pending account authorization")]
    NoPendingAuthorization,
    #[error("the account callback state does not match")]
    StateMismatch,
    #[error("account authorization was denied")]
    AuthorizationDenied,
    #[error("the account API is unavailable")]
    ApiUnavailable,
    #[error("the account API rejected the request with status {status} ({code})")]
    ApiRejected { status: u16, code: ApiErrorCode },
    #[error("the account API returned an invalid response")]
    InvalidApiResponse,
    #[error("the billing API returned an invalid HTTPS destination")]
    InvalidBillingUrl,
    #[error("the operating system keychain is unavailable")]
    KeychainUnavailable,
    #[error("the installation identifier is unavailable")]
    InstallationIdUnavailable,
    #[error("the stored installation identifier is invalid")]
    CorruptInstallationId,
    #[error("the stored account session is invalid")]
    CorruptStoredSession,
    #[error("the account state lock is unavailable")]
    StateUnavailable,
    #[error("an account sign-in is required")]
    SignedOut,
    #[error("the required account entitlement is unavailable")]
    EntitlementUnavailable,
}

impl AccountError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "invalid_configuration",
            Self::InvalidDeepLink(_) => "invalid_deep_link",
            Self::NoPendingAuthorization => "no_pending_authorization",
            Self::StateMismatch => "state_mismatch",
            Self::AuthorizationDenied => "authorization_denied",
            Self::ApiUnavailable => "api_unavailable",
            Self::ApiRejected { code, .. } => code.as_str(),
            Self::InvalidApiResponse => "invalid_api_response",
            Self::InvalidBillingUrl => "invalid_billing_url",
            Self::KeychainUnavailable => "keychain_unavailable",
            Self::InstallationIdUnavailable => "installation_id_unavailable",
            Self::CorruptInstallationId => "corrupt_installation_id",
            Self::CorruptStoredSession => "corrupt_stored_session",
            Self::StateUnavailable => "state_unavailable",
            Self::SignedOut => "signed_out",
            Self::EntitlementUnavailable => "entitlement_unavailable",
        }
    }

    fn is_invalid_session(&self) -> bool {
        matches!(
            self,
            Self::ApiRejected {
                code: ApiErrorCode::Unauthorized | ApiErrorCode::DesktopSessionExpired,
                ..
            }
        )
    }
}

/// Owns the account authorization flow and the OS-keychain session boundary.
#[derive(Clone)]
pub struct AccountService {
    client: AccountClient,
    sessions: SessionStore,
    installation: InstallationStore,
    pending: Arc<Mutex<Option<PendingAuthorization>>>,
    entitlement_cache: Arc<Mutex<Option<CachedAuthorization>>>,
    generation: Arc<AtomicU64>,
    session_transition: Arc<tokio::sync::Mutex<()>>,
}

const ENTITLEMENT_CACHE_TTL: Duration = Duration::from_secs(30);

struct CachedAuthorization {
    entitlement_code: String,
    session_token: String,
    account_id: String,
    generation: u64,
    expires_at: Instant,
}

/// A Rust-only authorized session capability. It is deliberately not
/// serializable and redacts its credential from Debug output.
pub struct AuthorizedSession {
    session_token: String,
    account_id: String,
    generation: u64,
}

impl AuthorizedSession {
    pub fn desktop_session_token(&self) -> &str {
        &self.session_token
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl std::fmt::Debug for AuthorizedSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AuthorizedSession([REDACTED])")
    }
}

impl AccountService {
    pub fn new(
        api_base_url: &str,
        web_base_url: &str,
        allow_loopback_http: bool,
    ) -> Result<Self, AccountError> {
        let secrets = unfour_secret_store::SecretStore::new(KEYCHAIN_SERVICE);
        Self::with_secret_store(api_base_url, web_base_url, allow_loopback_http, secrets)
    }

    fn with_secret_store(
        api_base_url: &str,
        web_base_url: &str,
        allow_loopback_http: bool,
        secrets: unfour_secret_store::SecretStore,
    ) -> Result<Self, AccountError> {
        Ok(Self {
            client: AccountClient::new(api_base_url, web_base_url, allow_loopback_http)?,
            sessions: SessionStore::from_secret_store(secrets.clone()),
            installation: InstallationStore::new(secrets),
            pending: Arc::new(Mutex::new(None)),
            entitlement_cache: Arc::new(Mutex::new(None)),
            generation: Arc::new(AtomicU64::new(0)),
            session_transition: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Creates a hosted checkout for the currently saved desktop session.
    pub async fn create_billing_checkout(&self) -> Result<BillingUrl, AccountError> {
        let session = self.active_stored_session().await?;
        let result = self
            .client
            .create_billing_checkout(&session.session_token)
            .await;
        self.finish_billing_request(&session, result).await
    }

    /// Creates a customer portal link for the currently saved desktop session.
    pub async fn create_billing_portal(&self) -> Result<BillingUrl, AccountError> {
        let session = self.active_stored_session().await?;
        let result = self
            .client
            .create_billing_portal(&session.session_token)
            .await;
        self.finish_billing_request(&session, result).await
    }

    async fn active_stored_session(&self) -> Result<StoredSession, AccountError> {
        loop {
            let session = match self.sessions.load().await {
                Ok(Some(session)) => session,
                Ok(None) => return Err(AccountError::SignedOut),
                Err(AccountError::CorruptStoredSession) => {
                    self.sessions.delete().await?;
                    self.advance_generation();
                    return Err(AccountError::SignedOut);
                }
                Err(error) => return Err(error),
            };
            if session.is_expired() {
                if self.delete_session_if_current(&session).await? {
                    return Err(AccountError::SignedOut);
                }
                continue;
            }
            return Ok(session);
        }
    }

    async fn finish_billing_request(
        &self,
        session: &StoredSession,
        result: Result<BillingUrl, AccountError>,
    ) -> Result<BillingUrl, AccountError> {
        match result {
            Err(error) if error.is_invalid_session() => {
                if self.delete_session_if_current(session).await? {
                    Err(AccountError::SignedOut)
                } else {
                    Err(error)
                }
            }
            result => result,
        }
    }

    /// Delete a session only when the invalid response belongs to the session
    /// that is still stored. A delayed `/v1/me` or billing response must not
    /// delete credentials written by a newer sign-in.
    async fn delete_session_if_current(
        &self,
        expected: &StoredSession,
    ) -> Result<bool, AccountError> {
        let _transition = self.session_transition.lock().await;
        let Some(current) = self.sessions.load().await? else {
            return Ok(false);
        };
        if current.session_token != expected.session_token
            || current.expires_at != expected.expires_at
        {
            return Ok(false);
        }
        self.sessions.delete().await?;
        self.advance_generation();
        Ok(true)
    }

    pub fn invalidate_entitlement_cache(&self) {
        if let Ok(mut cache) = self.entitlement_cache.lock() {
            cache.take();
        }
    }

    fn advance_generation(&self) {
        self.invalidate_entitlement_cache();
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    pub async fn state(&self) -> Result<AccountState, AccountError> {
        loop {
            if self
                .pending
                .lock()
                .map_err(|_| AccountError::StateUnavailable)?
                .is_some()
            {
                return Ok(AccountState::SigningIn);
            }
            let request_generation = self.generation();
            let session = match self.sessions.load().await {
                Ok(session) => session,
                Err(AccountError::CorruptStoredSession) => {
                    self.sessions.delete().await?;
                    self.advance_generation();
                    None
                }
                Err(error) => return Err(error),
            };
            let Some(session) = session else {
                if request_generation == self.generation() {
                    return Ok(AccountState::SignedOut);
                }
                continue;
            };
            if session.is_expired() {
                if self.delete_session_if_current(&session).await? {
                    return Ok(AccountState::SignedOut);
                }
                continue;
            }

            match self.client.get_account(&session.session_token).await {
                Ok(profile) if request_generation == self.generation() => {
                    return Ok(AccountState::SignedIn { profile });
                }
                Ok(_) => continue,
                Err(error) if error.is_invalid_session() => {
                    if self.delete_session_if_current(&session).await? {
                        return Ok(AccountState::SignedOut);
                    }
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Enforce a feature entitlement at the Rust network boundary and return
    /// the opaque desktop session only after `/v1/me` confirms it is active.
    pub async fn require_entitlement(
        &self,
        entitlement_code: &str,
    ) -> Result<AuthorizedSession, AccountError> {
        if entitlement_code.trim().is_empty() {
            return Err(AccountError::EntitlementUnavailable);
        }
        loop {
            let generation = self.generation();
            if let Ok(cache) = self.entitlement_cache.lock() {
                if let Some(cache) = cache.as_ref().filter(|cache| {
                    cache.entitlement_code == entitlement_code
                        && cache.generation == generation
                        && cache.expires_at > Instant::now()
                }) {
                    return Ok(AuthorizedSession {
                        session_token: cache.session_token.clone(),
                        account_id: cache.account_id.clone(),
                        generation,
                    });
                }
            }
            let session = match self.sessions.load().await? {
                Some(session) => session,
                None => {
                    if generation == self.generation() {
                        return Err(AccountError::SignedOut);
                    }
                    continue;
                }
            };
            if session.is_expired() {
                if self.delete_session_if_current(&session).await? {
                    return Err(AccountError::SignedOut);
                }
                continue;
            }
            let profile = match self.client.get_account(&session.session_token).await {
                Ok(profile) if generation == self.generation() => profile,
                Ok(_) => continue,
                Err(error) if error.is_invalid_session() => {
                    if self.delete_session_if_current(&session).await? {
                        return Err(AccountError::SignedOut);
                    }
                    continue;
                }
                Err(error) => return Err(error),
            };
            if !profile.has_active_entitlement(entitlement_code, time::OffsetDateTime::now_utc()) {
                return Err(AccountError::EntitlementUnavailable);
            }
            let account_id = profile.id;
            let session_token = session.session_token;
            if let Ok(mut cache) = self.entitlement_cache.lock() {
                *cache = Some(CachedAuthorization {
                    entitlement_code: entitlement_code.to_string(),
                    session_token: session_token.clone(),
                    account_id: account_id.clone(),
                    generation,
                    expires_at: Instant::now() + ENTITLEMENT_CACHE_TTL,
                });
            }
            return Ok(AuthorizedSession {
                session_token,
                account_id,
                generation,
            });
        }
    }

    /// Starts a new authorization attempt and returns the web URL to the Tauri adapter.
    /// The URL contains the stable installation ID, state, and PKCE challenge,
    /// never the verifier or a token.
    pub async fn begin_sign_in(&self) -> Result<Url, AccountError> {
        let installation_id = self.installation.get_or_create().await?;
        let pending = PendingAuthorization::generate();
        let url = self.client.authorization_url(&pending, &installation_id)?;
        *self
            .pending
            .lock()
            .map_err(|_| AccountError::StateUnavailable)? = Some(pending);
        Ok(url)
    }

    pub fn cancel_sign_in(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.take();
        }
    }

    pub async fn handle_deep_link(&self, raw: &str) -> Result<AccountState, AccountError> {
        let callback = AuthCallback::parse(raw)?;
        let generation = self.generation();
        let pending = {
            let mut guard = self
                .pending
                .lock()
                .map_err(|_| AccountError::StateUnavailable)?;
            let pending = guard.as_ref().ok_or(AccountError::NoPendingAuthorization)?;
            if callback.state() != pending.state {
                return Err(AccountError::StateMismatch);
            }
            guard.take().ok_or(AccountError::NoPendingAuthorization)?
        };

        let AuthCallback::Code {
            authorization_code,
            state,
        } = callback
        else {
            return Err(AccountError::AuthorizationDenied);
        };
        let session = self
            .client
            .exchange_code(authorization_code, state, pending.code_verifier)
            .await?;
        let profile = session.account.clone();
        // Sign-out and exchange completion must not interleave keychain writes.
        // A response from before sign-out cannot restore the deleted session.
        let _transition = self.session_transition.lock().await;
        if generation != self.generation() {
            return Err(AccountError::NoPendingAuthorization);
        }
        self.sessions.save(session.stored_session()).await?;
        self.advance_generation();
        Ok(AccountState::SignedIn { profile })
    }

    pub async fn sign_out(&self) -> Result<AccountState, AccountError> {
        let session = self.clear_local_session_for_sign_out().await?;
        if let Some(session) = session {
            // Revocation happens only after the local credential is gone and
            // the generation has invalidated every in-flight capability.
            let _ = self.client.revoke_session(&session.session_token).await;
        }
        Ok(AccountState::SignedOut)
    }

    async fn clear_local_session_for_sign_out(
        &self,
    ) -> Result<Option<StoredSession>, AccountError> {
        let _transition = self.session_transition.lock().await;
        self.cancel_sign_in();
        self.advance_generation();
        let session: Option<StoredSession> = match self.sessions.load().await {
            Ok(session) => session,
            // A malformed legacy/partial value must never make local sign-out
            // impossible. Deleting the keychain item is the recovery path.
            Err(AccountError::CorruptStoredSession) => None,
            // A read failure must not prevent a direct deletion attempt.
            Err(AccountError::KeychainUnavailable) => None,
            Err(error) => return Err(error),
        };
        self.sessions.delete().await?;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service(secrets: unfour_secret_store::SecretStore) -> AccountService {
        AccountService::with_secret_store(
            "https://api.example.test",
            "https://example.test",
            false,
            secrets,
        )
        .expect("valid service")
    }

    fn installation_id(url: &Url) -> String {
        let values: Vec<_> = url
            .query_pairs()
            .filter(|(key, _)| key == "installationId")
            .map(|(_, value)| value.into_owned())
            .collect();
        assert_eq!(values.len(), 1);
        values.into_iter().next().expect("installation id")
    }

    #[tokio::test]
    async fn stale_invalid_response_cannot_delete_a_newer_session() {
        let service = test_service(unfour_secret_store::SecretStore::in_memory("unfour-test"));
        let old = StoredSession {
            session_token: "A".repeat(43),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::days(1),
        };
        let current = StoredSession {
            session_token: "B".repeat(43),
            expires_at: old.expires_at,
        };
        service.sessions.save(old.clone()).await.unwrap();
        service.sessions.save(current.clone()).await.unwrap();

        let result = service
            .finish_billing_request(
                &old,
                Err(AccountError::ApiRejected {
                    status: 401,
                    code: ApiErrorCode::DesktopSessionExpired,
                }),
            )
            .await;

        assert!(matches!(
            result,
            Err(AccountError::ApiRejected {
                status: 401,
                code: ApiErrorCode::DesktopSessionExpired,
            })
        ));
        assert_eq!(
            service
                .sessions
                .load()
                .await
                .unwrap()
                .unwrap()
                .session_token,
            current.session_token
        );
        assert_eq!(service.generation(), 0);
    }

    #[test]
    fn begin_sign_in_keeps_secrets_out_of_the_public_url() {
        let service = test_service(unfour_secret_store::SecretStore::in_memory("unfour-test"));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime");
        let url = runtime
            .block_on(service.begin_sign_in())
            .expect("begin sign in");
        let pending = service.pending.lock().expect("pending state");
        let pending = pending.as_ref().expect("pending authorization");
        assert!(url.as_str().contains("codeChallenge="));
        assert!(!url.as_str().contains(&pending.code_verifier));
        assert!(!url.as_str().contains("codeVerifier"));
        for token in ["sessionToken", "accessToken", "refreshToken", "supabase"] {
            assert!(!url.as_str().contains(token));
        }
    }

    #[test]
    fn installation_id_is_stable_across_sign_ins_restart_and_sign_out() {
        let secrets = unfour_secret_store::SecretStore::in_memory("unfour-test");
        let first_service = test_service(secrets.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime");

        runtime.block_on(async {
            let first =
                installation_id(&first_service.begin_sign_in().await.expect("first sign in"));
            first_service.cancel_sign_in();
            let second =
                installation_id(&first_service.begin_sign_in().await.expect("second sign in"));
            assert_eq!(second, first);
            assert!(!first.is_empty());
            assert!(first.len() <= 128);

            first_service.sign_out().await.expect("sign out");
            let after_sign_out = installation_id(
                &first_service
                    .begin_sign_in()
                    .await
                    .expect("sign in after sign out"),
            );
            assert_eq!(after_sign_out, first);

            let restarted_service = test_service(secrets);
            let after_restart = installation_id(
                &restarted_service
                    .begin_sign_in()
                    .await
                    .expect("sign in after restart"),
            );
            assert_eq!(after_restart, first);
        });
    }

    #[test]
    fn error_codes_are_stable_and_do_not_include_sensitive_values() {
        assert_eq!(AccountError::StateMismatch.code(), "state_mismatch");
        assert_eq!(
            AccountError::InstallationIdUnavailable.code(),
            "installation_id_unavailable"
        );
        assert_eq!(
            AccountError::CorruptInstallationId.code(),
            "corrupt_installation_id"
        );
        assert_eq!(
            AccountError::ApiRejected {
                status: 400,
                code: ApiErrorCode::AuthorizationCodeExpired,
            }
            .code(),
            "authorization_code_expired"
        );
    }

    #[tokio::test]
    async fn local_sign_out_deletes_session_and_advances_generation_before_remote_work() {
        let secrets = unfour_secret_store::SecretStore::in_memory("unfour-test-sign-out");
        let service = test_service(secrets);
        service
            .sessions
            .save(StoredSession {
                session_token: "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde".into(),
                expires_at: time::OffsetDateTime::now_utc() + time::Duration::days(30),
            })
            .await
            .expect("seed session");
        let generation = service.generation();

        let session = service
            .clear_local_session_for_sign_out()
            .await
            .expect("clear local session")
            .expect("loaded session for best-effort revoke");

        assert_eq!(service.generation(), generation + 1);
        assert!(service
            .sessions
            .load()
            .await
            .expect("load after clear")
            .is_none());
        assert_eq!(session.session_token.len(), 43);
    }
}
