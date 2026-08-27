use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

use crate::AccountError;

pub const AUTH_CALLBACK_URI: &str = "unfour://auth/callback";
const AUTH_CALLBACK_SCHEME: &str = "unfour";
const AUTH_CALLBACK_HOST: &str = "auth";
const AUTH_CALLBACK_PATH: &str = "/callback";
const AUTHORIZATION_CODE_LENGTH: usize = 43;
const MAX_AUTH_ERROR_LENGTH: usize = 128;
const MAX_AVATAR_URL_LENGTH: usize = 2048;
const MAX_BILLING_URL_LENGTH: usize = 4096;
const MAX_DISPLAY_NAME_LENGTH: usize = 200;
const MAX_USERNAME_LENGTH: usize = 100;
const PKCE_STATE_LENGTH: usize = 43;
const SESSION_TOKEN_LENGTH: usize = 43;

/// Stable error codes defined by the account API OpenAPI contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    RequestError,
    InternalError,
    NotReady,
    InvalidState,
    PkceMismatch,
    AuthorizationCodeExpired,
    AuthorizationCodeUsed,
    DesktopSessionExpired,
    RateLimited,
    InvalidPlan,
    BillingUnavailable,
    BillingAlreadyActive,
    BillingCustomerNotFound,
    CheckoutCreationFailed,
    PortalCreationFailed,
}

impl ApiErrorCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::RequestError => "request_error",
            Self::InternalError => "internal_error",
            Self::NotReady => "not_ready",
            Self::InvalidState => "invalid_state",
            Self::PkceMismatch => "pkce_mismatch",
            Self::AuthorizationCodeExpired => "authorization_code_expired",
            Self::AuthorizationCodeUsed => "authorization_code_used",
            Self::DesktopSessionExpired => "desktop_session_expired",
            Self::RateLimited => "rate_limited",
            Self::InvalidPlan => "invalid_plan",
            Self::BillingUnavailable => "billing_unavailable",
            Self::BillingAlreadyActive => "billing_already_active",
            Self::BillingCustomerNotFound => "billing_customer_not_found",
            Self::CheckoutCreationFailed => "checkout_creation_failed",
            Self::PortalCreationFailed => "portal_creation_failed",
        }
    }
}

impl std::fmt::Display for ApiErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntitlementStatus {
    Active,
    Expired,
    Revoked,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementSummary {
    pub code: String,
    pub status: EntitlementStatus,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub valid_until: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSummary {
    pub id: String,
    pub name: String,
    pub platform: String,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub last_seen_at: Option<OffsetDateTime>,
    pub revoked: bool,
}

/// The only account data that may cross the Tauri IPC boundary.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub id: String,
    pub email: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub entitlements: Vec<EntitlementSummary>,
    pub devices: Vec<DeviceSummary>,
}

impl AccountSummary {
    pub(crate) fn validate(&self) -> Result<(), AccountError> {
        if self.id.trim().is_empty()
            || self.email.trim().is_empty()
            || !is_valid_optional_display_value(&self.username, MAX_USERNAME_LENGTH)
            || !is_valid_optional_display_value(&self.display_name, MAX_DISPLAY_NAME_LENGTH)
            || !is_valid_avatar_url(&self.avatar_url)
            || self
                .entitlements
                .iter()
                .any(|entitlement| entitlement.code.trim().is_empty())
            || self.devices.iter().any(|device| {
                device.id.trim().is_empty()
                    || device.name.trim().is_empty()
                    || device.platform.trim().is_empty()
            })
        {
            return Err(AccountError::InvalidApiResponse);
        }
        Ok(())
    }

    pub fn has_active_entitlement(&self, code: &str, now: OffsetDateTime) -> bool {
        self.entitlements.iter().any(|entitlement| {
            entitlement.code == code
                && entitlement.status == EntitlementStatus::Active
                && entitlement
                    .valid_until
                    .is_none_or(|valid_until| valid_until > now)
        })
    }
}

/// A billing destination accepted from an account API response.
///
/// This type is intentionally not serializable, so the frontend cannot supply
/// or round-trip a URL for the privileged browser-opening commands.
pub struct BillingUrl(Url);

impl BillingUrl {
    pub fn from_api_response(raw: &str, desktop_session_token: &str) -> Result<Self, AccountError> {
        if raw.is_empty()
            || raw.len() > MAX_BILLING_URL_LENGTH
            || !raw.starts_with("https://")
            || raw["https://".len()..].starts_with('/')
            || raw.trim() != raw
            || raw.chars().any(char::is_control)
            || raw.contains(desktop_session_token)
        {
            return Err(AccountError::InvalidBillingUrl);
        }
        let url = Url::parse(raw).map_err(|_| AccountError::InvalidBillingUrl)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.as_str().contains(desktop_session_token)
        {
            return Err(AccountError::InvalidBillingUrl);
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for BillingUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BillingUrl([REDACTED])")
    }
}

fn is_valid_optional_display_value(value: &Option<String>, max_length: usize) -> bool {
    value
        .as_ref()
        .is_none_or(|value| !value.trim().is_empty() && value.chars().count() <= max_length)
}

fn is_valid_avatar_url(value: &Option<String>) -> bool {
    value.as_ref().is_none_or(|value| {
        if value.len() > MAX_AVATAR_URL_LENGTH || value.trim() != value {
            return false;
        }
        Url::parse(value).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        })
    })
}

