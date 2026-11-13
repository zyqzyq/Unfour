//! Request-scoped SSH direct TCP channels. Forward target names are resolved on the SSH host.
use super::*;
use std::{future::Future, pin::Pin};
use unfour_core::network::{BoxedTunnelStream, TunnelConnector};

mod proxy;
mod temporary;
pub use proxy::TunnelProxyLease;
pub use temporary::TemporarySshTunnel;

#[derive(Clone)]
pub struct SshTunnel {
    revision: i64,
    #[cfg(feature = "ssh-native")]
    client: Arc<russh::client::Handle<SshClientHandler>>,
}

fn failure(code: &'static str) -> AppError {
    AppError::Validation(code.into())
}

impl SshService {
    pub async fn open_tunnel(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> AppResult<SshTunnel> {
        self.open_tunnel_at_revision(workspace_id, connection_id, None)
            .await
    }

    pub async fn open_tunnel_at_revision(
        &self,
        workspace_id: &str,
        connection_id: &str,
        expected_revision: Option<i64>,
    ) -> AppResult<SshTunnel> {
        let connection = self.get_connection(workspace_id, connection_id).await?;
        if expected_revision.is_some_and(|expected| expected != connection.revision) {
            return Err(failure("SSH_TUNNEL_ROUTE_CHANGED"));
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = connection;
            Err(failure("SSH_TUNNEL_UNAVAILABLE"))
        }
        #[cfg(feature = "ssh-native")]
        {
            use native_transport::NativeConnectError;
            use std::time::Duration;
            let pinned = HostKeyStore::new(self.db.pool().clone())
                .get_fingerprint(workspace_id, &connection.host, connection.port)
                .await?
                .ok_or_else(|| failure("SSH_TUNNEL_HOST_KEY_UNTRUSTED"))?;
            validate_connection_ready_for_session(&connection)
                .map_err(|_| failure("SSH_TUNNEL_AUTH_FAILED"))?;
            let mut handle = self
                .dial_saved_host(&connection, Some(pinned.clone()))
                .await
                .map_err(|e| match e {
                    NativeConnectError::TimedOut => failure("SSH_TUNNEL_TIMEOUT"),
                    NativeConnectError::Failed(russh::Error::UnknownKey) => {
                        failure("SSH_TUNNEL_HOST_KEY_CHANGED")
                    }
                    NativeConnectError::Failed(_) => failure("SSH_TUNNEL_CONNECT_FAILED"),
                })?;
            self.check_tunnel_target(&connection, &pinned).await?;
            tokio::time::timeout(
                Duration::from_secs(30),
                self.authenticate_native(&mut handle, &connection, None),
            )
            .await
            .map_err(|_| failure("SSH_TUNNEL_TIMEOUT"))?
            .map_err(|_| failure("SSH_TUNNEL_AUTH_FAILED"))?;
            self.check_tunnel_target(&connection, &pinned).await?;
            Ok(SshTunnel {
                revision: connection.revision,
                client: Arc::new(handle),
            })
        }
    }

    #[cfg(feature = "ssh-native")]
    async fn check_tunnel_target(&self, target: &SshConnection, pin: &str) -> AppResult<()> {
        // User input, DNS, SSH negotiation and keychain prompts can all outlive
        // a profile edit or a host-key reset. Recheck before auth and handoff.
        let current: Option<(i64, Option<String>)> = sqlx::query_as(
            "SELECT c.revision,k.fingerprint FROM connections c LEFT JOIN ssh_host_keys k
             ON k.workspace_id=c.workspace_id AND k.host=c.host AND k.port=c.port
             WHERE c.workspace_id=? AND c.id=? AND c.connection_type='ssh' AND c.deleted_at IS NULL",
        )
        .bind(&target.workspace_id)
        .bind(&target.id)
        .fetch_optional(self.db.pool())
        .await?;
        match current {
            None => Err(failure("SSH_TUNNEL_ROUTE_MISSING")),
            Some((revision, _)) if revision != target.revision => {
                Err(failure("SSH_TUNNEL_ROUTE_CHANGED"))
            }
            Some((_, None)) => Err(failure("SSH_TUNNEL_HOST_KEY_UNTRUSTED")),
            Some((_, Some(current))) if current != pin => {
                Err(failure("SSH_TUNNEL_HOST_KEY_CHANGED"))
            }
            Some(_) => Ok(()),
        }
    }
}

impl SshTunnel {
    pub fn connection_revision(&self) -> i64 {
        self.revision
    }

    pub async fn connect(&self, host: &str, port: u16) -> AppResult<BoxedTunnelStream> {
        let host = target_host(host, port)?;
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = host;
            Err(failure("SSH_TUNNEL_UNAVAILABLE"))
        }
        #[cfg(feature = "ssh-native")]
        open_direct_channel(&self.client, host, port).await
    }

    pub async fn http_proxy(&self) -> AppResult<TunnelProxyLease> {
        TunnelProxyLease::start(Arc::new(self.clone()), None).await
    }
}

fn target_host(host: &str, port: u16) -> AppResult<&str> {
    if host.is_empty()
        || host.len() > 255
        || port == 0
        || host.bytes().any(|b| {
            b.is_ascii_control() || b.is_ascii_whitespace() || matches!(b, b'/' | b'\\' | b'@')
        })
    {
        return Err(failure("SSH_TUNNEL_INVALID_TARGET"));
    }
    Ok(host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host))
}

#[cfg(feature = "ssh-native")]
async fn open_direct_channel(
    client: &russh::client::Handle<SshClientHandler>,
    host: &str,
    port: u16,
) -> AppResult<BoxedTunnelStream> {
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        forward_stream(client, host, port),
    )
    .await
    .map_err(|_| failure("SSH_TUNNEL_TIMEOUT"))?
}

#[cfg(feature = "ssh-native")]
async fn forward_stream(
    client: &russh::client::Handle<SshClientHandler>,
    host: &str,
    port: u16,
) -> AppResult<BoxedTunnelStream> {
    // The forwarding host resolves the original destination name.
    let channel = client
        .channel_open_direct_tcpip(host, u32::from(port), "127.0.0.1", 0)
        .await
        .map_err(|_| failure("SSH_TUNNEL_TARGET_UNREACHABLE"))?;
    Ok(Box::new(channel.into_stream()))
}

impl TunnelConnector for SshTunnel {
    fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = AppResult<BoxedTunnelStream>> + Send + 'a>> {
        Box::pin(self.connect(host, port))
    }
}
