use std::future::Future;
use std::time::Duration;

use reqwest::{header::ACCEPT, Response, StatusCode};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::pkce::PendingAuthorization;
use crate::types::{AccountSummary, ApiErrorCode, BillingUrl, DesktopSession};
use crate::AccountError;

const LOGIN_PATH: &str = "/login";
const EXCHANGE_PATH: &str = "/v1/desktop/token";
const ACCOUNT_PATH: &str = "/v1/me";
const SIGN_OUT_PATH: &str = "/v1/desktop/session";
const BILLING_CHECKOUT_PATH: &str = "/v1/billing/checkout";
const BILLING_PORTAL_PATH: &str = "/v1/billing/portal";
const PRO_MONTHLY_PLAN: &str = "pro_monthly";
const DESKTOP_SESSION_HEADER: &str = "X-Desktop-Session";
const DESKTOP_CLIENT: &str = "desktop";
const DEVICE_NAME: &str = "Unfour Desktop";
const DEVICE_PLATFORM: &str = std::env::consts::OS;
/// Delays between retries after a transient failure (1 initial attempt + these waits).
/// Total wait budget is ~10s before the final attempt, covering short API cold starts.
const TRANSIENT_RETRY_DELAYS_SECS: &[u64] = &[1, 2, 3, 4];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopTokenExchangeRequest {
    authorization_code: String,
    state: String,
    code_verifier: String,
}

#[derive(Serialize)]
struct BillingCheckoutRequest {
    plan: &'static str,
}

#[derive(Deserialize)]
struct BillingCheckoutResponse {
    checkout_url: String,
}

#[derive(Deserialize)]
struct BillingPortalResponse {
    portal_url: String,
}

#[derive(Clone)]
pub(crate) struct AccountClient {
    http: reqwest::Client,
    api_base_url: Url,
    web_base_url: Url,
}

