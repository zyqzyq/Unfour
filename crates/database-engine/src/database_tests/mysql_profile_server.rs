//! Minimal loopback MySQL protocol fixture. Completes handshake so a pool can
//! open, then fails `SELECT VERSION()`. Not a MySQL server.
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub(super) struct MysqlProfileServer {
    pub port: u16,
    pub queries: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl MysqlProfileServer {
    pub async fn start() -> Self {
        Self::start_with(false).await
    }

    /// Handshake succeeds, then `SELECT VERSION()` is accepted and never answered.
    pub async fn start_hanging_probe() -> Self {
        Self::start_with(true).await
    }

    async fn start_with(hang_version_probe: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let queries = Arc::new(Mutex::new(Vec::new()));
        let recorded = queries.clone();
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let queries = recorded.clone();
                clients.spawn(async move {
                    let _ = serve(stream, queries, hang_version_probe).await;
                });
            }
        });
        Self {
            port,
            queries,
            task,
        }
    }
}

impl Drop for MysqlProfileServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(
    mut stream: TcpStream,
    queries: Arc<Mutex<Vec<String>>>,
    hang_version_probe: bool,
) -> std::io::Result<()> {
    send_packet(&mut stream, 0, &handshake()).await?;
    let _client_handshake = read_packet(&mut stream).await?;
    send_packet(&mut stream, 2, &ok_packet()).await?;
    loop {
        let packet = read_packet(&mut stream).await?;
        if packet.is_empty() {
            continue;
        }
        match packet[0] {
            0x01 => return Ok(()),
            0x0e => send_packet(&mut stream, 1, &ok_packet()).await?,
            0x03 | 0x16 => {
                let sql = String::from_utf8_lossy(&packet[1..]).into_owned();
                let version_probe = sql.trim().eq_ignore_ascii_case("SELECT VERSION()");
                queries.lock().unwrap().push(sql.clone());
                if version_probe && hang_version_probe {
                    std::future::pending::<()>().await;
                } else if let Some(spec) = super::result_cap::result_fixture(&sql) {
                    let mut seq = 1u8;
                    send_text_results(&mut stream, &mut seq, &sql, &spec).await?;
                } else if version_probe {
                    send_packet(&mut stream, 1, &err_packet("probe denied")).await?;
                } else {
                    send_packet(&mut stream, 1, &ok_packet()).await?;
                }
            }
            _ => send_packet(&mut stream, 1, &err_packet("unsupported")).await?,
        }
    }
}

fn handshake() -> Vec<u8> {
    let caps: u32 = 1 // MYSQL
        | 2 // FOUND_ROWS
        | 8 // CONNECT_WITH_DB
        | 256 // IGNORE_SPACE
        | 512 // PROTOCOL_41
        | 8192 // TRANSACTIONS
        | (1 << 16) // MULTI_STATEMENTS
        | (1 << 17) // MULTI_RESULTS
        | (1 << 18) // PS_MULTI_RESULTS
        | (1 << 24); // DEPRECATE_EOF
    let mut body = Vec::new();
    body.push(10);
    body.extend(b"8.0.36\0");
    body.extend(&1u32.to_le_bytes());
    body.extend(b"12345678");
    body.push(0);
    body.extend(&(caps as u16).to_le_bytes());
    body.push(45);
    body.extend(&2u16.to_le_bytes());
    body.extend(&((caps >> 16) as u16).to_le_bytes());
    body.push(0);
    body.extend(&[0u8; 10]);
    body
}

fn ok_packet() -> [u8; 7] {
    [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00]
}

fn err_packet(message: &str) -> Vec<u8> {
    let mut body = vec![0xff, 0x51, 0x04, b'#'];
    body.extend(b"HY000");
    body.extend(message.as_bytes());
    body
}

async fn send_packet(stream: &mut TcpStream, seq: u8, payload: &[u8]) -> std::io::Result<()> {
    let len = payload.len() as u32;
    let mut message = Vec::with_capacity(4 + payload.len());
    message.extend_from_slice(&len.to_le_bytes()[..3]);
    message.push(seq);
    message.extend_from_slice(payload);
    stream.write_all(&message).await
}

fn lenenc_str(value: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + value.len());
    body.push(value.len() as u8);
    body.extend(value.as_bytes());
    body
}

fn column_definition(name: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(lenenc_str("def"));
    body.extend(lenenc_str(""));
    body.extend(lenenc_str(""));
    body.extend(lenenc_str(""));
    body.extend(lenenc_str(name));
    body.extend(lenenc_str(name));
    body.push(0x0c);
    body.extend(&45u16.to_le_bytes());
    body.extend(&255u32.to_le_bytes());
    body.push(0xfd);
    body.extend(&0u16.to_le_bytes());
    body.push(0);
    body.extend(&0u16.to_le_bytes());
    body
}

fn text_result_row(value: &str) -> Vec<u8> {
    lenenc_str(value)
}

/// With CLIENT_DEPRECATE_EOF, sqlx ends a text result set on a short 0xFE
/// packet and decodes it as an OK packet. A 0x00 header is treated as a row.
/// The payload must stay under 9 bytes or it is also treated as a row.
fn result_eof(affected: u8, more: bool) -> Vec<u8> {
    let status: u16 = 0x0002 | if more { 0x0008 } else { 0 };
    let mut body = vec![0xfe, affected, 0x00];
    body.extend(status.to_le_bytes());
    body.extend(0u16.to_le_bytes());
    body
}

async fn send_next(stream: &mut TcpStream, seq: &mut u8, payload: &[u8]) -> std::io::Result<()> {
    send_packet(stream, *seq, payload).await?;
    *seq = seq.wrapping_add(1);
    Ok(())
}

async fn send_text_results(
    stream: &mut TcpStream,
    seq: &mut u8,
    sql: &str,
    spec: &super::result_cap::ResultFixture,
) -> std::io::Result<()> {
    let affected =
        if sql.trim_start().len() >= 6 && sql.trim_start()[..6].eq_ignore_ascii_case("insert") {
            u8::try_from(spec.affected).unwrap_or(0)
        } else {
            0
        };
    for set in 0..spec.sets {
        send_next(stream, seq, &[1]).await?;
        send_next(stream, seq, &column_definition("n")).await?;
        for index in 1..=spec.rows {
            send_next(stream, seq, &text_result_row(&index.to_string())).await?;
        }
        let more = set + 1 < spec.sets;
        // sqlx currently reports rows_affected as 0 for this terminator.
        send_next(
            stream,
            seq,
            &result_eof(if set == 0 { affected } else { 0 }, more),
        )
        .await?;
    }
    Ok(())
}

async fn read_packet(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    let len = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}
