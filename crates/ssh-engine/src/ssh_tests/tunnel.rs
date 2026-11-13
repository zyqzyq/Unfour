use super::super::*;
use super::support::{password_input, service_with_workspaces};
use russh::server::{Auth, Server as _, Session};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    time::{timeout, Duration},
};

#[tokio::test]
async fn stale_tunnel_is_rejected_before_dial_and_cancel_closes_an_incomplete_kex() {
    let (service, workspace, _) = service_with_workspaces().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let connection = service
        .save_connection(SshConnectionInput {
            host: "127.0.0.1".into(),
            port: Some(port),
            ..password_input(&workspace)
        })
        .await
        .unwrap();
    let stale = service
        .open_tunnel_at_revision(&workspace, &connection.id, Some(connection.revision - 1))
        .await
        .err()
        .unwrap();
    assert_eq!(stale.code(), "SSH_TUNNEL_ROUTE_CHANGED");
    HostKeyStore::new(service.db.pool().clone())
        .record_fingerprint(&workspace, "127.0.0.1", port, "fixture")
        .await
        .unwrap();
    let open = tokio::spawn(async move { service.open_tunnel(&workspace, &connection.id).await });
    let (socket, _) = listener.accept().await.unwrap();
    let mut peer = BufReader::new(socket);
    let mut banner = String::new();
    peer.read_line(&mut banner).await.unwrap();
    assert!(banner.starts_with("SSH-2.0-"));
    peer.get_mut()
        .write_all(b"SSH-2.0-stalled-test\r\n")
        .await
        .unwrap();
    let mut kex = [0u8; 5];
    timeout(Duration::from_secs(2), peer.read_exact(&mut kex))
        .await
        .unwrap()
        .unwrap();
    open.abort();
    assert!(open.await.is_err());
    // The server never completes key exchange; teardown must not wait for a reply.
    let _ = timeout(Duration::from_secs(2), peer.read_to_end(&mut Vec::new()))
        .await
        .expect("cancelling an SSH handshake must close its TCP connection");
}

#[tokio::test]
async fn host_trust_revoked_during_handshake_stops_authentication() {
    let (service, workspace, _) = service_with_workspaces().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let host_key = fixture_key(7);
    let trust = HostKeyStore::new(service.db.pool().clone());
    trust
        .record_fingerprint(
            &workspace,
            "127.0.0.1",
            port,
            &host_key
                .public_key()
                .fingerprint(ssh_key::HashAlg::Sha256)
                .to_string(),
        )
        .await
        .unwrap();
    let id = service
        .save_connection(SshConnectionInput {
            host: "127.0.0.1".into(),
            port: Some(port),
            ..password_input(&workspace)
        })
        .await
        .unwrap()
        .id;
    let owner = workspace.clone();
    let open = tokio::spawn(async move { service.open_tunnel(&owner, &id).await });
    let (socket, _) = listener.accept().await.unwrap();
    // The client has dialled using the saved pin, but the SSH server has not started KEX yet.
    trust
        .delete_fingerprint(&workspace, "127.0.0.1", port)
        .await
        .unwrap();
    let server = tokio::spawn(async move {
        let config = Arc::new(russh::server::Config {
            keys: vec![host_key],
            ..Default::default()
        });
        let handler = Server {
            client_key: fixture_key(8).public_key().clone(),
        };
        if let Ok(running) = russh::server::run_stream(config, socket, handler).await {
            let _ = running.await;
        }
    });
    assert_eq!(
        open.await.unwrap().err().unwrap().code(),
        "SSH_TUNNEL_HOST_KEY_UNTRUSTED"
    );
    timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}

