use crate::{error, protocol, RedisEndpoint, RedisScanPage, PREVIEW_BYTES};
use protocol::{io_error, Reply};
use rustls_platform_verifier::BuilderVerifierExt;
use std::{future::Future, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufStream},
    net::TcpStream,
    time::{timeout_at, Instant},
};
use tokio_rustls::{rustls, TlsConnector};
use unfour_core::{AppError, AppResult};

trait Socket: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Socket for T {}

pub(super) struct Session {
    socket: BufStream<Box<dyn Socket>>,
    deadline: Instant,
}

async fn bounded<T>(deadline: Instant, task: impl Future<Output = AppResult<T>>) -> AppResult<T> {
    timeout_at(deadline, task)
        .await
        .map_err(|_| error("REDIS_TIMEOUT"))?
}

impl Session {
    pub(super) async fn connect(
        endpoint: &RedisEndpoint,
        password: Option<&str>,
    ) -> AppResult<Self> {
        let host = endpoint.host.as_str();
        if host.is_empty()
            || host.len() > 255
            || endpoint.port == 0
            || host
                .bytes()
                .any(|c| c.is_ascii_whitespace() || matches!(c, b'/' | b'@' | b'\\'))
            || endpoint.username.as_ref().is_some_and(|u| u.len() > 1024)
            || password.is_some_and(|s| s.len() > 64 * 1024)
        {
            return Err(error("REDIS_INVALID_INPUT"));
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        let socket = bounded(deadline, async {
            let stream = TcpStream::connect((host, endpoint.port))
                .await
                .map_err(io_error)?;
            let socket: Box<dyn Socket> = if endpoint.tls {
                let name = rustls::pki_types::ServerName::try_from(host.to_owned())
                    .map_err(|_| error("REDIS_INVALID_INPUT"))?;
                let config = rustls::ClientConfig::builder_with_provider(Arc::new(
                    rustls::crypto::ring::default_provider(),
                ))
                .with_safe_default_protocol_versions()
                .and_then(|builder| builder.with_platform_verifier())
                .map_err(|_| error("REDIS_TLS_FAILED"))?
                .with_no_client_auth();
                Box::new(
                    TlsConnector::from(Arc::new(config))
                        .connect(name, stream)
                        .await
                        .map_err(|_| error("REDIS_TLS_FAILED"))?,
                )
            } else {
                Box::new(stream)
            };
            Ok(socket)
        })
        .await?;
        let mut session = Self {
            socket: BufStream::new(socket),
            deadline,
        };
        if let Some(user) = endpoint.username.as_deref().filter(|u| !u.is_empty()) {
            session
                .expect_ok(
                    &[
                        b"AUTH",
                        user.as_bytes(),
                        password.unwrap_or_default().as_bytes(),
                    ],
                    "REDIS_AUTH_FAILED",
                )
                .await?;
        } else if let Some(password) = password {
            session
                .expect_ok(&[b"AUTH", password.as_bytes()], "REDIS_AUTH_FAILED")
                .await?;
        }
        // A fresh connection starts in DB 0, including servers restricting SELECT via ACL.
        if endpoint.db_index != 0 {
            let index = endpoint.db_index.to_string();
            session
                .expect_ok(&[b"SELECT", index.as_bytes()], "REDIS_DB_UNAVAILABLE")
                .await?;
        }
        Ok(session)
    }

    async fn expect_ok(&mut self, arguments: &[&[u8]], fallback: &'static str) -> AppResult<()> {
        match self.command(arguments).await {
            Ok(Reply::Simple(value)) if value == b"OK" => Ok(()),
            Err(AppError::Validation(reason)) if reason == "REDIS_COMMAND_FAILED" => {
                Err(error(fallback))
            }
            Err(other) => Err(other),
            _ => Err(error("REDIS_PROTOCOL_ERROR")),
        }
    }

    async fn send(&mut self, args: &[&[u8]]) -> AppResult<()> {
        self.socket
            .write_all(format!("*{}\r\n", args.len()).as_bytes())
            .await
            .map_err(io_error)?;
        for value in args {
            self.socket
                .write_all(format!("${}\r\n", value.len()).as_bytes())
                .await
                .map_err(io_error)?;
            self.socket.write_all(value).await.map_err(io_error)?;
            self.socket.write_all(b"\r\n").await.map_err(io_error)?;
        }
        self.socket.flush().await.map_err(io_error)
    }

    pub(super) async fn command(&mut self, arguments: &[&[u8]]) -> AppResult<Reply> {
        self.command_with_budget(arguments, PREVIEW_BYTES).await
    }

    pub(super) async fn string_prefix(&mut self, key: &[u8]) -> AppResult<Reply> {
        // Read one extra byte to distinguish a full preview from an exact-size value
        // even if the string has changed since STRLEN.
        let end = PREVIEW_BYTES.to_string();
        self.command_with_budget(&[b"GETRANGE", key, b"0", end.as_bytes()], PREVIEW_BYTES + 1)
            .await
    }

    pub(super) async fn single_result(&mut self, arguments: &[&[u8]]) -> AppResult<Option<Reply>> {
        bounded(self.deadline, async {
            self.send(arguments).await?;
            protocol::read_single_result(&mut self.socket).await
        })
        .await
    }

    async fn command_with_budget(
        &mut self,
        arguments: &[&[u8]],
        budget: usize,
    ) -> AppResult<Reply> {
        bounded(self.deadline, async {
            self.send(arguments).await?;
            protocol::read_reply(&mut self.socket, budget).await
        })
        .await
    }

    pub(super) async fn collection_values(
        &mut self,
        args: &[&[u8]],
        scan: bool,
        max_items: usize,
    ) -> AppResult<(String, Vec<Vec<u8>>)> {
        bounded(self.deadline, async {
            self.send(args).await?;
            protocol::read_collection_values(&mut self.socket, scan, max_items).await
        })
        .await
    }

    pub(super) async fn scan(&mut self, cursor: &[u8], pattern: &[u8]) -> AppResult<RedisScanPage> {
        bounded(self.deadline, async {
            self.send(&[b"SCAN", cursor, b"MATCH", pattern, b"COUNT", b"100"])
                .await?;
            protocol::read_scan(&mut self.socket).await
        })
        .await
    }
}
