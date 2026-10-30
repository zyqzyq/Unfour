//! In-memory SFTP protocol tests, not a real Linux/SSH server verification.
use super::*;
use russh_sftp::protocol::{Attrs, Data, FileAttributes, Handle, OpenFlags, Status, StatusCode};

#[derive(Default)]
struct Probe {
    opens: usize,
    closes: usize,
    bytes: usize,
    unexpected: usize,
}

struct Server {
    bytes: Vec<u8>,
    advertised_size: u64,
    permissions: u32,
    open_error: Option<StatusCode>,
    read_error: Option<StatusCode>,
    probe: Arc<Mutex<Probe>>,
    reading: Option<Arc<tokio::sync::Notify>>,
    resume: Option<Arc<tokio::sync::Notify>>,
}

impl Server {
    fn text(bytes: Vec<u8>) -> Self {
        Self {
            advertised_size: bytes.len() as u64,
            bytes,
            permissions: 0o100644,
            open_error: None,
            read_error: None,
            probe: Arc::new(Mutex::new(Probe::default())),
            reading: None,
            resume: None,
        }
    }
    fn attrs(&self, id: u32) -> Attrs {
        let mut attrs = FileAttributes::empty();
        attrs.size = Some(self.advertised_size);
        attrs.permissions = Some(self.permissions);
        Attrs { id, attrs }
    }
}

impl russh_sftp::server::Handler for Server {
    type Error = StatusCode;
    fn unimplemented(&self) -> Self::Error {
        self.probe.lock().unwrap().unexpected += 1;
        StatusCode::OpUnsupported
    }
    async fn lstat(&mut self, id: u32, _: String) -> Result<Attrs, Self::Error> {
        Ok(self.attrs(id))
    }
    async fn fstat(&mut self, id: u32, _: String) -> Result<Attrs, Self::Error> {
        Ok(self.attrs(id))
    }
    async fn open(
        &mut self,
        id: u32,
        _: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        assert_eq!(
            flags.bits(),
            OpenFlags::READ.bits(),
            "preview must never request write/create flags"
        );
        self.probe.lock().unwrap().opens += 1;
        if let Some(error) = self.open_error {
            return Err(error);
        }
        Ok(Handle {
            id,
            handle: "fixture".into(),
        })
    }
    async fn read(
        &mut self,
        id: u32,
        _: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        if let Some(reading) = self.reading.take() {
            reading.notify_one();
            self.resume.as_ref().unwrap().notified().await;
        }
        if let Some(error) = self.read_error {
            return Err(error);
        }
        let start = offset as usize;
        if start >= self.bytes.len() {
            return Err(StatusCode::Eof);
        }
        let end = (start + len as usize).min(self.bytes.len());
        self.probe.lock().unwrap().bytes += end - start;
        Ok(Data {
            id,
            data: self.bytes[start..end].to_vec(),
        })
    }
    async fn close(&mut self, id: u32, _: String) -> Result<Status, Self::Error> {
        self.probe.lock().unwrap().closes += 1;
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: String::new(),
        })
    }
}

async fn fixture(server: Server) -> (SshService, SftpPathInput, tokio::task::JoinHandle<()>) {
    let (client, mut client_proxy) = tokio::io::duplex(128 * 1024);
    let (mut server_proxy, stream) = tokio::io::duplex(128 * 1024);
    russh_sftp::server::run(stream, server).await;
    // The server helper spawns internally. Own the proxy task so abort really
    // closes the transport instead of aborting an already-finished wrapper.
    let task = tokio::spawn(async move {
        let _ = tokio::io::copy_bidirectional(&mut client_proxy, &mut server_proxy).await;
    });
    let sftp = SftpSession::new(client).await.unwrap();
    sftp.set_timeout(1);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_lazy("sqlite::memory:")
        .unwrap();
    // No schema: any accidental persistence operation would fail this fixture.
    let service = SshService::new(
        LocalDb::from_pool(pool),
        SecretStore::in_memory("preview-test"),
    );
    let input = SftpPathInput {
        workspace_id: Uuid::new_v4().to_string(),
        session_id: Uuid::new_v4().to_string(),
        path: "/fixture.txt".into(),
    };
    let summary: SshSessionSummary = serde_json::from_value(serde_json::json!({
        "workspaceId": input.workspace_id, "sessionId": input.session_id,
        "connectionId": Uuid::new_v4().to_string(), "status": "connected", "reconnectAttempt": 0,
        "authKind": "password", "host": "fixture", "username": "fixture", "cols": 80, "rows": 24,
        "createdAt": "", "updatedAt": ""
    }))
    .unwrap();
    service.sessions.lock().unwrap().insert(
        input.session_id.clone(),
        SshSessionState {
            summary,
            events: vec![],
            command_line: SshCommandLineTracker::new(),
            pending_output: String::new(),
            history_flush_running: false,
            intentional_close: false,
            native_handle: None,
            cancel_tx: None,
            sftp: Some(SftpChannelState {
                session: Arc::new(sftp),
                home_path: "/".into(),
                generation: 1,
            }),
            sftp_generation: 1,
        },
    );
    (service, input, task)
}