impl AccountClient {
    pub(crate) fn new(
        api_base_url: &str,
        web_base_url: &str,
        allow_loopback_http: bool,
    ) -> Result<Self, AccountError> {
        let api_base_url = parse_https_base_url(api_base_url, allow_loopback_http)?;
        let web_base_url = parse_https_base_url(web_base_url, allow_loopback_http)?;
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AccountError::InvalidConfiguration)?;
        Ok(Self {
            http,
            api_base_url,
            web_base_url,
        })
    }

    pub(crate) fn authorization_url(
        &self,
        pending: &PendingAuthorization,
        installation_id: &str,
    ) -> Result<Url, AccountError> {
        let mut url = self
            .web_base_url
            .join(LOGIN_PATH)
            .map_err(|_| AccountError::InvalidConfiguration)?;
        url.query_pairs_mut()
            .append_pair("client", DESKTOP_CLIENT)
            .append_pair("state", &pending.state)
            .append_pair("codeChallenge", &pending.code_challenge)
            .append_pair("codeChallengeMethod", "S256")
            .append_pair("deviceName", DEVICE_NAME)
            .append_pair("devicePlatform", DEVICE_PLATFORM)
            .append_pair("installationId", installation_id);
        Ok(url)
    }

    pub(crate) async fn exchange_code(
        &self,
        authorization_code: String,
        state: String,
        code_verifier: String,
    ) -> Result<DesktopSession, AccountError> {
        let endpoint = self.api_endpoint(EXCHANGE_PATH)?;
        with_transient_retry(|| {
            let http = self.http.clone();
            let endpoint = endpoint.clone();
            let authorization_code = authorization_code.clone();
            let state = state.clone();
            let code_verifier = code_verifier.clone();
            async move {
                let response = http
                    .post(endpoint)
                    .header(ACCEPT, "application/json")
                    .json(&DesktopTokenExchangeRequest {
                        authorization_code,
                        state,
                        code_verifier,
                    })
                    .send()
                    .await
                    .map_err(|_| AccountError::ApiUnavailable)?;
                let response = require_status(response, StatusCode::OK).await?;
                let session = response
                    .json::<DesktopSession>()
                    .await
                    .map_err(|_| AccountError::InvalidApiResponse)?;
                session.validate()?;
                Ok(session)
            }
        })
        .await
    }

    pub(crate) async fn get_account(
        &self,
        session_token: &str,
    ) -> Result<AccountSummary, AccountError> {
        let endpoint = self.api_endpoint(ACCOUNT_PATH)?;
        with_transient_retry(|| {
            let http = self.http.clone();
            let endpoint = endpoint.clone();
            let session_token = session_token.to_owned();
            async move {
                let response = http
                    .get(endpoint)
                    .header(ACCEPT, "application/json")
                    .header(DESKTOP_SESSION_HEADER, session_token)
                    .send()
                    .await
                    .map_err(|_| AccountError::ApiUnavailable)?;
                let response = require_status(response, StatusCode::OK).await?;
                let account = response
                    .json::<AccountSummary>()
                    .await
                    .map_err(|_| AccountError::InvalidApiResponse)?;
                account.validate()?;
                Ok(account)
            }
        })
        .await
    }

    pub(crate) async fn revoke_session(&self, session_token: &str) -> Result<(), AccountError> {
        let endpoint = self.api_endpoint(SIGN_OUT_PATH)?;
        let response = self
            .http
            .delete(endpoint)
            .header(ACCEPT, "application/json")
            .header(DESKTOP_SESSION_HEADER, session_token)
            .send()
            .await
            .map_err(|_| AccountError::ApiUnavailable)?;
        require_status(response, StatusCode::NO_CONTENT).await?;
        Ok(())
    }

    pub(crate) async fn create_billing_checkout(
        &self,
        session_token: &str,
    ) -> Result<BillingUrl, AccountError> {
        let response = self
            .http
            .post(self.api_endpoint(BILLING_CHECKOUT_PATH)?)
            .header(ACCEPT, "application/json")
            .header(DESKTOP_SESSION_HEADER, session_token)
            .json(&BillingCheckoutRequest {
                plan: PRO_MONTHLY_PLAN,
            })
            .send()
            .await
            .map_err(|_| AccountError::ApiUnavailable)?;
        let response = require_status(response, StatusCode::OK).await?;
        let response = response
            .json::<BillingCheckoutResponse>()
            .await
            .map_err(|_| AccountError::InvalidApiResponse)?;
        BillingUrl::from_api_response(&response.checkout_url, session_token)
    }

    pub(crate) async fn create_billing_portal(
        &self,
        session_token: &str,
    ) -> Result<BillingUrl, AccountError> {
        let response = self
            .http
            .post(self.api_endpoint(BILLING_PORTAL_PATH)?)
            .header(ACCEPT, "application/json")
            .header(DESKTOP_SESSION_HEADER, session_token)
            .send()
            .await
            .map_err(|_| AccountError::ApiUnavailable)?;
        let response = require_status(response, StatusCode::OK).await?;
        let response = response
            .json::<BillingPortalResponse>()
            .await
            .map_err(|_| AccountError::InvalidApiResponse)?;
        BillingUrl::from_api_response(&response.portal_url, session_token)
    }

    fn api_endpoint(&self, path: &str) -> Result<Url, AccountError> {
        self.api_base_url
            .join(path)
            .map_err(|_| AccountError::InvalidConfiguration)
    }
}

fn parse_https_base_url(raw: &str, allow_loopback_http: bool) -> Result<Url, AccountError> {
    let url = Url::parse(raw).map_err(|_| AccountError::InvalidConfiguration)?;
    // Production builds must use `https://` only. Debug builds additionally
    // allow `http://` on loopback hosts so local account-service debugging
    // works without a self-signed TLS certificate. A `-dev` release version
    // (signalled by `allow_loopback_http`) is likewise allowed to target
    // loopback `http://`, so a `tauri build` of `X.Y.Z-dev` can use a local
    // backend without a TLS cert. This keeps a *signed* release (stable or
    // `-test.N`) unable to be redirected to a plaintext endpoint.
    let scheme_ok = if cfg!(debug_assertions) || allow_loopback_http {
        url.scheme() == "https" || (url.scheme() == "http" && is_loopback_host(&url))
    } else {
        url.scheme() == "https"
    };
    if !scheme_ok
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AccountError::InvalidConfiguration);
    }
    Ok(url)
}

