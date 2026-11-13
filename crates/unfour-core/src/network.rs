use crate::AppResult;
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};
use tokio::io::{AsyncRead, AsyncWrite};

pub trait TunnelIo: AsyncRead + AsyncWrite + Unpin + Send + Sync {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + Sync> TunnelIo for T {}
pub type BoxedTunnelStream = Box<dyn TunnelIo>;

/// Opens a TCP channel by destination name, leaving DNS to the selected SSH host.
pub trait TunnelConnector: Send + Sync {
    fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = AppResult<BoxedTunnelStream>> + Send + 'a>>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ApiRoute {
    Inherit,
    Direct,
    Ssh { ssh_connection_id: String },
}

/// Only an explicit edit from a loaded route can replace a stored policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiRouteEdit {
    pub expected: ApiRoute,
    pub route: ApiRoute,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRoutingInput {
    pub request_id: Option<String>,
    pub route: Option<ApiRoute>,
    pub expected_environment_id: Option<String>,
    pub expected_effective: ApiRoute,
    pub expected_ssh_revision: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRoutes {
    pub request: ApiRoute,
    pub environment: ApiRoute,
    pub effective: ApiRoute,
    pub environment_id: Option<String>,
}
