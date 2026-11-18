use super::*;
use russh_sftp::protocol::{Data, Handle, Status};

#[derive(Default)]
struct State {
    files: HashMap<String, Vec<u8>>,
    removes: Vec<String>,
    short_reads: bool,
    reject_create: bool,
    create_without_handle: bool,
    reject_write: bool,
    reject_remove: bool,
    corrupt: bool,
    hold_create: Option<Arc<tokio::sync::Notify>>,
    create_sent: Option<Arc<tokio::sync::Notify>>,
    hold_write: Option<Arc<tokio::sync::Notify>>,
    write_sent: Option<Arc<tokio::sync::Notify>>,
}
struct Server(Arc<Mutex<State>>);
fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}
impl russh_sftp::server::Handler for Server {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }
    async fn open(
        &mut self,
        id: u32,
        path: String,
        flags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        assert_eq!(
            flags.bits(),
            (OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE).bits()
        );
        assert_eq!(attrs.permissions, Some(0o600));
        assert!(path.starts_with("/tmp/unfour-a4-"));
        assert!(Uuid::parse_str(path.trim_start_matches("/tmp/unfour-a4-")).is_ok());
        let (hold, sent) = {
            let mut state = self.0.lock().unwrap();
            if state.reject_create {
                return Err(StatusCode::PermissionDenied);
            }
            assert!(!state.files.contains_key(&path));
            state.files.insert(path.clone(), Vec::new());
            if state.create_without_handle {
                return Err(StatusCode::Ok);
            }
            (state.hold_create.take(), state.create_sent.take())
        };
        if let Some(sent) = sent {
            sent.notify_one();
        }
        if let Some(hold) = hold {
            hold.notified().await;
        }
        Ok(Handle { id, handle: path })
    }
    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        assert_eq!(offset, 0);
        assert_eq!(data, FILE_CONTENT);
        let (hold, sent) = {
            let mut state = self.0.lock().unwrap();
            if state.reject_write {
                return Err(StatusCode::PermissionDenied);
            }
            let corrupt = state.corrupt;
            let bytes = state.files.get_mut(&handle).unwrap();
            bytes.extend_from_slice(&data);
            if corrupt {
                bytes.push(b'x');
            }
            (state.hold_write.take(), state.write_sent.take())
        };
        if let Some(sent) = sent {
            sent.notify_one();
        }
        if let Some(hold) = hold {
            hold.notified().await;
        }
        Ok(ok(id))
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, StatusCode> {
        assert!(len as usize <= FILE_CONTENT.len() + 1);
        let state = self.0.lock().unwrap();
        let bytes = state.files.get(&handle).unwrap();
        let offset = offset as usize;
        if offset >= bytes.len() {
            return Err(StatusCode::Eof);
        }
        let len = if state.short_reads { 1 } else { len as usize };
        Ok(Data {
            id,
            data: bytes[offset..bytes.len().min(offset + len)].to_vec(),
        })
    }
    async fn close(&mut self, id: u32, _: String) -> Result<Status, StatusCode> {
        Ok(ok(id))
    }
    async fn remove(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        let mut state = self.0.lock().unwrap();
        state.removes.push(path.clone());
        if state.reject_remove {
            return Err(StatusCode::PermissionDenied);
        }
        assert!(state.files.remove(&path).is_some());
        Ok(ok(id))
    }
}