#[tokio::test]
async fn sftp_preview_protocol_is_read_only_and_does_not_create_transfers_or_events() {
    let server = Server::text("\u{feff}hello 世界".as_bytes().to_vec());
    let probe = server.probe.clone();
    let (service, input, task) = fixture(server).await;
    let preview = service.sftp_preview_text(input.clone()).await.unwrap();
    assert_eq!(preview.content, "hello 世界");
    assert!(service.transfers.lock().unwrap().is_empty());
    let sessions = service.sessions.lock().unwrap();
    assert!(sessions[&input.session_id].events.is_empty());
    assert!(sessions[&input.session_id].pending_output.is_empty());
    let probe = probe.lock().unwrap();
    assert_eq!((probe.opens, probe.closes, probe.unexpected), (1, 1, 0));
    assert_eq!(probe.bytes, preview.size as usize);
    task.abort();
}

#[tokio::test]
async fn sftp_preview_protocol_limits_reads_even_with_incorrect_size() {
    for advertised_size in [0, (MAX_PREVIEW_BYTES * 2) as u64] {
        let mut server = Server::text(vec![b'x'; MAX_PREVIEW_BYTES * 2]);
        server.advertised_size = advertised_size;
        let probe = server.probe.clone();
        let (service, input, task) = fixture(server).await;
        assert_eq!(
            service.sftp_preview_text(input).await.err().unwrap().code(),
            "SFTP_PREVIEW_TOO_LARGE"
        );
        let probe = probe.lock().unwrap();
        assert_eq!(
            probe.bytes,
            if advertised_size == 0 {
                MAX_PREVIEW_BYTES + 1
            } else {
                0
            }
        );
        assert_eq!(probe.opens, probe.closes);
        assert_eq!(probe.unexpected, 0);
        task.abort();
    }
}

#[tokio::test]
async fn sftp_preview_protocol_classifies_open_and_read_failures_and_closes_handles() {
    for status in [StatusCode::PermissionDenied, StatusCode::ConnectionLost] {
        for at_open in [true, false] {
            let mut server = Server::text(b"secret".to_vec());
            if at_open {
                server.open_error = Some(status);
            } else {
                server.read_error = Some(status);
            }
            let probe = server.probe.clone();
            let (service, input, task) = fixture(server).await;
            let error = service.sftp_preview_text(input).await.err().unwrap();
            assert_eq!(
                error.code(),
                if status == StatusCode::PermissionDenied {
                    "SFTP_PREVIEW_PERMISSION_DENIED"
                } else {
                    "SFTP_PREVIEW_DISCONNECTED"
                }
            );
            assert!(!error.to_string().contains("secret"));
            assert_eq!(probe.lock().unwrap().closes, usize::from(!at_open));
            task.abort();
        }
    }
}

#[tokio::test]
async fn sftp_preview_rejects_non_regular_files_and_wrong_workspace_before_open() {
    for permissions in [0o040755, 0o120777, 0o010644] {
        let mut server = Server::text(vec![]);
        server.permissions = permissions;
        let probe = server.probe.clone();
        let (service, input, task) = fixture(server).await;
        let mut wrong = input.clone();
        wrong.workspace_id = Uuid::new_v4().to_string();
        assert!(service.sftp_preview_text(wrong).await.is_err());
        assert_eq!(
            service.sftp_preview_text(input).await.err().unwrap().code(),
            "SFTP_PREVIEW_NOT_FILE"
        );
        assert_eq!(probe.lock().unwrap().opens, 0);
        task.abort();
    }
}

#[tokio::test]
async fn sftp_preview_rejects_result_after_session_generation_changes() {
    let mut server = Server::text(b"stale secret".to_vec());
    let reading = Arc::new(tokio::sync::Notify::new());
    let resume = Arc::new(tokio::sync::Notify::new());
    server.reading = Some(reading.clone());
    server.resume = Some(resume.clone());
    let (service, input, task) = fixture(server).await;
    let future = service.sftp_preview_text(input.clone());
    let change_session = async {
        reading.notified().await;
        service
            .sessions
            .lock()
            .unwrap()
            .get_mut(&input.session_id)
            .unwrap()
            .sftp
            .as_mut()
            .unwrap()
            .generation += 1;
        resume.notify_one();
    };
    let (result, ()) = tokio::join!(future, change_session);
    assert_eq!(result.err().unwrap().code(), "SFTP_PREVIEW_DISCONNECTED");
    task.abort();
}

#[tokio::test]
async fn sftp_preview_reports_transport_disconnect_during_read() {
    let mut server = Server::text(b"unread secret".to_vec());
    let reading = Arc::new(tokio::sync::Notify::new());
    server.reading = Some(reading.clone());
    server.resume = Some(Arc::new(tokio::sync::Notify::new()));
    let (service, input, task) = fixture(server).await;
    let disconnect = async {
        reading.notified().await;
        task.abort();
    };
    let (result, ()) = tokio::join!(service.sftp_preview_text(input), disconnect);
    assert_eq!(result.err().unwrap().code(), "SFTP_PREVIEW_DISCONNECTED");
}