/// Returns true only for loopback hosts (`localhost`, `127.0.0.1`, `::1`).
///
/// Always compiled (referenced by `parse_https_base_url` in all profiles) but
/// only ever invoked at runtime when `cfg!(debug_assertions)` is true, so it
/// has no effect on a signed release.
fn is_loopback_host(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain("localhost")) => true,
        Some(Host::Ipv4(addr)) => addr.is_loopback(),
        Some(Host::Ipv6(addr)) => addr.is_loopback(),
        _ => false,
    }
}

#[derive(Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDetail {
    code: ApiErrorCode,
    message: String,
    request_id: String,
}

fn is_gateway_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
    )
}

/// Transport and readiness failures that are safe to retry during API cold start.
fn is_transient(error: &AccountError) -> bool {
    match error {
        AccountError::ApiUnavailable => true,
        AccountError::ApiRejected { status, code } => {
            *code == ApiErrorCode::NotReady || matches!(*status, 502 | 503 | 504)
        }
        _ => false,
    }
}

async fn with_transient_retry<T, F, Fut>(mut operation: F) -> Result<T, AccountError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, AccountError>>,
{
    let mut attempt = 0usize;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) if is_transient(&error) && attempt < TRANSIENT_RETRY_DELAYS_SECS.len() => {
                tokio::time::sleep(Duration::from_secs(TRANSIENT_RETRY_DELAYS_SECS[attempt])).await;
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn require_status(
    response: Response,
    expected: StatusCode,
) -> Result<Response, AccountError> {
    let status = response.status();
    if status == expected {
        return Ok(response);
    }
    if status.is_success() {
        return Err(AccountError::InvalidApiResponse);
    }
    // Gateway HTML/empty bodies must not become non-retryable InvalidApiResponse.
    if is_gateway_status(status) {
        let _ = response.bytes().await;
        return Err(AccountError::ApiUnavailable);
    }

    let body = response
        .bytes()
        .await
        .map_err(|_| AccountError::InvalidApiResponse)?;
    Err(parse_api_rejection(status, &body)?)
}

fn parse_api_rejection(status: StatusCode, body: &[u8]) -> Result<AccountError, AccountError> {
    let response = serde_json::from_slice::<ApiErrorEnvelope>(body)
        .map_err(|_| AccountError::InvalidApiResponse)?;
    let _ = (&response.error.message, &response.error.request_id);
    Ok(AccountError::ApiRejected {
        status: status.as_u16(),
        code: response.error.code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_url_matches_the_web_login_contract() {
        let client = AccountClient::new("https://api.example.test", "https://example.test", false)
            .expect("valid configuration");
        let pending = PendingAuthorization::generate();
        let url = client
            .authorization_url(&pending, "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdef")
            .expect("authorization URL");
        let pairs: Vec<_> = url.query_pairs().into_owned().collect();
        let query: std::collections::HashMap<_, _> = pairs.iter().cloned().collect();

        assert_eq!(
            url.as_str().split('?').next(),
            Some("https://example.test/login")
        );
        assert_eq!(query.get("client").map(String::as_str), Some("desktop"));
        assert_eq!(query.get("state"), Some(&pending.state));
        assert_eq!(query.get("codeChallenge"), Some(&pending.code_challenge));
        assert_eq!(
            query.get("codeChallengeMethod").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            query.get("deviceName").map(String::as_str),
            Some(DEVICE_NAME)
        );
        assert_eq!(
            query.get("devicePlatform").map(String::as_str),
            Some(DEVICE_PLATFORM)
        );
        assert_eq!(
            query.get("installationId").map(String::as_str),
            Some("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdef")
        );
        assert_eq!(
            pairs
                .iter()
                .filter(|(key, _)| key == "installationId")
                .count(),
            1
        );
        assert_eq!(pairs.len(), 7);
        assert!(!query.contains_key("redirect_uri"));
        assert!(!query.contains_key("response_type"));
        assert!(!url.as_str().contains(&pending.code_verifier));
        for forbidden in [
            "codeVerifier",
            "sessionToken",
            "accessToken",
            "refreshToken",
            "supabase",
        ] {
            assert!(!query.contains_key(forbidden));
        }
    }

    #[test]
    fn parses_the_openapi_error_envelope_and_stable_code() {
        let error = parse_api_rejection(
            StatusCode::BAD_REQUEST,
            br#"{"error":{"code":"pkce_mismatch","message":"PKCE mismatch","requestId":"request-1"}}"#,
        )
        .expect("OpenAPI error response");
        assert!(matches!(
            error,
            AccountError::ApiRejected {
                status: 400,
                code: ApiErrorCode::PkceMismatch
            }
        ));
        assert_eq!(error.code(), "pkce_mismatch");
    }

    #[test]
    fn token_exchange_payload_matches_openapi() {
        let value = serde_json::to_value(DesktopTokenExchangeRequest {
            authorization_code: "authorization-code".to_string(),
            state: "authorization-state".to_string(),
            code_verifier: "code-verifier".to_string(),
        })
        .expect("serialize token exchange request");
        assert_eq!(value["authorizationCode"], "authorization-code");
        assert_eq!(value["state"], "authorization-state");
        assert_eq!(value["codeVerifier"], "code-verifier");
        assert_eq!(value.as_object().expect("request object").len(), 3);
        assert!(value.get("code").is_none());
        assert!(value.get("redirectUri").is_none());
    }

    #[test]
    fn rejects_non_https_or_credentialed_build_configuration() {
        for api_url in [
            "http://api.example.test",
            "https://user@example.test",
            "https://api.example.test?override=1",
        ] {
            assert!(AccountClient::new(api_url, "https://example.test", false).is_err());
        }
        for web_url in [
            "http://example.test",
            "https://user:password@example.test",
            "https://example.test?path=/upgrade",
            "https://example.test/#override",
        ] {
            assert!(AccountClient::new("https://api.example.test", web_url, false).is_err());
        }
    }

    #[test]
    fn transient_errors_cover_cold_start_signals() {
        assert!(is_transient(&AccountError::ApiUnavailable));
        assert!(is_transient(&AccountError::ApiRejected {
            status: 503,
            code: ApiErrorCode::NotReady,
        }));
        assert!(is_transient(&AccountError::ApiRejected {
            status: 502,
            code: ApiErrorCode::InternalError,
        }));
        assert!(is_transient(&AccountError::ApiRejected {
            status: 504,
            code: ApiErrorCode::RequestError,
        }));
        assert!(is_gateway_status(StatusCode::BAD_GATEWAY));
        assert!(is_gateway_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_gateway_status(StatusCode::GATEWAY_TIMEOUT));
        assert!(!is_gateway_status(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn non_transient_errors_are_not_retried() {
        assert!(!is_transient(&AccountError::ApiRejected {
            status: 400,
            code: ApiErrorCode::PkceMismatch,
        }));
        assert!(!is_transient(&AccountError::ApiRejected {
            status: 401,
            code: ApiErrorCode::Unauthorized,
        }));
        assert!(!is_transient(&AccountError::InvalidApiResponse));
        assert!(!is_transient(&AccountError::InvalidConfiguration));
        assert!(!is_transient(&AccountError::AuthorizationDenied));
    }

    #[tokio::test]
    async fn with_transient_retry_retries_then_succeeds() {
        let mut attempts = 0u32;
        let value = with_transient_retry(|| {
            attempts += 1;
            let attempt = attempts;
            async move {
                if attempt < 3 {
                    Err(AccountError::ApiUnavailable)
                } else {
                    Ok(attempt)
                }
            }
        })
        .await
        .expect("eventual success");
        assert_eq!(value, 3);
        assert_eq!(attempts, 3);
    }

    #[tokio::test]
    async fn with_transient_retry_stops_on_non_transient_error() {
        let mut attempts = 0u32;
        let error = with_transient_retry(|| {
            attempts += 1;
            async move {
                Err::<(), _>(AccountError::ApiRejected {
                    status: 400,
                    code: ApiErrorCode::PkceMismatch,
                })
            }
        })
        .await
        .expect_err("non-transient failure");
        assert_eq!(attempts, 1);
        assert_eq!(error.code(), "pkce_mismatch");
    }
}
