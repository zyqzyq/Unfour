//! Offline acceptance fixture. One ephemeral loopback listener, generated host
//! key and synthetic identity. The PTY answers a fixed marker locally.
use russh::server::{Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Default)]
struct Fixture {
    line: Vec<u8>,
    channel_open: bool,
}

impl russh::server::Server for Fixture {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        Self::default()
    }
}

impl russh::server::Handler for Fixture {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        if user == "fixture-user" && password == "fixture-grant" {
            println!("FIXTURE_AUTH_OK");
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn channel_open_session(
        &mut self,
        _: Channel<Msg>,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(!std::mem::replace(&mut self.channel_open, true))
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(
            channel,
            b"A2 fixture ready. Type check and press Enter.\r\n".to_vec(),
        )?;
        println!("FIXTURE_PTY_OK");
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        for &byte in data {
            if matches!(byte, b'\r' | b'\n') {
                if self.line == b"check" {
                    session.data(channel, b"\r\nA2_ECHO_OK\r\n".to_vec())?;
                    println!("FIXTURE_ROUNDTRIP_OK");
                }
                self.line.clear();
            } else if self.line.len() < 6 {
                self.line.push(byte);
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let key = russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)?;
    let fingerprint = key.public_key().fingerprint(russh::keys::HashAlg::Sha256);
    println!(
        "FIXTURE_READY {} {fingerprint}",
        socket.local_addr()?.port()
    );
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let mut fixture = Fixture::default();
    let _ = tokio::time::timeout(
        Duration::from_secs(600),
        fixture.run_on_socket(config, &socket),
    )
    .await;
    Ok(())
}
