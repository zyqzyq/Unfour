use super::*;
use std::{
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Default)]
pub(super) struct State {
    pub auths: AtomicUsize,
    pub ptys: AtomicUsize,
    pub data_bytes: AtomicUsize,
    pub wire_read: AtomicUsize,
    pub silent: AtomicBool,
    pub closed: AtomicBool,
    pub auth_hold: Option<Arc<tokio::sync::Notify>>,
    pub pty_hold: Option<Arc<tokio::sync::Notify>>,
    pub server: Mutex<Option<russh::server::Handle>>,
    pub channel: Mutex<Option<russh::ChannelId>>,
}

struct Wire {
    socket: tokio::net::TcpStream,
    state: Arc<State>,
}
impl AsyncRead for Wire {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.socket).poll_read(cx, buffer);
        if matches!(result, Poll::Ready(Ok(()))) {
            self.state
                .wire_read
                .fetch_add(buffer.filled().len() - before, Ordering::SeqCst);
        }
        result
    }
}
impl AsyncWrite for Wire {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.state.silent.load(Ordering::SeqCst) {
            Poll::Ready(Ok(bytes.len()))
        } else {
            Pin::new(&mut self.socket).poll_write(cx, bytes)
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.socket).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.socket).poll_shutdown(cx)
    }
}

struct Server(Arc<State>);
impl russh::server::Handler for Server {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        assert_eq!((user, password), ("synthetic-user", "synthetic-grant"));
        self.0.auths.fetch_add(1, Ordering::SeqCst);
        if let Some(hold) = &self.0.auth_hold {
            hold.notified().await;
        }
        Ok(russh::server::Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        mut channel: russh::Channel<russh::server::Msg>,
        _: &mut russh::server::Session,
    ) -> Result<bool, Self::Error> {
        // The handler observes input, but russh also queues a duplicate for the
        // Channel owner. Consume it so the fixture cannot backpressure after N packets.
        tokio::spawn(async move { while channel.wait().await.is_some() {} });
        Ok(true)
    }
    async fn pty_request(
        &mut self,
        channel: russh::ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        self.0.ptys.fetch_add(1, Ordering::SeqCst);
        if let Some(hold) = &self.0.pty_hold {
            hold.notified().await;
        }
        session.channel_success(channel)
    }
    async fn shell_request(
        &mut self,
        channel: russh::ChannelId,
        session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        *self.0.server.lock().unwrap() = Some(session.handle());
        *self.0.channel.lock().unwrap() = Some(channel);
        session.channel_success(channel)
    }
    async fn data(
        &mut self,
        channel: russh::ChannelId,
        bytes: &[u8],
        session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        self.0.data_bytes.fetch_add(bytes.len(), Ordering::SeqCst);
        session.data(channel, bytes.to_vec())
    }
}

pub(super) struct Fixture {
    pub service: SshService,
    pub workspace: String,
    pub handle: String,
    pub state: Arc<State>,
    server_task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.service.cancel_temporary(&self.workspace, &self.handle);
        self.server_task.abort();
    }
}
impl Fixture {
    pub fn summary(&self) -> TemporarySummary {
        self.service
            .list_temporary(&self.workspace)
            .unwrap()
            .into_iter()
            .find(|item| item.handle == self.handle)
            .unwrap()
    }
    pub async fn connected(&self) {
        until(|| self.summary().status == "connected").await;
        assert!(self.summary().error_code.is_none());
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    pub fn count(&self) -> usize {
        self.state.wire_read.load(Ordering::SeqCst)
    }
}

pub(super) async fn create(state: State) -> Fixture {
    let state = Arc::new(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let key = ssh_key::private::Ed25519Keypair::from_seed(&[57; 32]).into();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::ZERO,
        ..Default::default()
    });
    let observed = state.clone();
    let server_task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let wire = Wire {
            socket,
            state: observed.clone(),
        };
        let running = russh::server::run_stream(config, wire, Server(observed.clone()))
            .await
            .unwrap();
        let _ = running.await;
        observed.closed.store(true, Ordering::SeqCst);
    });
    let db = LocalDb::connect_ephemeral().await.unwrap();
    db.migrate().await.unwrap();
    let workspace = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO workspaces (id,name,is_default,created_at,updated_at,revision,sync_status) VALUES (?,'keepalive fixture',0,'now','now',1,'local')")
        .bind(&workspace).execute(db.pool()).await.unwrap();
    let service = SshService::new(db, SecretStore::in_memory("temporary-keepalive-test"));
    let request = TemporaryRequest::password(
        "127.0.0.1".into(),
        port,
        "synthetic-user".into(),
        "synthetic-grant".into(),
    )
    .unwrap();
    let handle = service
        .receive_temporary(&LaunchReceipt::default(), request)
        .unwrap();
    service.claim_temporary(&workspace, &handle).await.unwrap();
    service.start_temporary(&workspace, &handle).unwrap();
    Fixture {
        service,
        workspace,
        handle,
        state,
        server_task,
    }
}

pub(super) async fn until(condition: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("native keepalive fixture did not reach expected state");
}
pub(super) async fn advance(seconds: u64) {
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(seconds)).await;
    tokio::time::resume();
    // Let real TCP IO settle between accelerated idle deadlines.
    tokio::time::sleep(Duration::from_millis(40)).await;
}
