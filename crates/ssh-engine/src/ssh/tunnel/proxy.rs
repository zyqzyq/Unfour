//! Loopback SOCKS5 bridge for HTTP clients. A lease owns the listener, password
//! and relays. No HTTP headers or TLS records are inspected by the bridge.
use super::{failure, AppResult, BoxedTunnelStream, TunnelConnector};
use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};
use uuid::Uuid;
use zeroize::Zeroizing;

const USER: &str = "unfour";
const MAX_PENDING_CLIENTS: usize = 32;

#[derive(Default)]
struct ProxyProgress {
    failure: OnceLock<&'static str>,
    forwarding: Mutex<Forwarding>,
}

#[derive(Default, PartialEq)]
enum Forwarding {
    #[default]
    AwaitingBytes,
    BytesAccepted,
    ClosedBeforeWrite,
}

pub struct TunnelProxyLease {
    address: SocketAddr,
    password: Arc<Zeroizing<String>>,
    task: JoinHandle<()>,
    progress: Arc<ProxyProgress>,
}

impl TunnelProxyLease {
    pub(super) async fn start(
        connector: Arc<dyn TunnelConnector>,
        parent: Option<super::TemporarySshTunnel>,
    ) -> AppResult<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| failure("SSH_TUNNEL_PROXY_FAILED"))?;
        let address = listener
            .local_addr()
            .map_err(|_| failure("SSH_TUNNEL_PROXY_FAILED"))?;
        let password = Arc::new(Zeroizing::new(Uuid::new_v4().simple().to_string()));
        let progress = Arc::new(ProxyProgress::default());
        let server = serve(listener, connector, password.clone(), progress.clone());
        let status = progress.clone();
        let task = tokio::spawn(async move {
            if let Some(parent) = parent {
                tokio::select! {
                    biased;
                    _ = parent.closed() => { let _ = status.failure.set("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"); }
                    _ = server => {}
                }
            } else {
                server.await;
            }
        });
        Ok(Self {
            address,
            password,
            task,
            progress,
        })
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn username(&self) -> &str {
        USER
    }
    pub fn password(&self) -> &str {
        self.password.as_str()
    }
    /// Seal pending relays and report whether any SSH channel accepted client bytes,
    /// including on a previous redirect hop. A SOCKS handshake alone sends none.
    pub fn finish_after_error(&self) -> bool {
        let mut stage = self.progress.forwarding.lock().unwrap();
        if *stage == Forwarding::BytesAccepted {
            return true;
        }
        *stage = Forwarding::ClosedBeforeWrite;
        false
    }

    pub fn remote_failure_code(&self) -> Option<&'static str> {
        self.progress.failure.get().copied()
    }
}

impl Drop for TunnelProxyLease {
    fn drop(&mut self) {
        // Dropping serve's JoinSet also aborts relays already accepted. This holds
        // even if a partially read greeting or a remote channel remains open.
        self.task.abort();
    }
}

async fn serve(
    listener: TcpListener,
    connector: Arc<dyn TunnelConnector>,
    password: Arc<Zeroizing<String>>,
    progress: Arc<ProxyProgress>,
) {
    let mut relays = JoinSet::new();
    loop {
        tokio::select! {
            result = listener.accept(), if relays.len() < MAX_PENDING_CLIENTS => {
                let Ok((socket, _)) = result else { break };
                let connector = connector.clone();
                let password = password.clone();
                let status = progress.clone();
                relays.spawn(async move { let _ = relay(socket, &*connector, password.as_str(), &status).await; });
            }
            _ = relays.join_next(), if !relays.is_empty() => {}
        }
    }
}

fn invalid() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidData)
}

// Each SOCKS length is a single octet, checked before allocating.
async fn read_field(socket: &mut TcpStream) -> io::Result<Vec<u8>> {
    let len = socket.read_u8().await?;
    if len == 0 {
        return Err(invalid());
    }
    let mut field = vec![0; len as usize];
    socket.read_exact(&mut field).await?;
    Ok(field)
}

async fn target(socket: &mut TcpStream, password: &str) -> io::Result<(String, u16)> {
    if socket.read_u8().await? != 5 {
        return Err(invalid());
    }
    let methods = read_field(socket).await?;
    if !methods.contains(&2) {
        socket.write_all(&[5, 255]).await?;
        return Err(invalid());
    }
    socket.write_all(&[5, 2]).await?;
    if socket.read_u8().await? != 1 {
        return Err(invalid());
    }
    let username = read_field(socket).await?;
    let supplied = Zeroizing::new(read_field(socket).await?);
    if username != USER.as_bytes() || supplied.as_slice() != password.as_bytes() {
        socket.write_all(&[1, 1]).await?;
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    socket.write_all(&[1, 0]).await?;
    let mut header = [0u8; 4];
    socket.read_exact(&mut header).await?;
    if header[..3] != [5, 1, 0] {
        reply(socket, 7).await?;
        return Err(invalid());
    }
    let host = match header[3] {
        1 => Ipv4Addr::from(socket.read_u32().await?).to_string(),
        3 => String::from_utf8(read_field(socket).await?).map_err(|_| invalid())?,
        4 => {
            let mut bytes = [0; 16];
            socket.read_exact(&mut bytes).await?;
            Ipv6Addr::from(bytes).to_string()
        }
        _ => {
            reply(socket, 8).await?;
            return Err(invalid());
        }
    };
    Ok((host, socket.read_u16().await?))
}

async fn reply(socket: &mut TcpStream, status: u8) -> io::Result<()> {
    socket.write_all(&[5, status, 0, 1, 0, 0, 0, 0, 0, 0]).await
}

async fn relay(
    mut socket: TcpStream,
    connector: &dyn TunnelConnector,
    password: &str,
    progress: &ProxyProgress,
) -> io::Result<()> {
    let (host, port) =
        tokio::time::timeout(Duration::from_secs(5), target(&mut socket, password)).await??;
    let channel = match connector.connect(&host, port).await {
        Ok(channel) => channel,
        Err(error) => {
            let _ = progress.failure.set(error.code());
            return reply(&mut socket, 4).await;
        }
    };
    reply(&mut socket, 0).await?;
    let mut channel = TrackedChannel {
        channel,
        progress,
        forwarded: false,
    };
    tokio::io::copy_bidirectional(&mut socket, &mut channel).await?;
    Ok(())
}

struct TrackedChannel<'a> {
    channel: BoxedTunnelStream,
    progress: &'a ProxyProgress,
    forwarded: bool,
}

impl AsyncRead for TrackedChannel<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.channel).poll_read(cx, buf)
    }
}

impl AsyncWrite for TrackedChannel<'_> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let relay = self.get_mut();
        if relay.forwarded || buf.is_empty() {
            return Pin::new(&mut relay.channel).poll_write(cx, buf);
        }
        // Serialize the first accepted payload with finish_after_error. Aborting a
        // JoinHandle alone would allow a late first write after reporting "not sent".
        let mut stage = relay.progress.forwarding.lock().unwrap();
        if *stage == Forwarding::ClosedBeforeWrite {
            return Poll::Ready(Err(io::ErrorKind::ConnectionAborted.into()));
        }
        let result = Pin::new(&mut relay.channel).poll_write(cx, buf);
        if matches!(result, Poll::Ready(Ok(n)) if n > 0) {
            *stage = Forwarding::BytesAccepted;
            relay.forwarded = true;
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.channel).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.channel).poll_shutdown(cx)
    }
}
