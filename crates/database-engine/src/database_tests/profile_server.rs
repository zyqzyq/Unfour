//! Minimal loopback PostgreSQL protocol fixture. Exercises sqlx connect/probe,
//! not PostgreSQL metadata correctness; no installed or developer DB is touched.
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub(super) struct ProfileServer {
    pub port: u16,
    pub version: Arc<Mutex<String>>,
    pub queries: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl ProfileServer {
    pub async fn start(version: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let version = Arc::new(Mutex::new(version.to_string()));
        let queries = Arc::new(Mutex::new(Vec::new()));
        let state = (version.clone(), queries.clone());
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let state = state.clone();
                clients.spawn(async move {
                    let _ = serve(stream, state.0, state.1).await;
                });
            }
        });
        Self {
            port,
            version,
            queries,
            task,
        }
    }
}

impl Drop for ProfileServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn send(stream: &mut TcpStream, tag: u8, body: &[u8]) -> std::io::Result<()> {
    let mut message = vec![tag];
    message.extend(((body.len() + 4) as u32).to_be_bytes());
    message.extend(body);
    stream.write_all(&message).await
}

async fn serve(
    mut stream: TcpStream,
    version: Arc<Mutex<String>>,
    queries: Arc<Mutex<Vec<String>>>,
) -> std::io::Result<()> {
    loop {
        let length = stream.read_u32().await?;
        let mut body = vec![0; (length - 4) as usize];
        stream.read_exact(&mut body).await?;
        if body == 80877103u32.to_be_bytes() {
            // SSLRequest
            stream.write_all(b"N").await?;
        } else {
            break;
        }
    }
    send(&mut stream, b'R', &0u32.to_be_bytes()).await?;
    send(&mut stream, b'S', b"server_version\x0016.4\x00").await?;
    send(&mut stream, b'S', b"client_encoding\x00UTF8\x00").await?;
    send(&mut stream, b'Z', b"I").await?;
    loop {
        let tag = stream.read_u8().await?;
        let length = stream.read_u32().await?;
        let mut body = vec![0; (length - 4) as usize];
        stream.read_exact(&mut body).await?;
        match tag {
            b'P' => {
                let sql = body.split(|b| *b == 0).nth(1).unwrap();
                queries
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(sql).into_owned());
                send(&mut stream, b'1', &[]).await?;
            }
            b'D' => {
                send(&mut stream, b't', &0u16.to_be_bytes()).await?;
                let mut description = 1u16.to_be_bytes().to_vec();
                description.extend(b"version\0");
                description.extend(0u32.to_be_bytes()); // table
                description.extend(0u16.to_be_bytes()); // attribute
                description.extend(25u32.to_be_bytes()); // text OID
                description.extend((-1i16).to_be_bytes());
                description.extend((-1i32).to_be_bytes());
                description.extend(0u16.to_be_bytes());
                send(&mut stream, b'T', &description).await?;
            }
            b'B' => send(&mut stream, b'2', &[]).await?,
            b'E' => {
                let value = version.lock().unwrap().clone();
                if value == "probe-hang" {
                    std::future::pending::<()>().await;
                } else if value == "probe-error" {
                    send(
                        &mut stream,
                        b'E',
                        b"SERROR\0VERROR\0C42501\0Mprobe denied\0\0",
                    )
                    .await?;
                } else {
                    let mut row = 1u16.to_be_bytes().to_vec();
                    row.extend((value.len() as u32).to_be_bytes());
                    row.extend(value.as_bytes());
                    send(&mut stream, b'D', &row).await?;
                    send(&mut stream, b'C', b"SELECT 1\0").await?;
                }
            }
            b'S' => send(&mut stream, b'Z', b"I").await?,
            b'C' => send(&mut stream, b'3', &[]).await?,
            b'X' => return Ok(()),
            other => panic!("unexpected PostgreSQL fixture message: {other}"),
        }
    }
}