async fn fixture(
    state: State,
) -> (
    RawSftpSession,
    Arc<Mutex<State>>,
    Control,
    watch::Sender<bool>,
    watch::Sender<bool>,
    TemporaryProbeReport,
) {
    let state = Arc::new(Mutex::new(state));
    let (client, server) = tokio::io::duplex(8192);
    russh_sftp::server::run(server, Server(state.clone())).await;
    let raw = RawSftpSession::new_with_config(
        client,
        Config {
            max_packet_len: 4096,
            max_concurrent_writes: 1,
            request_timeout_secs: 1,
        },
    );
    raw.init().await.unwrap();
    let (parent, parent_rx) = watch::channel(false);
    let (probe, probe_rx) = watch::channel(false);
    let control = Control {
        parent: parent_rx,
        probe: probe_rx,
    };
    let report = TemporaryProbeReport {
        run_id: Uuid::new_v4().to_string(),
        kind: TemporaryProbeKind::Sftp,
        status: "running".into(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
        checks: Vec::new(),
        probe_file: Some(format!("/tmp/unfour-a4-{}", Uuid::new_v4())),
        cleanup: Some("not-created".into()),
    };
    (raw, state, control, parent, probe, report)
}

#[tokio::test]
async fn exclusive_roundtrip_accepts_short_reads_and_removes_only_owned_file() {
    let (raw, state, control, _parent, _probe, mut report) = fixture(State {
        short_reads: true,
        ..Default::default()
    })
    .await;
    state
        .lock()
        .unwrap()
        .files
        .insert("/untouched".into(), b"original".to_vec());
    sftp_roundtrip(&mut report, &raw, &control).await;
    assert!(report.checks.iter().all(|check| check.status == "pass"));
    assert_eq!(report.cleanup.as_deref(), Some("removed"));
    let state = state.lock().unwrap();
    assert_eq!(state.files.len(), 1);
    assert_eq!(state.files["/untouched"], b"original");
    assert_eq!(state.removes, vec![report.probe_file.unwrap()]);
}

#[tokio::test]
async fn refused_create_never_removes_and_failed_write_or_integrity_still_cleans() {
    for (state, code, cleanup) in [
        (
            State {
                reject_create: true,
                ..Default::default()
            },
            "SFTP_CREATE_FAILED",
            "not-created",
        ),
        (
            State {
                reject_write: true,
                ..Default::default()
            },
            "SFTP_WRITE_FAILED",
            "removed",
        ),
        (
            State {
                corrupt: true,
                ..Default::default()
            },
            "SFTP_MISMATCH",
            "removed",
        ),
    ] {
        let (raw, state, control, _parent, _probe, mut report) = fixture(state).await;
        sftp_roundtrip(&mut report, &raw, &control).await;
        assert_eq!(report.checks[0].code, code);
        assert_eq!(report.cleanup.as_deref(), Some(cleanup));
        let state = state.lock().unwrap();
        assert!(state.files.is_empty());
        assert_eq!(state.removes.len(), usize::from(cleanup == "removed"));
    }
}

#[tokio::test]
async fn cancellation_during_create_waits_for_ownership_before_cleanup() {
    let hold = Arc::new(tokio::sync::Notify::new());
    let sent = Arc::new(tokio::sync::Notify::new());
    let (raw, state, control, _parent, probe, mut report) = fixture(State {
        hold_create: Some(hold.clone()),
        create_sent: Some(sent.clone()),
        ..Default::default()
    })
    .await;
    let run = tokio::spawn(async move {
        sftp_roundtrip(&mut report, &raw, &control).await;
        report
    });
    sent.notified().await;
    probe.send_replace(true);
    hold.notify_one();
    let report = run.await.unwrap();
    assert_eq!(report.checks[0].status, "cancelled");
    assert_eq!(report.cleanup.as_deref(), Some("removed"));
    assert!(state.lock().unwrap().files.is_empty());
}

#[tokio::test]
async fn cancellation_during_write_still_attempts_cleanup() {
    let hold = Arc::new(tokio::sync::Notify::new());
    let sent = Arc::new(tokio::sync::Notify::new());
    let (raw, state, control, _parent, probe, mut report) = fixture(State {
        hold_write: Some(hold.clone()),
        write_sent: Some(sent.clone()),
        ..Default::default()
    })
    .await;
    let run = tokio::spawn(async move {
        sftp_roundtrip(&mut report, &raw, &control).await;
        report
    });
    sent.notified().await;
    probe.send_replace(true);
    hold.notify_one();
    let report = run.await.unwrap();
    assert_eq!(report.checks[0].status, "cancelled");
    assert_eq!(report.cleanup.as_deref(), Some("removed"));
    assert!(state.lock().unwrap().files.is_empty());
}

#[tokio::test(start_paused = true)]
async fn lost_create_ack_cannot_claim_cleanup_or_delete_unproven_ownership() {
    let hold = Arc::new(tokio::sync::Notify::new());
    let (raw, state, control, _parent, _probe, mut report) = fixture(State {
        hold_create: Some(hold.clone()),
        ..Default::default()
    })
    .await;
    sftp_roundtrip(&mut report, &raw, &control).await;
    assert_eq!(report.checks[0].status, "inconclusive");
    assert_eq!(report.checks[0].code, "TIMEOUT");
    assert_eq!(report.cleanup.as_deref(), Some("unverified"));
    assert_eq!(state.lock().unwrap().files.len(), 1);
    assert!(state.lock().unwrap().removes.is_empty());
    hold.notify_one();
}

#[tokio::test]
async fn create_ok_without_handle_is_unverified_and_cannot_authorize_delete() {
    let (raw, state, control, _parent, _probe, mut report) = fixture(State {
        create_without_handle: true,
        ..Default::default()
    })
    .await;
    sftp_roundtrip(&mut report, &raw, &control).await;
    assert_eq!(report.checks[0].code, "SFTP_PROTOCOL_ERROR");
    assert_eq!(report.checks[0].status, "inconclusive");
    assert_eq!(report.cleanup.as_deref(), Some("unverified"));
    assert_eq!(state.lock().unwrap().files.len(), 1);
    assert!(state.lock().unwrap().removes.is_empty());
}

#[tokio::test]
async fn cleanup_refusal_keeps_unverified_even_after_roundtrip_success() {
    let (raw, state, control, _parent, _probe, mut report) = fixture(State {
        reject_remove: true,
        ..Default::default()
    })
    .await;
    sftp_roundtrip(&mut report, &raw, &control).await;
    assert_eq!(report.checks[0].status, "pass");
    assert_eq!(report.checks[1].code, "SFTP_CLEANUP_FAILED");
    assert_eq!(report.cleanup.as_deref(), Some("unverified"));
    assert_eq!(state.lock().unwrap().files.len(), 1);
}

#[test]
fn remote_error_text_never_enters_report() {
    let error = SftpError::Status(Status {
        id: 5,
        status_code: StatusCode::PermissionDenied,
        error_message: "secret-sentinel".into(),
        language_tag: "secret-sentinel".into(),
    });
    let result = check(
        "sftp-roundtrip",
        sftp_error(error, "SFTP_WRITE_FAILED"),
        None,
        None,
    );
    assert_eq!(result.code, "SFTP_WRITE_FAILED");
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("secret-sentinel"));
    assert_eq!(
        check(
            "sftp-roundtrip",
            sftp_error(SftpError::Timeout, "SFTP_WRITE_FAILED"),
            None,
            None
        )
        .status,
        "inconclusive"
    );
}
