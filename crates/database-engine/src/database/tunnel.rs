use super::*;
use futures_util::{
    future::BoxFuture,
    task::{waker_ref, ArcWake, AtomicWaker},
};
use sqlx_core::{
    io::ReadBuf,
    net::{tcp_connector::TcpConnector, Socket},
};
use std::{
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use unfour_core::network::{BoxedTunnelStream, TunnelConnector};

pub trait DatabaseTunnelResolver: Send + Sync {
    fn resolve(
        &self,
        connection: &DatabaseConnection,
        temporary_handle: Option<&str>,
        draft_source: Option<&DatabaseConnection>,
    ) -> AppResult<Option<Arc<dyn TunnelConnector>>>;
}

impl DatabaseService {
    pub fn with_tunnel_resolver(mut self, resolver: Arc<dyn DatabaseTunnelResolver>) -> Self {
        self.tunnel_resolver = Some(resolver);
        self
    }

    pub fn with_connection_route(
        &self,
        handle: Option<&str>,
        expected: Option<DatabaseConnectionVersion>,
    ) -> Self {
        let mut service = self.clone();
        service.temporary_handle = handle.map(str::to_owned);
        service.expected_connection = expected;
        service
    }

    pub(super) fn routed_pool_options<DB: sqlx::Database>(
        &self,
        connection: &DatabaseConnection,
        options: sqlx::pool::PoolOptions<DB>,
    ) -> AppResult<sqlx::pool::PoolOptions<DB>> {
        let requested = connection.ssh_connection_id.is_some() || self.temporary_handle.is_some();
        let connector = match &self.tunnel_resolver {
            Some(resolver) => resolver.resolve(
                connection,
                self.temporary_handle.as_deref(),
                self.draft_source.as_ref(),
            )?,
            None => None,
        };
        match connector {
            Some(connector) => {
                Ok(options.tcp_connector(Arc::new(DatabaseSocketFactory(connector))))
            }
            None if requested => Err(AppError::Validation("SSH_TUNNEL_ROUTE_MISSING".into())),
            None => Ok(options),
        }
    }
}

struct DatabaseSocketFactory(Arc<dyn TunnelConnector>);
impl TcpConnector for DatabaseSocketFactory {
    fn connect(
        self: Arc<Self>,
        host: String,
        port: u16,
    ) -> BoxFuture<'static, sqlx::Result<Box<dyn Socket>>> {
        Box::pin(async move {
            let stream = self
                .0
                .connect(&host, port)
                .await
                .map_err(|err| sqlx::Error::Configuration(Box::new(err)))?;
            Ok(Box::new(TunnelSocket {
                stream,
                read_ready: Arc::default(),
                write_ready: Arc::default(),
            }) as Box<dyn Socket>)
        })
    }
}

// SQLx separates a nonblocking attempt from readiness registration. A channel
// can wake between those two calls. Keep one latched wake per direction.
#[derive(Default)]
struct Readiness {
    signalled: AtomicBool,
    waiter: AtomicWaker,
}
impl ArcWake for Readiness {
    fn wake_by_ref(this: &Arc<Self>) {
        this.signalled.store(true, Ordering::Release);
        this.waiter.wake();
    }
}
impl Readiness {
    fn poll(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.waiter.register(cx.waker());
        if self.signalled.swap(false, Ordering::AcqRel) {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}
struct TunnelSocket {
    stream: BoxedTunnelStream,
    read_ready: Arc<Readiness>,
    write_ready: Arc<Readiness>,
}

impl Socket for TunnelSocket {
    fn try_read(&mut self, buf: &mut dyn ReadBuf) -> io::Result<usize> {
        let waker = waker_ref(&self.read_ready);
        let mut cx = Context::from_waker(&waker);
        let mut scratch = [0; 8192];
        let size = buf.remaining_mut().min(scratch.len());
        let mut read = tokio::io::ReadBuf::new(&mut scratch[..size]);
        match Pin::new(&mut *self.stream).poll_read(&mut cx, &mut read) {
            Poll::Ready(Ok(())) => {
                buf.put_slice(read.filled());
                Ok(read.filled().len())
            }
            Poll::Ready(Err(err)) => Err(err),
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
    fn try_write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let waker = waker_ref(&self.write_ready);
        match Pin::new(&mut *self.stream).poll_write(&mut Context::from_waker(&waker), buf) {
            Poll::Ready(result) => result,
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
    fn poll_read_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.read_ready.poll(cx)
    }
    fn poll_write_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.write_ready.poll(cx)
    }
    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.stream).poll_flush(cx)
    }
    fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.stream).poll_shutdown(cx)
    }
}

pub(super) fn route_error(error: sqlx::Error) -> Result<sqlx::Error, AppError> {
    if let sqlx::Error::Configuration(cause) = error {
        return match cause.downcast::<AppError>() {
            Ok(cause) => Err(*cause),
            Err(other) => Ok(sqlx::Error::Configuration(other)),
        };
    }
    Ok(error)
}
