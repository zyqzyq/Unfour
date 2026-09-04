use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use url::Url;

use crate::{
    ApiErrorEnvelope, ChangesPage, CloudWorkspace, CreateCloudWorkspaceRequest, PushRequest,
    PushResponse, RemoteSyncProblem, RemoteSyncProblemCategory, SnapshotPage, SyncAccountContext,
    SyncConflictDetails, SyncError, SyncPhase, PROTOCOL_VERSION,
};

/// Opaque desktop-session credential. It is never serializable or clonable and
/// its Debug representation cannot expose the token.
pub struct DesktopSessionCredential {
    token: String,
    account_id: String,
    generation: u64,
}

impl DesktopSessionCredential {
    pub fn new(token: String, account_id: String, generation: u64) -> Result<Self, SyncError> {
        if token.trim().is_empty()
            || token.chars().any(char::is_whitespace)
            || account_id.trim().is_empty()
        {
            return Err(SyncError::Unauthorized);
        }
        Ok(Self {
            token,
            account_id,
            generation,
        })
    }

    fn expose_token(&self) -> &str {
        &self.token
    }

    fn context(&self) -> SyncAccountContext {
        SyncAccountContext {
            account_id: self.account_id.clone(),
            generation: self.generation,
        }
    }
}

impl fmt::Debug for DesktopSessionCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopSessionCredential")
            .field("token", &"[REDACTED]")
            .field("account_id", &self.account_id)
            .field("generation", &self.generation)
            .finish()
    }
}

#[async_trait]
pub trait DesktopSessionProvider: Send + Sync {
    async fn session_for_cloud_sync(&self) -> Result<DesktopSessionCredential, SyncError>;
    fn generation(&self) -> u64;
    fn invalidate_cloud_sync(&self, request_generation: u64, failure: CloudSyncAuthFailure);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudSyncAuthFailure {
    Unauthorized,
    EntitlementRequired,
}

#[derive(Clone)]
pub enum TransportError {
    Unauthorized,
    EntitlementRequired,
    ProtocolIncompatible,
    NotFound,
    Conflict(SyncConflictDetails),
    Permanent(String),
    PermanentOperation {
        code: String,
        operation_id: String,
    },
    InvalidResponse,
    Retryable,
    ResultUnknown,
    Remote(RemoteSyncProblem),
    RemoteConflict {
        problem: RemoteSyncProblem,
        details: SyncConflictDetails,
    },
}

impl fmt::Debug for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict(details) => formatter.debug_tuple("Conflict").field(details).finish(),
            Self::Permanent(code) => formatter.debug_tuple("Permanent").field(code).finish(),
            Self::PermanentOperation { code, operation_id } => formatter
                .debug_struct("PermanentOperation")
                .field("code", code)
                .field("operation_id", operation_id)
                .finish(),
            Self::Unauthorized => formatter.write_str("Unauthorized"),
            Self::EntitlementRequired => formatter.write_str("EntitlementRequired"),
            Self::ProtocolIncompatible => formatter.write_str("ProtocolIncompatible"),
            Self::NotFound => formatter.write_str("NotFound"),
            Self::InvalidResponse => formatter.write_str("InvalidResponse"),
            Self::Retryable => formatter.write_str("Retryable"),
            Self::ResultUnknown => formatter.write_str("ResultUnknown"),
            Self::Remote(problem) => formatter.debug_tuple("Remote").field(problem).finish(),
            Self::RemoteConflict { problem, details } => formatter
                .debug_struct("RemoteConflict")
                .field("problem", problem)
                .field("details", details)
                .finish(),
        }
    }
}

impl From<TransportError> for SyncError {
    fn from(value: TransportError) -> Self {
        match value {
            TransportError::Unauthorized => Self::Unauthorized,
            TransportError::EntitlementRequired => Self::EntitlementRequired,
            TransportError::ProtocolIncompatible => Self::ProtocolIncompatible,
            TransportError::NotFound => Self::NotFound,
            TransportError::Conflict(_) => Self::Conflict,
            TransportError::Permanent(_) => Self::Permanent,
            TransportError::PermanentOperation { .. } => Self::Permanent,
            TransportError::InvalidResponse => Self::InvalidData,
            TransportError::Retryable | TransportError::ResultUnknown => Self::Transport,
            TransportError::Remote(problem) => problem.sync_error(),
            TransportError::RemoteConflict { .. } => Self::Conflict,
        }
    }
}

