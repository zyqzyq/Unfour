use super::*;
use tokio::sync::watch;

/// A live forwarding source. It holds no credentials and cannot start a login.
#[derive(Clone)]
pub struct TemporarySshTunnel {
    #[cfg(feature = "ssh-native")]
    client: std::sync::Weak<russh::client::Handle<SshClientHandler>>,
    closed: watch::Receiver<bool>,
    #[cfg(feature = "ssh-native")]
    pending_forwards: Arc<tokio::sync::Semaphore>,
}

impl SshService {
    pub fn temporary_tunnel(&self, workspace: &str, handle: &str) -> AppResult<TemporarySshTunnel> {
        #[cfg(feature = "ssh-native")]
        {
            let (client, closed, pending_forwards) =
                self.temporary_forward_source(workspace, handle)?;
            Ok(TemporarySshTunnel {
                client: Arc::downgrade(&client),
                closed,
                pending_forwards,
            })
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (workspace, handle);
            Err(failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))
        }
    }
}

impl TemporarySshTunnel {
    pub async fn closed(&self) {
        let mut closed = self.closed.clone();
        let _ = closed.wait_for(|closed| *closed).await;
    }

    pub async fn connect(&self, host: &str, port: u16) -> AppResult<BoxedTunnelStream> {
        self.connect_for_handshake(host, port, std::future::pending())
            .await
    }

    pub(in crate::ssh) async fn connect_for_handshake(
        &self,
        host: &str,
        port: u16,
        abandoned: impl Future<Output = ()> + Send + 'static,
    ) -> AppResult<BoxedTunnelStream> {
        let host = target_host(host, port)?;
        #[cfg(feature = "ssh-native")]
        {
            let client = self
                .client
                .upgrade()
                .ok_or_else(|| failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))?;
            let mut stream = self.open_channel(client, host, port).await?;
            // Wake an in-flight consumer even if the SSH peer never acknowledges
            // disconnect. Each stream has bounded buffering and its own channel.
            let (consumer, mut relay) = tokio::io::duplex(64 * 1024);
            let parent = self.clone();
            let (done, dropped) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                tokio::select! {
                    biased;
                    _ = parent.closed() => {},
                    _ = dropped => {},
                    _ = abandoned => {},
                    _ = tokio::io::copy_bidirectional(&mut stream, &mut relay) => {},
                }
            });
            Ok(Box::new(ForwardedStream {
                stream: consumer,
                _lifetime: done,
            }))
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (host, abandoned);
            Err(failure("SSH_TUNNEL_UNAVAILABLE"))
        }
    }

    #[cfg(feature = "ssh-native")]
    async fn open_channel(
        &self,
        client: Arc<russh::client::Handle<SshClientHandler>>,
        host: &str,
        port: u16,
    ) -> AppResult<BoxedTunnelStream> {
        let permit = self
            .pending_forwards
            .clone()
            .try_acquire_owned()
            .map_err(|_| failure("SSH_TUNNEL_FORWARD_BUSY"))?;
        let (result, opened) = tokio::sync::oneshot::channel();
        let parent = self.clone();
        let host = host.to_owned();
        // Once russh has sent CHANNEL_OPEN, await even a late reply so a channel
        // accepted after the caller cancels can be closed without ending the parent.
        tokio::spawn(async move {
            let _permit = permit;
            if result.is_closed() {
                return;
            }
            let stream = tokio::select! {
                biased;
                _ = parent.closed() => return,
                stream = forward_stream(&client, &host, port) => stream,
            };
            let _ = result.send(stream);
        });
        tokio::time::timeout(std::time::Duration::from_secs(15), opened)
            .await
            .map_err(|_| failure("SSH_TUNNEL_TIMEOUT"))?
            .map_err(|_| failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))?
    }

    pub async fn http_proxy(&self) -> AppResult<TunnelProxyLease> {
        tokio::select! {
            biased;
            _ = self.closed() => Err(failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE")),
            lease = TunnelProxyLease::start(Arc::new(self.clone()), Some(self.clone())) => lease,
        }
    }
}

#[cfg(feature = "ssh-native")]
struct ForwardedStream {
    stream: tokio::io::DuplexStream,
    _lifetime: tokio::sync::oneshot::Sender<()>,
}

#[cfg(feature = "ssh-native")]
impl tokio::io::AsyncRead for ForwardedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

#[cfg(feature = "ssh-native")]
impl tokio::io::AsyncWrite for ForwardedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl TunnelConnector for TemporarySshTunnel {
    fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = AppResult<BoxedTunnelStream>> + Send + 'a>> {
        Box::pin(self.connect(host, port))
    }
}