#[derive(Clone)]
struct Server {
    client_key: ssh_key::PublicKey,
}
impl russh::server::Server for Server {
    type Handler = Self;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self {
        self.clone()
    }
}
impl russh::server::Handler for Server {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _: &str,
        key: &ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(if key == &self.client_key {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: russh::Channel<russh::server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        if host != "internal.invalid" || port != 8443 {
            return Ok(false);
        }
        tokio::spawn(async move {
            let (mut read, mut write) = tokio::io::split(channel.into_stream());
            let _ = tokio::io::copy(&mut read, &mut write).await;
        });
        Ok(true)
    }
}

fn fixture_key(seed: u8) -> ssh_key::PrivateKey {
    ssh_key::PrivateKey::new(
        ssh_key::private::Ed25519Keypair::from_seed(&[seed; 32]).into(),
        "",
    )
    .unwrap()
}

async fn forward(lease: &TunnelProxyLease) -> TcpStream {
    let mut socket = TcpStream::connect(lease.address()).await.unwrap();
    socket.write_all(&[5, 2, 0, 2]).await.unwrap();
    let mut two = [0; 2];
    socket.read_exact(&mut two).await.unwrap();
    assert_eq!(two, [5, 2]);
    let username = lease.username().as_bytes();
    let password = lease.password().as_bytes();
    let mut auth = vec![1, username.len() as u8];
    auth.extend(username);
    auth.push(password.len() as u8);
    auth.extend(password);
    socket.write_all(&auth).await.unwrap();
    socket.read_exact(&mut two).await.unwrap();
    assert_eq!(two, [1, 0]);
    let host = b"internal.invalid";
    let mut request = vec![5, 1, 0, 3, host.len() as u8];
    request.extend(host);
    request.extend(8443u16.to_be_bytes());
    socket.write_all(&request).await.unwrap();
    let mut ack = [0; 10];
    socket.read_exact(&mut ack).await.unwrap();
    assert_eq!(&ack[..2], &[5, 0]);
    socket
}

#[tokio::test]
async fn tunnel_uses_pinned_host_saved_key_and_lease_scoped_remote_dns_channels() {
    let (service, workspace, other) = service_with_workspaces().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let host_key = fixture_key(1);
    let fingerprint = host_key
        .public_key()
        .fingerprint(ssh_key::HashAlg::Sha256)
        .to_string();
    let client = fixture_key(2);
    let mut server = Server {
        client_key: client.public_key().clone(),
    };
    let config = Arc::new(russh::server::Config {
        keys: vec![host_key],
        ..Default::default()
    });
    let server = tokio::spawn(async move {
        server.run_on_socket(config, &listener).await.unwrap();
    });

    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("test-key");
    let encrypted = client
        .encrypt_with(
            ssh_key::Cipher::Aes256Ctr,
            ssh_key::Kdf::Bcrypt {
                salt: vec![3; 16],
                rounds: 1,
            },
            100,
            "fixture passphrase",
        )
        .unwrap();
    encrypted
        .write_openssh_file(&file, ssh_key::LineEnding::LF)
        .unwrap();
    let definition = SshConnectionInput {
        host: "127.0.0.1".into(),
        port: Some(port),
        auth_kind: "private-key".into(),
        credential_ref: None,
        secret: Some("fixture passphrase".into()),
        key_path: Some(file.to_str().unwrap().into()),
        ..password_input(&workspace)
    };
    let id = service.save_connection(definition).await.unwrap().id;
    assert!(matches!(
        service.open_tunnel(&other, &id).await,
        Err(AppError::NotFound(_))
    ));
    assert_eq!(
        service
            .open_tunnel(&workspace, &id)
            .await
            .err()
            .unwrap()
            .to_string(),
        "validation error: SSH_TUNNEL_HOST_KEY_UNTRUSTED"
    );
    let trust = HostKeyStore::new(service.db.pool().clone());
    assert!(trust
        .get_fingerprint(&workspace, "127.0.0.1", port)
        .await
        .unwrap()
        .is_none());
    trust
        .record_fingerprint(&workspace, "127.0.0.1", port, "wrong")
        .await
        .unwrap();
    assert_eq!(
        service
            .open_tunnel(&workspace, &id)
            .await
            .err()
            .unwrap()
            .to_string(),
        "validation error: SSH_TUNNEL_HOST_KEY_CHANGED"
    );
    trust
        .record_fingerprint(&workspace, "127.0.0.1", port, &fingerprint)
        .await
        .unwrap();
    let tunnel = service.open_tunnel(&workspace, &id).await.unwrap();
    assert!(tunnel.connect("not-permitted.invalid", 80).await.is_err());
    let lease = tunnel.http_proxy().await.unwrap();
    assert!(lease.address().ip().is_loopback());
    let mut anonymous = TcpStream::connect(lease.address()).await.unwrap();
    anonymous.write_all(&[5, 1, 0]).await.unwrap();
    let mut response = [0; 2];
    anonymous.read_exact(&mut response).await.unwrap();
    assert_eq!(response, [5, 255]);
    let mut wrong_password = TcpStream::connect(lease.address()).await.unwrap();
    wrong_password.write_all(&[5, 1, 2]).await.unwrap();
    wrong_password.read_exact(&mut response).await.unwrap();
    assert_eq!(response, [5, 2]);
    wrong_password
        .write_all(b"\x01\x06unfour\x03bad")
        .await
        .unwrap();
    wrong_password.read_exact(&mut response).await.unwrap();
    assert_eq!(response, [1, 1]);
    let mut socket = forward(&lease).await;
    socket.write_all(b"data").await.unwrap();
    let mut echo = [0; 4];
    socket.read_exact(&mut echo).await.unwrap();
    assert_eq!(echo, *b"data");
    drop(lease);
    // Either orderly EOF or a socket reset is a completed teardown.
    assert!(!matches!(
        timeout(Duration::from_secs(2), socket.read(&mut echo))
            .await
            .unwrap(),
        Ok(1..)
    ));
    assert!(service.sessions.lock().unwrap().is_empty());
    server.abort();
}