#[async_trait]
pub trait SyncTransport: Send + Sync {
    async fn account_context(&self) -> Result<SyncAccountContext, TransportError>;
    fn account_generation(&self) -> u64;
    async fn list_workspaces(&self) -> Result<Vec<CloudWorkspace>, TransportError>;
    async fn create_workspace(
        &self,
        root_entity_id: &str,
    ) -> Result<CloudWorkspace, TransportError>;
    async fn push(
        &self,
        cloud_workspace_id: &str,
        request: &PushRequest,
    ) -> Result<PushResponse, TransportError>;
    async fn changes(
        &self,
        cloud_workspace_id: &str,
        after_cursor: i64,
        limit: usize,
    ) -> Result<ChangesPage, TransportError>;
    async fn snapshot(
        &self,
        cloud_workspace_id: &str,
        at_cursor: Option<i64>,
        page_token: Option<&str>,
    ) -> Result<SnapshotPage, TransportError>;
}

pub struct HttpSyncTransport {
    base_url: Url,
    client: reqwest::Client,
    sessions: std::sync::Arc<dyn DesktopSessionProvider>,
}

struct AuthenticatedRequest {
    builder: reqwest::RequestBuilder,
    generation: u64,
}

impl AuthenticatedRequest {
    fn json<T: serde::Serialize>(self, body: &T) -> Self {
        Self {
            builder: self.builder.json(body),
            generation: self.generation,
        }
    }

    async fn send(self) -> AuthenticatedResponse {
        AuthenticatedResponse {
            generation: self.generation,
            response: self.builder.send().await,
        }
    }
}

struct AuthenticatedResponse {
    generation: u64,
    response: Result<reqwest::Response, reqwest::Error>,
}

impl HttpSyncTransport {
    pub fn new(
        base_url: &str,
        sessions: std::sync::Arc<dyn DesktopSessionProvider>,
    ) -> Result<Self, SyncError> {
        let mut base_url = Url::parse(base_url).map_err(|_| SyncError::InvalidData)?;
        if base_url.cannot_be_a_base()
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || !matches!(base_url.scheme(), "https" | "http")
        {
            return Err(SyncError::InvalidData);
        }
        base_url.set_path("/");
        base_url.set_query(None);
        base_url.set_fragment(None);
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| SyncError::Transport)?;
        Ok(Self {
            base_url,
            client,
            sessions,
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, TransportError> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(|_| TransportError::InvalidResponse)
    }

    fn map_session_error(error: SyncError) -> TransportError {
        match error {
            SyncError::Unauthorized => TransportError::Unauthorized,
            SyncError::EntitlementRequired => TransportError::EntitlementRequired,
            SyncError::Transport => TransportError::Retryable,
            _ => TransportError::Unauthorized,
        }
    }

    fn invalidate_for_response(&self, request_generation: u64, failure: CloudSyncAuthFailure) {
        // Keep the check here as well as in the desktop provider. This avoids
        // invoking an invalidation side effect for an already stale response;
        // the provider repeats the fence to cover the check-to-call race.
        if self.sessions.generation() == request_generation {
            self.sessions
                .invalidate_cloud_sync(request_generation, failure);
        }
    }

    async fn request(
        &self,
        method: Method,
        url: Url,
    ) -> Result<AuthenticatedRequest, TransportError> {
        let credential = self
            .sessions
            .session_for_cloud_sync()
            .await
            .map_err(Self::map_session_error)?;
        Ok(AuthenticatedRequest {
            builder: self
                .client
                .request(method, url)
                .header("X-Desktop-Session", credential.expose_token()),
            generation: credential.generation,
        })
    }

    async fn decode<T: DeserializeOwned>(
        &self,
        response: AuthenticatedResponse,
        phase: SyncPhase,
    ) -> Result<T, TransportError> {
        let AuthenticatedResponse {
            generation,
            response,
        } = response;
        let response = response.map_err(|error| {
            let code = if error.is_timeout() {
                "request_timeout"
            } else if error.is_connect() {
                "connection_failed"
            } else {
                "request_failed"
            };
            TransportError::Remote(transport_failure_problem(phase, code, None))
        })?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|_| {
            TransportError::Remote(transport_failure_problem(
                phase,
                "response_body_interrupted",
                Some(status.as_u16()),
            ))
        })?;
        if status.is_success() {
            return serde_json::from_slice(&bytes).map_err(|_| {
                TransportError::Remote(RemoteSyncProblem {
                    server_error_code: "invalid_api_response".into(),
                    request_id: None,
                    http_status: Some(status.as_u16()),
                    phase,
                    operation_id: None,
                    operation_index: None,
                    entity_type: None,
                    entity_id: None,
                    category: RemoteSyncProblemCategory::InvalidResponse,
                })
            });
        }
        let error = serde_json::from_slice::<ApiErrorEnvelope>(&bytes)
            .ok()
            .map(|value| value.error);
        let classified = classify_api_error(status, phase, error);
        match &classified {
            TransportError::Remote(problem)
                if problem.category == RemoteSyncProblemCategory::Auth =>
            {
                self.invalidate_for_response(generation, CloudSyncAuthFailure::Unauthorized);
            }
            TransportError::Remote(problem)
                if problem.category == RemoteSyncProblemCategory::Entitlement =>
            {
                self.invalidate_for_response(generation, CloudSyncAuthFailure::EntitlementRequired);
            }
            _ => {}
        }
        Err(classified)
    }
}

