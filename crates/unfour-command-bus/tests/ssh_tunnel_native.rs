//! Exercise the routed command bus path across real SSH direct-tcpip channels.
#![cfg(feature = "ssh-native")]

#[path = "ssh_tunnel_native/temporary_routes.rs"]
mod temporary_routes;

use base64::{engine::general_purpose::STANDARD, Engine};
use russh::server::{Auth, Msg, Server as _, Session};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use unfour_command_bus::{CommandBus, RedisConnectionInput, RedisMutation};
use unfour_core::{
    models::{ApiRequestInput, FlowRunStatus, SshConnectionInput},
    network::{ApiRoute, ApiRouteEdit},
};
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::HostKeyStore;

#[derive(Clone)]
struct JumpHost(
    Vec<(&'static str, u16, SocketAddr)>,
    Option<Arc<ForwardGate>>,
);

#[derive(Default)]
struct ForwardGate {
    opened: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl russh::server::Server for JumpHost {
    type Handler = Self;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self {
        self.clone()
    }
}
impl russh::server::Handler for JumpHost {
    type Error = russh::Error;
    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if user == "fixture" && password == "temporary" {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        _: russh::Channel<Msg>,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
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
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: russh::ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: russh::Channel<Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        let Some((_, _, mapped)) = self
            .0
            .iter()
            .find(|(name, destination, _)| *name == host && u32::from(*destination) == port)
        else {
            return Ok(false);
        };
        let mut remote = TcpStream::connect(mapped).await?;
        if let Some(gate) = &self.1 {
            gate.opened.notify_one();
            gate.release.notified().await;
        }
        tokio::spawn(async move {
            let _ = tokio::io::copy_bidirectional(&mut remote, &mut channel.into_stream()).await;
        });
        Ok(true)
    }
}

struct Fixture {
    bus: CommandBus,
    db: LocalDb,
    workspace: String,
    jump: String,
    port: u16,
    server: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn fixture(routes: Vec<(&'static str, u16, SocketAddr)>) -> Fixture {
    fixture_with_gate(routes, None).await
}

async fn fixture_with_gate(
    routes: Vec<(&'static str, u16, SocketAddr)>,
    gate: Option<Arc<ForwardGate>>,
) -> Fixture {
    let db = LocalDb::connect_ephemeral().await.unwrap();
    db.migrate().await.unwrap();
    let bus =
        CommandBus::from_db_with_secret_store(db.clone(), SecretStore::in_memory("tunnel-test"))
            .await
            .unwrap();
    let workspace = bus.list_workspaces().await.unwrap().active_workspace_id;
    let mut server = JumpHost(routes, gate);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let host_key =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let fingerprint = host_key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    HostKeyStore::new(db.pool().clone())
        .record_fingerprint(&workspace, "127.0.0.1", port, &fingerprint)
        .await
        .unwrap();
    let config = Arc::new(russh::server::Config {
        keys: vec![host_key],
        ..Default::default()
    });
    let server = tokio::spawn(async move {
        server.run_on_socket(config, &listener).await.unwrap();
    });
    let jump = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace.clone(),
            name: "Fixture jump".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            username: "fixture".into(),
            auth_kind: "password".into(),
            key_path: None,
            credential_ref: None,
            secret: Some("temporary".into()),
        })
        .await
        .unwrap()
        .id;
    Fixture {
        bus,
        db,
        workspace,
        jump,
        port,
        server,
    }
}

async fn http_origin(reply: &'static str) -> (SocketAddr, JoinHandle<String>) {
    let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let observed = tokio::spawn(async move {
        let (stream, _) = socket.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut headers = String::new();
        loop {
            let len = reader.read_line(&mut headers).await.unwrap();
            assert!(len > 0 && headers.len() < 8192);
            if headers.ends_with("\r\n\r\n") {
                break;
            }
        }
        reader.get_mut().write_all(reply.as_bytes()).await.unwrap();
        headers
    });
    (address, observed)
}

#[tokio::test]
async fn redis_auth_and_reads_complete_on_the_forwarded_channel() {
    let backend = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = backend.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = backend.accept().await.unwrap();
        for (request, reply) in [
            ("*2\r\n$4\r\nAUTH\r\n$7\r\nfixture\r\n", "+OK\r\n"),
            ("*2\r\n$6\r\nSELECT\r\n$1\r\n2\r\n", "+OK\r\n"),
            ("*2\r\n$4\r\nTYPE\r\n$1\r\nk\r\n", "+string\r\n"),
            ("*2\r\n$4\r\nPTTL\r\n$1\r\nk\r\n", ":1234\r\n"),
        ] {
            let mut actual = vec![0; request.len()];
            stream.read_exact(&mut actual).await.unwrap();
            assert_eq!(actual, request.as_bytes());
            stream.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let f = fixture(vec![("cache.invalid", 6379, address)]).await;
    let saved = f
        .bus
        .save_redis_connection(
            RedisConnectionInput {
                id: None,
                workspace_id: f.workspace.clone(),
                name: "cache".into(),
                host: "cache.invalid".into(),
                port: 6379,
                db_index: 2,
                username: None,
                tls: false,
                ssh_connection_id: Some(Some(f.jump.clone())),
                expected_ssh_revision: None,
                expected_revision: None,
                allow_writes: None,
            },
            Some("fixture".into()),
        )
        .await
        .unwrap();
    let result = f
        .bus
        .check_redis_key(&f.workspace, &saved.id, b"k", None, None)
        .await
        .unwrap();
    assert!(result.exists && result.matched);
    assert_eq!(result.pttl_ms, 1234);
    peer.await.unwrap();
}

#[tokio::test]
async fn redis_deadline_includes_waiting_for_the_jump_handshake() {
    let f = fixture(vec![]).await;
    let stalled = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = stalled.local_addr().unwrap().port();
    sqlx::query("UPDATE connections SET port=? WHERE workspace_id=? AND id=?")
        .bind(port)
        .bind(&f.workspace)
        .bind(&f.jump)
        .execute(f.db.pool())
        .await
        .unwrap();
    HostKeyStore::new(f.db.pool().clone())
        .record_fingerprint(&f.workspace, "127.0.0.1", port, "fixture")
        .await
        .unwrap();
    // Accept the SSH socket but never complete the identification exchange.
    let server = tokio::spawn(async move {
        let (stream, _) = stalled.accept().await.unwrap();
        (stream, stalled)
    });
    let target = RedisConnectionInput {
        workspace_id: f.workspace.clone(),
        id: None,
        name: "test".into(),
        host: "cache.invalid".into(),
        port: 6379,
        db_index: 0,
        username: None,
        tls: false,
        allow_writes: None,
        ssh_connection_id: Some(Some(f.jump.clone())),
        expected_ssh_revision: None,
        expected_revision: None,
    };
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        f.bus.test_redis_connection_input(target, None),
    )
    .await
    .expect("Redis did not bound SSH setup");
    assert_eq!(result.unwrap_err().code(), "REDIS_TIMEOUT");
    let _ = server.await.unwrap();
}

#[tokio::test]
async fn environment_http_route_reaches_both_redirect_hosts_via_the_jump() {
    let (first_addr, first) = http_origin("HTTP/1.1 302 Found\r\nLocation: http://second.invalid/done\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
    let (second_addr, second) =
        http_origin("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK").await;
    let (flow_addr, flow_origin) =
        http_origin("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK").await;
    let f = fixture(vec![
        ("first.invalid", 80, first_addr),
        ("second.invalid", 80, second_addr),
        ("flow.invalid", 80, flow_addr),
    ])
    .await;
    let environment = f
        .bus
        .workspace_environment_create(f.workspace.clone(), "Tunnel env".into())
        .await
        .unwrap();
    f.bus
        .set_api_environment_route(
            &f.workspace,
            &environment.id,
            ApiRouteEdit {
                expected: ApiRoute::Direct,
                route: ApiRoute::Ssh {
                    ssh_connection_id: f.jump.clone(),
                },
            },
        )
        .await
        .unwrap();
    let mut input: ApiRequestInput = serde_json::from_value(serde_json::json!({
        "workspaceId":f.workspace, "method":"GET", "url":"http://first.invalid/start",
        "headers":[], "query":[], "bodyKind":"none", "timeoutMs":5000
    }))
    .unwrap();
    let response = f
        .bus
        .send_api_request_in_environment(input.clone(), Some(environment.id.clone()))
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, "OK");
    assert!(first
        .await
        .unwrap()
        .to_ascii_lowercase()
        .contains("\r\nhost: first.invalid\r\n"));
    assert!(second
        .await
        .unwrap()
        .to_ascii_lowercase()
        .contains("\r\nhost: second.invalid\r\n"));
    input.url = "http://third.invalid/denied".into();
    let rejected = f
        .bus
        .send_api_request_in_environment(input.clone(), Some(environment.id.clone()))
        .await
        .unwrap_err();
    assert_eq!(rejected.code(), "SSH_TUNNEL_TARGET_UNREACHABLE");

    input.url = "http://flow.invalid/health".into();
    let saved = f.bus.save_api_request(input).await.unwrap();
    let definition = serde_json::from_value(serde_json::json!({
        "id":"", "workspaceId":f.workspace, "revision":0, "name":"SSH HTTP", "inputs":[],
        "steps":[{ "id":"check", "name":"Check", "kind":"action", "timeoutMs":5000,
            "action":{ "capability":"api", "resourceId":saved.id, "arguments":{} } }]
    }))
    .unwrap();
    let flow = f.bus.save_flow(definition).await.unwrap();
    let run = f
        .bus
        .run_flow(
            serde_json::from_value(serde_json::json!({
                "workspaceId":f.workspace, "flowId":flow.id, "environmentId":environment.id,
                "inputs":{}, "confirmEffects":true, "initiator":"human"
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let finished = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let state = f
                .bus
                .get_flow_run(f.workspace.clone(), run.id.clone())
                .await
                .unwrap();
            if state.status != FlowRunStatus::Running {
                break state;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        finished.status,
        FlowRunStatus::Succeeded,
        "{:?}",
        finished.error
    );
    assert!(flow_origin
        .await
        .unwrap()
        .to_ascii_lowercase()
        .contains("\r\nhost: flow.invalid\r\n"));
}

#[tokio::test]
async fn successful_ssh_http_response_survives_local_history_backpressure() {
    let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let f = fixture(vec![("history.invalid", 80, origin.local_addr().unwrap())]).await;
    let environment = f
        .bus
        .workspace_environment_create(f.workspace.clone(), "Test".into())
        .await
        .unwrap();
    f.bus
        .set_api_environment_route(
            &f.workspace,
            &environment.id,
            ApiRouteEdit {
                expected: ApiRoute::Direct,
                route: ApiRoute::Ssh {
                    ssh_connection_id: f.jump.clone(),
                },
            },
        )
        .await
        .unwrap();
    let input: ApiRequestInput = serde_json::from_value(serde_json::json!({
        "workspaceId":f.workspace,"method":"POST","url":"http://history.invalid/",
        "headers":[],"query":[],"bodyKind":"none","timeoutMs":1500
    }))
    .unwrap();
    let db = f.db.clone();
    let peer = tokio::spawn(async move {
        let (stream, _) = origin.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut headers = String::new();
        while !headers.ends_with("\r\n\r\n") {
            assert!(reader.read_line(&mut headers).await.unwrap() > 0 && headers.len() < 8192);
        }
        // Hold the sole SQLite connection before completing the remote POST.
        let held = db.pool().acquire().await.unwrap();
        reader
            .get_mut()
            .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        drop(reader);
        tokio::time::sleep(Duration::from_millis(1700)).await;
        drop(held);
    });
    let response = f
        .bus
        .send_api_request_in_environment(input, Some(environment.id))
        .await
        .unwrap();
    assert_eq!(response.status, 201);
    assert!(!response.history_id.is_empty());
    peer.await.unwrap();
}

struct RedisProcess(std::process::Child);
impl Drop for RedisProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[tokio::test]
#[ignore = "set UNFOUR_TEST_REDIS_SERVER to a real local redis-server executable"]
async fn redis_reads_and_confirmed_writes_reach_a_real_server_through_the_jump() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let password = unfour_core::id::new_id();
    let mut command = Command::new(std::env::var_os("UNFOUR_TEST_REDIS_SERVER").unwrap());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut server = RedisProcess(
        command
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    writeln!(server.0.stdin.take().unwrap(),
        "bind 127.0.0.1\nprotected-mode yes\nport {port}\nsave \"\"\nappendonly no\nrequirepass {password}").unwrap();
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    tokio::time::timeout(Duration::from_secs(12), async {
        while TcpStream::connect(address).await.is_err() {
            assert!(server.0.try_wait().unwrap().is_none());
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
    let f = fixture(vec![("cache.invalid", 6379, address)]).await;
    let connection = f
        .bus
        .save_redis_connection(
            RedisConnectionInput {
                id: None,
                workspace_id: f.workspace.clone(),
                name: "Tunnel cache".into(),
                host: "cache.invalid".into(),
                port: 6379,
                db_index: 2,
                username: None,
                tls: false,
                ssh_connection_id: Some(Some(f.jump.clone())),
                expected_ssh_revision: None,
                allow_writes: Some(true),
                expected_revision: None,
            },
            Some(password),
        )
        .await
        .unwrap();
    let key = STANDARD.encode("tunnel:fixture");
    let value_base64 = STANDARD.encode([0xff, 0, b'a']);
    let revision = connection.revision;
    f.bus
        .mutate_redis_key(
            &f.workspace,
            &connection.id,
            &key,
            RedisMutation::CreateString {
                value_base64: value_base64.clone(),
                ttl_ms: Some(60_000),
            },
            revision,
            true,
        )
        .await
        .unwrap();
    let value = f
        .bus
        .inspect_redis_key(&f.workspace, &connection.id, &key, None, Some(revision))
        .await
        .unwrap();
    assert!(value.exists && value.pttl_ms > 0 && value.pttl_ms <= 60_000);
    assert_eq!(value.preview.unwrap().base64, value_base64);
}
