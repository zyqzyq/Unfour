use super::Socket;
use futures_core::future::BoxFuture;
use std::sync::Arc;

/// A concrete handoff type avoids exposing a trait object as the generic
/// socket parameter of the drivers' borrowing TLS handshake futures.
pub(crate) struct DriverSocket(Box<dyn Socket>);
impl Socket for DriverSocket {
    fn try_read(&mut self, buf: &mut dyn crate::io::ReadBuf) -> std::io::Result<usize> {
        self.0.try_read(buf)
    }
    fn try_write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.try_write(buf)
    }
    fn poll_read_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.0.poll_read_ready(cx)
    }
    fn poll_write_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.0.poll_write_ready(cx)
    }
    fn poll_flush(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.0.poll_flush(cx)
    }
    fn poll_shutdown(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.0.poll_shutdown(cx)
    }
}

/// Replaces only the TCP dial for physical connections opened by one pool.
pub trait TcpConnector: Send + Sync {
    fn connect(
        self: Arc<Self>,
        host: String,
        port: u16,
    ) -> BoxFuture<'static, crate::Result<Box<dyn Socket>>>;
}

tokio::task_local! {
    pub(crate) static ACTIVE: Option<Arc<dyn TcpConnector>>;
}

pub(crate) fn current() -> Option<Arc<dyn TcpConnector>> {
    ACTIVE.try_with(Clone::clone).ok().flatten()
}

pub(crate) async fn open(host: String, port: u16) -> crate::Result<DriverSocket> {
    if let Some(connector) = current() {
        return connector.connect(host, port).await.map(DriverSocket);
    }
    let stream = tokio::net::TcpStream::connect((host.as_str(), port)).await?;
    stream.set_nodelay(true)?;
    Ok(DriverSocket(Box::new(stream)))
}

pub(crate) fn during_connect<'a, T: 'a>(
    connector: Option<Arc<dyn TcpConnector>>,
    connect: BoxFuture<'a, T>,
) -> BoxFuture<'a, T> {
    Box::pin(ACTIVE.scope(connector, connect))
}
