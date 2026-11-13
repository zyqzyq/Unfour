use crate::{CommandBus, DatabaseConnection, LocalDb, SshService};
use std::{future::Future, pin::Pin, sync::Arc};
use unfour_core::{
    network::{BoxedTunnelStream, TunnelConnector},
    AppError, AppResult,
};
use unfour_database_engine::DatabaseTunnelResolver;

pub(super) struct SqlRoutes {
    ssh: SshService,
    db: LocalDb,
}

impl SqlRoutes {
    pub(super) fn new(ssh: SshService, db: LocalDb) -> Self {
        Self { ssh, db }
    }
}

impl CommandBus {
    pub fn with_database_route(
        &self,
        handle: Option<&str>,
        expected: Option<unfour_core::models::DatabaseConnectionVersion>,
    ) -> Self {
        let mut bus = self.clone();
        bus.database = self.database.with_connection_route(handle, expected);
        bus
    }
}

impl DatabaseTunnelResolver for SqlRoutes {
    fn resolve(
        &self,
        connection: &DatabaseConnection,
        temporary_handle: Option<&str>,
        draft_source: Option<&DatabaseConnection>,
    ) -> AppResult<Option<Arc<dyn TunnelConnector>>> {
        let source = if let Some(handle) = temporary_handle {
            SqlJump::Temporary(
                self.ssh
                    .temporary_tunnel(&connection.workspace_id, handle)?,
            )
        } else if let Some(id) = &connection.ssh_connection_id {
            SqlJump::Saved {
                ssh: self.ssh.clone(),
                id: id.clone(),
                revision: connection.ssh_revision,
            }
        } else {
            return Ok(None);
        };
        Ok(Some(Arc::new(SqlChannel {
            source,
            db: self.db.clone(),
            connection: draft_source.unwrap_or(connection).clone(),
        })))
    }
}

enum SqlJump {
    Saved {
        ssh: SshService,
        id: String,
        revision: Option<i64>,
    },
    Temporary(unfour_ssh_engine::TemporarySshTunnel),
}

struct SqlChannel {
    source: SqlJump,
    db: LocalDb,
    connection: DatabaseConnection,
}

impl TunnelConnector for SqlChannel {
    fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = AppResult<BoxedTunnelStream>> + Send + 'a>> {
        Box::pin(async move {
            let record = &self.connection;
            let stream = match &self.source {
                SqlJump::Temporary(tunnel) => tunnel.connect(host, port).await?,
                SqlJump::Saved { ssh, id, revision } => {
                    ssh.open_tunnel_at_revision(&record.workspace_id, id, *revision)
                        .await
                        .map_err(|err| match err {
                            AppError::NotFound(_) => {
                                AppError::Validation("SSH_TUNNEL_ROUTE_MISSING".into())
                            }
                            other => other,
                        })?
                        .connect(host, port)
                        .await?
                }
            };
            // A jump handshake can outlast edits in another window. Pin the
            // local route as well as the shared row before SQL sends its login.
            if record.revision > 0 {
                let current: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM connections c JOIN database_connections d ON c.id=d.connection_id WHERE c.workspace_id=? AND c.id=? AND c.deleted_at IS NULL AND c.revision=? AND c.credential_ref IS ? AND d.ssh_connection_id IS ? AND d.ssh_revision IS ?)")
                    .bind(&record.workspace_id).bind(&record.id).bind(record.revision).bind(&record.credential_ref)
                    .bind(&record.ssh_connection_id).bind(record.ssh_revision).fetch_one(self.db.pool()).await?;
                if !current {
                    return Err(AppError::Validation("SSH_TUNNEL_ROUTE_CHANGED".into()));
                }
            }
            Ok(stream)
        })
    }
}