/// Frontend-visible account state. Session credentials are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AccountState {
    SignedOut,
    SigningIn,
    SignedIn { profile: AccountSummary },
}

impl AccountState {
    pub fn has_active_entitlement(&self, code: &str) -> bool {
        match self {
            Self::SignedIn { profile } => {
                profile.has_active_entitlement(code, OffsetDateTime::now_utc())
            }
            Self::SignedOut | Self::SigningIn => false,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopSession {
    pub session_token: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    pub account: AccountSummary,
}

impl DesktopSession {
    pub(crate) fn validate(&self) -> Result<(), AccountError> {
        validate_session_token(&self.session_token)?;
        if self.expires_at <= OffsetDateTime::now_utc() {
            return Err(AccountError::InvalidApiResponse);
        }
        self.account.validate()
    }

    pub(crate) fn stored_session(&self) -> StoredSession {
        StoredSession {
            session_token: self.session_token.clone(),
            expires_at: self.expires_at,
        }
    }
}

/// The complete credential-store payload. Account data is fetched from `/v1/me`.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredSession {
    pub session_token: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

impl StoredSession {
    pub(crate) fn validate(&self) -> Result<(), AccountError> {
        validate_session_token(&self.session_token)
    }

    pub(crate) fn is_expired(&self) -> bool {
        self.expires_at <= OffsetDateTime::now_utc()
    }
}

fn validate_session_token(session_token: &str) -> Result<(), AccountError> {
    if !is_url_safe_value(session_token, SESSION_TOKEN_LENGTH) {
        return Err(AccountError::InvalidApiResponse);
    }
    Ok(())
}

pub(crate) enum AuthCallback {
    Code {
        authorization_code: String,
        state: String,
    },
    Denied {
        state: String,
    },
}

impl AuthCallback {
    pub(crate) fn parse(raw: &str) -> Result<Self, AccountError> {
        let url = Url::parse(raw).map_err(|_| AccountError::InvalidDeepLink("invalid URL"))?;

        if url.scheme() != AUTH_CALLBACK_SCHEME
            || url.host_str() != Some(AUTH_CALLBACK_HOST)
            || url.path() != AUTH_CALLBACK_PATH
        {
            return Err(AccountError::InvalidDeepLink("unexpected callback route"));
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.fragment().is_some()
        {
            return Err(AccountError::InvalidDeepLink(
                "callback contains forbidden URL components",
            ));
        }

        let mut authorization_code = None;
        let mut state = None;
        let mut error = None;
        for (key, value) in url.query_pairs() {
            let slot = match key.as_ref() {
                "authorizationCode" => &mut authorization_code,
                "state" => &mut state,
                "error" => &mut error,
                _ => {
                    return Err(AccountError::InvalidDeepLink(
                        "callback contains an unknown query parameter",
                    ))
                }
            };
            if slot.replace(value.into_owned()).is_some() {
                return Err(AccountError::InvalidDeepLink(
                    "callback contains a duplicate query parameter",
                ));
            }
        }

        let state = state
            .filter(|value| is_url_safe_value(value, PKCE_STATE_LENGTH))
            .ok_or(AccountError::InvalidDeepLink("invalid state"))?;

        match (authorization_code, error) {
            (Some(authorization_code), None)
                if is_url_safe_value(&authorization_code, AUTHORIZATION_CODE_LENGTH) =>
            {
                Ok(Self::Code {
                    authorization_code,
                    state,
                })
            }
            (None, Some(error)) if is_bounded_value(&error, MAX_AUTH_ERROR_LENGTH) => {
                Ok(Self::Denied { state })
            }
            _ => Err(AccountError::InvalidDeepLink(
                "callback must contain exactly one result",
            )),
        }
    }

    pub(crate) fn state(&self) -> &str {
        match self {
            Self::Code { state, .. } | Self::Denied { state } => state,
        }
    }
}

fn is_bounded_value(value: &str, max_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_length
        && !value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
}

fn is_url_safe_value(value: &str, exact_length: usize) -> bool {
    value.len() == exact_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