fn transport_failure_problem(
    phase: SyncPhase,
    code: &str,
    http_status: Option<u16>,
) -> RemoteSyncProblem {
    RemoteSyncProblem {
        server_error_code: code.into(),
        request_id: None,
        http_status,
        phase,
        operation_id: None,
        operation_index: None,
        entity_type: None,
        entity_id: None,
        category: if phase == SyncPhase::Push {
            RemoteSyncProblemCategory::ResultUnknown
        } else {
            RemoteSyncProblemCategory::Retryable
        },
    }
}

fn classify_api_error(
    status: StatusCode,
    phase: SyncPhase,
    error: Option<crate::ApiErrorDetail>,
) -> TransportError {
    if error.is_none() {
        if status == StatusCode::UNAUTHORIZED {
            let mut problem = transport_failure_problem(phase, "unauthorized", Some(401));
            problem.category = RemoteSyncProblemCategory::Auth;
            return TransportError::Remote(problem);
        }
        if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
            let mut problem = transport_failure_problem(
                phase,
                if status == StatusCode::TOO_MANY_REQUESTS {
                    "rate_limited"
                } else {
                    "server_error"
                },
                Some(status.as_u16()),
            );
            problem.category = RemoteSyncProblemCategory::Retryable;
            return TransportError::Remote(problem);
        }
        let mut problem =
            transport_failure_problem(phase, "invalid_api_response", Some(status.as_u16()));
        problem.category = RemoteSyncProblemCategory::InvalidResponse;
        return TransportError::Remote(problem);
    }
    let code = error
        .as_ref()
        .map(|value| value.code.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let Some(code) = code else {
        return TransportError::Remote(RemoteSyncProblem {
            server_error_code: "invalid_api_response".into(),
            request_id: error
                .as_ref()
                .map(|value| value.request_id.trim())
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            http_status: Some(status.as_u16()),
            phase,
            operation_id: None,
            operation_index: None,
            entity_type: None,
            entity_id: None,
            category: RemoteSyncProblemCategory::InvalidResponse,
        });
    };
    let operation = error
        .as_ref()
        .and_then(|value| value.permanent_operation_details());
    let conflict_details = error.as_ref().and_then(|value| value.conflict_details());
    let problem = |category| RemoteSyncProblem {
        server_error_code: code.clone(),
        request_id: error
            .as_ref()
            .map(|value| value.request_id.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        http_status: Some(status.as_u16()),
        phase,
        operation_id: operation.as_ref().map(|value| value.operation_id.clone()),
        operation_index: operation.as_ref().and_then(|value| value.operation_index),
        entity_type: operation
            .as_ref()
            .and_then(|value| value.entity_type.clone()),
        entity_id: operation.as_ref().and_then(|value| value.entity_id.clone()),
        category,
    };
    if status == StatusCode::UNAUTHORIZED || code == "unauthorized" {
        return TransportError::Remote(problem(RemoteSyncProblemCategory::Auth));
    }
    if code == "entitlement_required" {
        return TransportError::Remote(problem(RemoteSyncProblemCategory::Entitlement));
    }
    if code == "protocol_version_unsupported" {
        return TransportError::Remote(problem(RemoteSyncProblemCategory::Protocol));
    }
    if code == "base_version_conflict" {
        return match conflict_details {
            Some(details) => TransportError::RemoteConflict {
                problem: problem(RemoteSyncProblemCategory::Conflict),
                details,
            },
            None => TransportError::Remote(problem(RemoteSyncProblemCategory::InvalidResponse)),
        };
    }
    if code == "snapshot_required" {
        return TransportError::Remote(problem(RemoteSyncProblemCategory::SnapshotRequired));
    }
    if status == StatusCode::NOT_FOUND
        || matches!(
            code.as_str(),
            "sync_workspace_deleted" | "sync_workspace_not_found" | "not_found"
        )
    {
        return TransportError::Remote(problem(RemoteSyncProblemCategory::Workspace));
    }
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        return TransportError::Remote(problem(RemoteSyncProblemCategory::Retryable));
    }
    let operation_code = operation
        .as_ref()
        .and_then(|details| details.error_code.as_deref())
        .unwrap_or(&code);
    if matches!(
        operation_code,
        "invalid_sync_entity"
            | "invalid_parent_entity"
            | "payload_schema_version_unsupported"
            | "operation_id_reuse"
            | "secret_value_not_allowed"
    ) || (operation.is_some()
        && matches!(operation_code, "request_too_large" | "payload_too_large"))
    {
        let mut operation_problem = problem(RemoteSyncProblemCategory::OperationPermanent);
        operation_problem.server_error_code = operation_code.to_string();
        return TransportError::Remote(operation_problem);
    }
    TransportError::Remote(problem(RemoteSyncProblemCategory::RequestPermanent))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceListResponse {
    protocol_version: u32,
    workspaces: Vec<CloudWorkspace>,
}

#[async_trait]
impl SyncTransport for HttpSyncTransport {
    async fn account_context(&self) -> Result<SyncAccountContext, TransportError> {
        self.sessions
            .session_for_cloud_sync()
            .await
            .map(|value| value.context())
            .map_err(Self::map_session_error)
    }

    fn account_generation(&self) -> u64 {
        self.sessions.generation()
    }

    async fn list_workspaces(&self) -> Result<Vec<CloudWorkspace>, TransportError> {
        let mut url = self.endpoint("v1/sync/workspaces")?;
        url.query_pairs_mut()
            .append_pair("protocolVersion", &PROTOCOL_VERSION.to_string());
        let response = self.request(Method::GET, url).await?.send().await;
        let body = self
            .decode::<WorkspaceListResponse>(response, SyncPhase::ListWorkspaces)
            .await?;
        if body.protocol_version != PROTOCOL_VERSION {
            return Err(TransportError::ProtocolIncompatible);
        }
        Ok(body.workspaces)
    }

    async fn create_workspace(
        &self,
        root_entity_id: &str,
    ) -> Result<CloudWorkspace, TransportError> {
        let url = self.endpoint("v1/sync/workspaces")?;
        let request = CreateCloudWorkspaceRequest {
            protocol_version: PROTOCOL_VERSION,
            root_entity_id: root_entity_id.to_string(),
        };
        let response = self
            .request(Method::POST, url)
            .await?
            .json(&request)
            .send()
            .await;
        self.decode(response, SyncPhase::CreateWorkspace).await
    }

    async fn push(
        &self,
        cloud_workspace_id: &str,
        request: &PushRequest,
    ) -> Result<PushResponse, TransportError> {
        let url = self.endpoint(&format!("v1/sync/workspaces/{cloud_workspace_id}/push"))?;
        let response = self
            .request(Method::POST, url)
            .await?
            .json(request)
            .send()
            .await;
        self.decode(response, SyncPhase::Push).await
    }

    async fn changes(
        &self,
        cloud_workspace_id: &str,
        after_cursor: i64,
        limit: usize,
    ) -> Result<ChangesPage, TransportError> {
        let mut url = self.endpoint(&format!("v1/sync/workspaces/{cloud_workspace_id}/changes"))?;
        url.query_pairs_mut()
            .append_pair("protocolVersion", &PROTOCOL_VERSION.to_string())
            .append_pair("afterCursor", &after_cursor.to_string())
            .append_pair("limit", &limit.to_string());
        let response = self.request(Method::GET, url).await?.send().await;
        self.decode(response, SyncPhase::Changes).await
    }

    async fn snapshot(
        &self,
        cloud_workspace_id: &str,
        at_cursor: Option<i64>,
        page_token: Option<&str>,
    ) -> Result<SnapshotPage, TransportError> {
        let mut url =
            self.endpoint(&format!("v1/sync/workspaces/{cloud_workspace_id}/snapshot"))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("protocolVersion", &PROTOCOL_VERSION.to_string());
            if let Some(at_cursor) = at_cursor {
                query.append_pair("atCursor", &at_cursor.to_string());
            }
            if let Some(page_token) = page_token {
                query.append_pair("pageToken", page_token);
            }
        }
        let response = self.request(Method::GET, url).await?.send().await;
        self.decode(response, SyncPhase::Snapshot).await
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
