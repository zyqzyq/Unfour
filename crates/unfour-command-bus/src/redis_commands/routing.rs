use super::*;
use unfour_ssh_engine::TemporarySshTunnel;

pub(crate) struct TemporaryRedisRoute {
    pub(super) workspace: String,
    pub(super) handle: String,
    pub(super) password: Option<Zeroizing<String>>,
    tunnel: TemporarySshTunnel,
}

enum RedisRoute {
    Saved { id: String, revision: Option<i64> },
    Temporary(TemporarySshTunnel),
}

/// Redis calls this connector inside its single network deadline. SSH setup,
/// the forwarded channel and every subsequent command share that same budget.
struct RedisJump {
    bus: CommandBus,
    workspace: String,
    jump: RedisRoute,
    saved: Option<(String, i64)>,
}

impl TunnelConnector for RedisJump {
    fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = AppResult<BoxedTunnelStream>> + Send + 'a>> {
        Box::pin(async move {
            let stream = match &self.jump {
                RedisRoute::Temporary(tunnel) => tunnel.connect(host, port).await?,
                RedisRoute::Saved { id, revision } => {
                    self.bus
                        .ssh
                        .open_tunnel_at_revision(&self.workspace, id, *revision)
                        .await
                        .map_err(|error| match error {
                            AppError::NotFound(_) => {
                                AppError::Validation("SSH_TUNNEL_ROUTE_MISSING".into())
                            }
                            other => other,
                        })?
                        .connect(host, port)
                        .await?
                }
            };
            if let Some((id, revision)) = &self.saved {
                self.bus
                    .check_redis_revision(&self.workspace, id, *revision)
                    .await?;
            }
            Ok(stream)
        })
    }
}

impl CommandBus {
    /// Scope the selection and Redis target password to this invocation.
    pub fn with_redis_temporary_route(
        &self,
        workspace: &str,
        handle: Option<&str>,
        password: Option<String>,
    ) -> AppResult<Self> {
        let mut bus = self.clone();
        bus.redis_temporary = match handle {
            Some(handle) => Some(Arc::new(TemporaryRedisRoute {
                workspace: workspace.to_owned(),
                handle: handle.to_owned(),
                tunnel: self.ssh.temporary_tunnel(workspace, handle)?,
                password: password.map(Zeroizing::new),
            })),
            None => None,
        };
        Ok(bus)
    }

    pub(super) fn redis_endpoint_via(
        &self,
        workspace: &str,
        mut endpoint: RedisEndpoint,
        jump: Option<&str>,
        expected_ssh_revision: Option<i64>,
        saved: Option<&RedisConnection>,
    ) -> RedisEndpoint {
        let jump = match (&self.redis_temporary, jump) {
            (Some(route), _) => RedisRoute::Temporary(route.tunnel.clone()),
            (None, Some(id)) => RedisRoute::Saved {
                id: id.to_owned(),
                revision: expected_ssh_revision,
            },
            (None, None) => return endpoint,
        };
        let connector = Arc::new(RedisJump {
            bus: self.clone(),
            workspace: workspace.to_owned(),
            jump,
            saved: saved.map(|record| (record.id.clone(), record.revision)),
        });
        if self.redis_temporary.is_some() {
            // A live override uses a fresh target password; a stale saved
            // jump affects only its original route. Revoked writes stay revoked.
            endpoint.transport = unfour_redis_engine::RedisTransport::Tunnel(connector);
            endpoint
        } else {
            endpoint.with_tunnel(connector)
        }
    }
}
