use super::super::*;
use super::mysql_profile_server::{handshake, read_packet, send_packet, MysqlProfileServer};
use super::profile_server::ProfileServer;
use super::support::{mysql_input, postgres_input, service_with_workspace};
use futures_util::future::BoxFuture;
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use unfour_core::network::{BoxedTunnelStream, TunnelConnector};

const TARGET: &str = "database.private.invalid";

#[derive(Clone)]
struct Forward(SocketAddr);
impl TunnelConnector for Forward {
    fn connect<'a>(&'a self, host: &'a str, _: u16) -> BoxFuture<'a, AppResult<BoxedTunnelStream>> {
        Box::pin(async move {
            assert_eq!(host, TARGET, "only the jump resolves the database address");
            Ok(Box::new(TcpStream::connect(self.0).await?) as BoxedTunnelStream)
        })
    }
}
impl DatabaseTunnelResolver for Forward {
    fn resolve(
        &self,
        _: &DatabaseConnection,
        handle: Option<&str>,
        _: Option<&DatabaseConnection>,
    ) -> AppResult<Option<Arc<dyn TunnelConnector>>> {
        assert_eq!(handle, Some("fixture-parent"));
        Ok(Some(Arc::new(self.clone())))
    }
}

#[tokio::test]
async fn tunnel_pool_runs_postgres_opengauss_and_mysql_handshakes() {
    let (service, workspace) = service_with_workspace().await;
    let pg = ProfileServer::start("PostgreSQL 16.6").await;
    let mysql = MysqlProfileServer::start().await;
    for driver in ["postgres", "opengauss", "mysql"] {
        let (mut input, port) = if driver == "mysql" {
            (mysql_input(&workspace, None), mysql.port)
        } else {
            if driver == "opengauss" {
                *pg.version.lock().unwrap() = "openGauss 6.0.0".into();
            }
            (postgres_input(&workspace), pg.port)
        };
        input.host = Some(TARGET.into());
        input.ssl_mode = Some("disable".into());
        let routed = service
            .clone()
            .with_tunnel_resolver(Arc::new(Forward(([127, 0, 0, 1], port).into())))
            .with_connection_route(Some("fixture-parent"), None);
        let result = routed.test_connection_input(input, None).await.unwrap();
        assert!(result.ok);
        if driver != "mysql" {
            assert!(result
                .server_version
                .unwrap()
                .to_lowercase()
                .contains(driver));
        }
    }
    assert!(!mysql.queries.lock().unwrap().is_empty());
}

#[tokio::test]
async fn verify_full_sends_the_original_database_name_and_requires_tls() {
    let (service, workspace) = service_with_workspace().await;
    for mysql in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let forward = Forward(listener.local_addr().unwrap());
        let peer = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            if mysql {
                send_packet(&mut stream, 0, &handshake(true)).await.unwrap();
                assert_eq!(read_packet(&mut stream).await.unwrap().len(), 32);
            } else {
                let mut ssl_request = [0; 8];
                stream.read_exact(&mut ssl_request).await.unwrap();
                assert_eq!(ssl_request, [0, 0, 0, 8, 4, 210, 22, 47]);
                stream.write_all(b"S").await.unwrap();
            }
            let mut header = [0; 5];
            stream.read_exact(&mut header).await.unwrap();
            assert_eq!(header[0], 22);
            let mut hello = vec![0; u16::from_be_bytes([header[3], header[4]]) as usize];
            stream.read_exact(&mut hello).await.unwrap();
            assert!(hello.windows(TARGET.len()).any(|w| w == TARGET.as_bytes()));
        });
        let mut input = if mysql {
            mysql_input(&workspace, None)
        } else {
            postgres_input(&workspace)
        };
        input.host = Some(TARGET.into());
        input.ssl_mode = Some("verify-full".into());
        let routed = service
            .clone()
            .with_tunnel_resolver(Arc::new(forward))
            .with_connection_route(Some("fixture-parent"), None);
        assert!(routed.test_connection_input(input, None).await.is_err());
        peer.await.unwrap();
    }
}

#[tokio::test]
async fn tunnel_cannot_fall_back_to_a_local_postgres_socket() {
    let (service, workspace) = service_with_workspace().await;
    let mut input = postgres_input(&workspace);
    input.host = Some("/tmp".into());
    let routed = service
        .with_tunnel_resolver(Arc::new(Forward(([127, 0, 0, 1], 1).into())))
        .with_connection_route(Some("fixture-parent"), None);
    let err = routed.test_connection_input(input, None).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("a routed TCP pool cannot use a local socket"),
        "{err}"
    );
}

#[tokio::test]
async fn a_second_pool_connection_uses_the_same_jump() {
    let (service, workspace) = service_with_workspace().await;
    let pg = ProfileServer::start("PostgreSQL 16.6").await;
    let mut input = postgres_input(&workspace);
    input.host = Some(TARGET.into());
    input.ssl_mode = Some("disable".into());
    let connection = service.save_connection(input).await.unwrap();
    let service = service
        .with_tunnel_resolver(Arc::new(Forward(([127, 0, 0, 1], pg.port).into())))
        .with_connection_route(Some("fixture-parent"), None);
    let pool = service.postgres_pool(&connection).await.unwrap();
    let first = pool.acquire().await.unwrap();
    let second = pool.acquire().await.unwrap();
    assert_eq!(pg.catalogs.lock().unwrap().len(), 2);
    drop((first, second));
    pool.close().await;
}
