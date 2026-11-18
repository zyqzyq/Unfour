//! Offline acceptance fixture. One ephemeral loopback listener, generated host
//! key and synthetic identity. No OS commands or filesystem are exposed.
#[path = "bastion_fixture/raw_terminal.rs"]
mod raw_terminal;
#[path = "bastion_fixture/sftp.rs"]
mod sftp;
#[path = "bastion_fixture/terminal.rs"]
mod terminal;

use russh::server::{Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId};
use std::{collections::HashMap, sync::Arc, time::Duration};

#[derive(Default)]
struct Fixture {
    line: Vec<u8>,
    channels: HashMap<ChannelId, Channel<Msg>>,
    shell_channel: Option<ChannelId>,
    shell_reader: Option<tokio::task::JoinHandle<()>>,
    terminal: terminal::Terminal,
    raw_terminal: raw_terminal::RawTerminal,
    bootstrap: Vec<u8>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(reader) = self.shell_reader.take() {
            reader.abort();
        }
    }
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
        channel: Channel<Msg>,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        if self.channels.len() + usize::from(self.shell_channel.is_some()) >= 8 {
            return Ok(false);
        }
        self.channels.insert(channel.id(), channel);
        if self.channels.len() + usize::from(self.shell_channel.is_some()) == 3 {
            println!("FIXTURE_THREE_CHANNELS_OK");
        }
        Ok(true)
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&channel);
        if self.shell_channel == Some(channel) {
            self.shell_channel = None;
            self.line.clear();
            if let Some(reader) = self.shell_reader.take() {
                reader.abort();
            }
        }
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        if host != "tcp.fixture.invalid" || port != 32123 {
            println!("FIXTURE_TCP_DENIED");
            return Ok(false);
        }
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut stream = channel.into_stream();
            let mut bytes = [0u8; 1];
            if matches!(stream.read(&mut bytes).await, Ok(0)) {
                println!("FIXTURE_TCP_EMPTY_OK");
            } else {
                println!("FIXTURE_TCP_UNEXPECTED_DATA");
            }
        });
        Ok(true)
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        command: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let exit_code = match command {
            b"fixture-task-login" => {
                session.channel_success(channel)?;
                session.data(channel, b"\x1b]0;synthetic-title\x07login: ".to_vec())?;
                println!("FIXTURE_TASK_LOGIN");
                return Ok(());
            }
            b"fixture-task-ok" => {
                session.channel_success(channel)?;
                session.data(channel, b"UNFOUR_TASK_STDOUT\n".to_vec())?;
                session.extended_data(channel, 1, b"UNFOUR_TASK_STDERR\n".to_vec())?;
                session.exit_status_request(channel, 0)?;
                session.eof(channel)?;
                session.close(channel)?;
                println!("FIXTURE_TASK_OK");
                return Ok(());
            }
            b"fixture-task-wait" => {
                session.channel_success(channel)?;
                session.data(channel, b"UNFOUR_TASK_WAITING\n".to_vec())?;
                println!("FIXTURE_TASK_WAITING");
                return Ok(());
            }
            b"printf 'UNFOUR_A4_STDOUT\\n'; printf 'UNFOUR_A4_STDERR\\n' >&2; exit 0" => 0,
            b"printf 'UNFOUR_A4_STDOUT\\n'; printf 'UNFOUR_A4_STDERR\\n' >&2; exit 23" => 23,
            _ => return session.channel_failure(channel),
        };
        session.channel_success(channel)?;
        session.data(channel, b"UNFOUR_A4_STDOUT\n".to_vec())?;
        session.extended_data(channel, 1, b"UNFOUR_A4_STDERR\n".to_vec())?;
        session.exit_status_request(channel, exit_code)?;
        session.eof(channel)?;
        session.close(channel)?;
        println!("FIXTURE_EXEC_OK {exit_code}");
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name != "sftp" {
            return session.channel_failure(channel_id);
        }
        let Some(channel) = self.channels.remove(&channel_id) else {
            return session.channel_failure(channel_id);
        };
        session.channel_success(channel_id)?;
        println!("FIXTURE_SFTP_OK");
        // The library starts its own worker and returns immediately.
        russh_sftp::server::run_with_config(
            channel.into_stream(),
            sftp::MemoryFile::default(),
            russh_sftp::server::Config {
                max_client_packet_len: 4096,
            },
        )
        .await;
        Ok(())
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
        if self.shell_channel.is_some() {
            return session.channel_failure(channel);
        }
        self.shell_channel = Some(channel);
        // russh sends input to both this callback and Channel's bounded reader
        // queue. The callback simulates the shell; drain the duplicate queue so
        // repeated tasks/probes cannot block after channel_buffer_size packets.
        let mut messages = self
            .channels
            .remove(&channel)
            .expect("opened shell channel");
        self.shell_reader = Some(tokio::spawn(async move {
            while messages.wait().await.is_some() {}
        }));
        session.channel_success(channel)?;
        session.data(
            channel,
            b"A2 fixture ready. Type check and press Enter.\r\n".to_vec(),
        )?;
        println!("FIXTURE_PTY_OK");
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _: ChannelId,
        cols: u32,
        rows: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        println!("FIXTURE_RESIZE {cols} {rows}");
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.shell_channel != Some(channel) {
            return Ok(());
        }
        if data.contains(&3) {
            println!("FIXTURE_PTY_INTERRUPT");
        }
        if std::env::var_os("UNFOUR_BASTION_QA_KEEPALIVE").is_some() {
            // Count bytes only; never print protocol payloads or credentials.
            println!("FIXTURE_PTY_DATA {}", data.len());
        }
        // Fixed bootstraps may span several SSH packets. Route only a complete
        // bounded bootstrap; all later input belongs to its exclusive protocol.
        if !self.bootstrap.is_empty() || data.starts_with(b" sh -c '") {
            self.bootstrap.extend_from_slice(data);
            assert!(self.bootstrap.len() < 4096);
            if self.bootstrap.ends_with(b"\n") {
                let bytes = std::mem::take(&mut self.bootstrap);
                return if bytes.windows(13).any(|s| s == b"UNFOUR_RAW_V3") {
                    self.raw_terminal.feed(channel, &bytes, session)
                } else {
                    self.terminal.feed(channel, &bytes, session)
                };
            }
            return Ok(());
        }
        if self.raw_terminal.active() {
            return self.raw_terminal.feed(channel, data, session);
        }
        if self.terminal.active() {
            return self.terminal.feed(channel, data, session);
        }
        for &byte in data {
            if matches!(byte, b'\r' | b'\n') {
                if self.line == b"check" {
                    session.data(channel, b"\r\nA2_ECHO_OK\r\n".to_vec())?;
                    println!("FIXTURE_ROUNDTRIP_OK");
                } else if self.line == b"quit" {
                    session.eof(channel)?;
                    session.close(channel)?;
                    println!("FIXTURE_PTY_CLOSED");
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
    let mut config = russh::server::Config {
        keys: vec![key],
        ..Default::default()
    };
    if std::env::var_os("UNFOUR_BASTION_QA_KEEPALIVE").is_some() {
        // An unmaintained connection must expire before the UI's 130s idle
        // check. Rekey is accelerated in the server fixture only, not the app.
        config.inactivity_timeout = Some(Duration::from_secs(75));
        config.limits.rekey_time_limit = Duration::from_secs(10);
        println!("FIXTURE_KEEPALIVE_QA idle=75 rekey=10");
    }
    let config = Arc::new(config);
    let mut fixture = Fixture::default();
    let _ = tokio::time::timeout(
        Duration::from_secs(600),
        fixture.run_on_socket(config, &socket),
    )
    .await;
    Ok(())
}
