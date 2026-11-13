use super::*;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::net::TcpStream;
use unfour_core::network::{BoxedTunnelStream, TunnelConnector};

struct TestJump {
    port: u16,
}

impl TunnelConnector for TestJump {
    fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = AppResult<BoxedTunnelStream>> + Send + 'a>> {
        Box::pin(async move {
            assert_eq!((host, port), ("internal.invalid", 6380));
            let stream = TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
            Ok(Box::new(stream) as BoxedTunnelStream)
        })
    }
}

#[tokio::test]
async fn tunnel_resolves_original_name_and_preserves_auth_and_logical_db() {
    let (mut endpoint, server) = scripted(vec![
        exchange(&[b"AUTH", b"reader", b"private"], b"+OK\r\n"),
        exchange(&[b"SELECT", b"4"], b"+OK\r\n"),
        exchange(&[b"PING"], b"+PONG\r\n"),
    ])
    .await;
    let jump = Arc::new(TestJump {
        port: endpoint.port,
    });
    endpoint.host = "internal.invalid".into();
    endpoint.port = 6380;
    endpoint.username = Some("reader".into());
    endpoint.db_index = 4;
    endpoint.transport = RedisTransport::RequiresTunnel;
    assert_eq!(
        test_connection(&endpoint, None).await.unwrap_err().code(),
        "SSH_TUNNEL_ROUTE_MISSING"
    );
    test_connection(&endpoint.with_tunnel(jump), Some("private"))
        .await
        .unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn redis_tls_uses_destination_sni_inside_the_tunnel() {
    use tokio::io::AsyncReadExt;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let jump = Arc::new(TestJump {
        port: listener.local_addr().unwrap().port(),
    });
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut header = [0u8; 5];
        stream.read_exact(&mut header).await.unwrap();
        assert_eq!(header[0], 22); // TLS handshake record.
        let mut hello = vec![0; u16::from_be_bytes([header[3], header[4]]) as usize];
        stream.read_exact(&mut hello).await.unwrap();
        assert!(hello
            .windows(b"internal.invalid".len())
            .any(|s| s == b"internal.invalid"));
    });
    let endpoint = RedisEndpoint {
        host: "internal.invalid".into(),
        port: 6380,
        username: None,
        db_index: 0,
        tls: true,
        transport: RedisTransport::Tunnel(jump),
    };
    assert_eq!(
        test_connection(&endpoint, None).await.unwrap_err().code(),
        "REDIS_TLS_FAILED"
    );
    server.await.unwrap();
}
