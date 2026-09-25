//! Minimal loopback PostgreSQL protocol fixture. Exercises sqlx connect/probe,
//! not PostgreSQL metadata correctness; no installed or developer DB is touched.
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub(super) struct ProfileServer {
    pub port: u16,
    pub version: Arc<Mutex<String>>,
    pub query_catalogs: Arc<Mutex<Vec<(String, String)>>>,
    pub catalogs: Arc<Mutex<Vec<String>>>,
    pub queries: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl ProfileServer {
    pub async fn start(version: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let version = Arc::new(Mutex::new(version.to_string()));
        let queries = Arc::new(Mutex::new(Vec::new()));
        let catalogs = Arc::new(Mutex::new(Vec::new()));
        let query_catalogs = Arc::new(Mutex::new(Vec::new()));
        let state = (
            version.clone(),
            queries.clone(),
            catalogs.clone(),
            query_catalogs.clone(),
        );
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let state = state.clone();
                clients.spawn(async move {
                    let _ = serve(stream, state.0, state.1, state.2, state.3).await;
                });
            }
        });
        Self {
            port,
            version,
            queries,
            catalogs,
            query_catalogs,
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
    catalogs: Arc<Mutex<Vec<String>>>,
    query_catalogs: Arc<Mutex<Vec<(String, String)>>>,
) -> std::io::Result<()> {
    let mut catalog = String::new();
    loop {
        let length = stream.read_u32().await?;
        let mut body = vec![0; (length - 4) as usize];
        stream.read_exact(&mut body).await?;
        if body == 80877103u32.to_be_bytes() {
            // SSLRequest
            stream.write_all(b"N").await?;
        } else {
            let fields: Vec<_> = body[4..].split(|b| *b == 0).collect();
            for pair in fields.chunks_exact(2) {
                if pair[0] == b"database" {
                    catalog = String::from_utf8_lossy(pair[1]).into_owned();
                    catalogs
                        .lock()
                        .unwrap()
                        .push(String::from_utf8_lossy(pair[1]).into_owned());
                }
            }
            break;
        }
    }
    send(&mut stream, b'R', &0u32.to_be_bytes()).await?;
    send(&mut stream, b'S', b"server_version\x0016.4\x00").await?;
    send(&mut stream, b'S', b"client_encoding\x00UTF8\x00").await?;
    send(&mut stream, b'Z', b"I").await?;
    let mut sql = String::new();
    loop {
        let tag = stream.read_u8().await?;
        let length = stream.read_u32().await?;
        let mut body = vec![0; (length - 4) as usize];
        stream.read_exact(&mut body).await?;
        match tag {
            b'Q' => {
                let query = String::from_utf8_lossy(&body[..body.len() - 1]).into_owned();
                queries.lock().unwrap().push(query.clone());
                query_catalogs
                    .lock()
                    .unwrap()
                    .push((catalog.clone(), query.clone()));
                if query.starts_with("SET search_path") {
                    send(&mut stream, b'C', b"SET\0").await?;
                } else if query.starts_with("UPDATE ") {
                    send(&mut stream, b'C', b"UPDATE 1\0").await?;
                } else {
                    let version = version.lock().unwrap().clone();
                    let (columns, rows) = super::opengauss_fixture::response(&query, &version);
                    let mut description = (columns.len() as u16).to_be_bytes().to_vec();
                    for (name, oid) in columns {
                        description.extend(name.as_bytes());
                        description.push(0);
                        description.extend(0u32.to_be_bytes());
                        description.extend(0u16.to_be_bytes());
                        description.extend(oid.to_be_bytes());
                        description.extend((-1i16).to_be_bytes());
                        description.extend((-1i32).to_be_bytes());
                        description.extend(0u16.to_be_bytes());
                    }
                    send(&mut stream, b'T', &description).await?;
                    for values in rows {
                        let mut row = (values.len() as u16).to_be_bytes().to_vec();
                        for value in values {
                            if let Some(value) = value {
                                row.extend((value.len() as u32).to_be_bytes());
                                row.extend(value);
                            } else {
                                row.extend((-1i32).to_be_bytes());
                            }
                        }
                        send(&mut stream, b'D', &row).await?;
                    }
                    send(&mut stream, b'C', b"SELECT 1\0").await?;
                }
                send(&mut stream, b'Z', b"I").await?;
            }
            b'P' => {
                sql = String::from_utf8_lossy(body.split(|b| *b == 0).nth(1).unwrap()).into_owned();
                queries.lock().unwrap().push(sql.clone());
                query_catalogs
                    .lock()
                    .unwrap()
                    .push((catalog.clone(), sql.clone()));
                send(&mut stream, b'1', &[]).await?;
            }
            b'D' => {
                let fixture = version.lock().unwrap().contains("metadata fixture");
                let params = if fixture {
                    sql.matches('$').count() as u16
                } else {
                    0
                };
                let mut parameters = params.to_be_bytes().to_vec();
                for _ in 0..params {
                    parameters.extend(25u32.to_be_bytes());
                }
                send(&mut stream, b't', &parameters).await?;
                let value = version.lock().unwrap().clone();
                let (columns, _) = super::opengauss_fixture::response(&sql, &value);
                let mut description = (columns.len() as u16).to_be_bytes().to_vec();
                for (name, oid) in columns {
                    description.extend(name.as_bytes());
                    description.push(0);
                    description.extend(0u32.to_be_bytes());
                    description.extend(0u16.to_be_bytes());
                    description.extend(oid.to_be_bytes());
                    description.extend((-1i16).to_be_bytes());
                    description.extend((-1i32).to_be_bytes());
                    description.extend(0u16.to_be_bytes());
                }
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
                    let (_, rows) = super::opengauss_fixture::response(&sql, &value);
                    for values in rows {
                        let mut row = (values.len() as u16).to_be_bytes().to_vec();
                        for value in values {
                            if let Some(value) = value {
                                row.extend((value.len() as u32).to_be_bytes());
                                row.extend(value);
                            } else {
                                row.extend((-1i32).to_be_bytes());
                            }
                        }
                        send(&mut stream, b'D', &row).await?;
                    }
                    let command: &[u8] = if sql.starts_with("UPDATE ") {
                        b"UPDATE 1\0"
                    } else if sql.starts_with("INSERT INTO ") {
                        b"INSERT 0 1\0"
                    } else if sql.starts_with("DELETE FROM ") {
                        b"DELETE 1\0"
                    } else {
                        b"SELECT 1\0"
                    };
                    send(&mut stream, b'C', command).await?;
                }
            }
            b'S' => send(&mut stream, b'Z', b"I").await?,
            b'C' => send(&mut stream, b'3', &[]).await?,
            b'X' => return Ok(()),
            other => panic!("unexpected PostgreSQL fixture message: {other}"),
        }
    }
}
