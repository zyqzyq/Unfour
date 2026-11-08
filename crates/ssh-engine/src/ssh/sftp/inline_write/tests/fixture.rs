use super::*;
use crate::ssh::sftp::inline_session::POSIX_RENAME;
use russh_sftp::protocol::{Attrs, Handle, Packet, Status, Version};

pub(super) type Gate = (Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>);
#[derive(Default)]
pub(super) struct State {
    pub files: HashMap<String, Vec<u8>>,
    pub flags: HashMap<String, OpenFlags>,
    pub fail_write: bool,
    pub fail_after_writes: Option<usize>,
    pub fail_commit: bool,
    pub fail_close: bool,
    pub fail_metadata: bool,
    pub fail_sync: bool,
    pub fail_remove: bool,
    pub race_create: bool,
    pub symlink: bool,
    pub no_atomic: bool,
    pub atomic_version: Option<String>,
    pub writes: usize,
    pub closes: usize,
    pub commits: usize,
    pub removes: Vec<String>,
    pub renames: Vec<(String, String)>,
    pub hold_write: Option<Gate>,
    pub hold_open_after: Option<Gate>,
    pub hold_commit_before: Option<Gate>,
    pub hold_commit_after: Option<Gate>,
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
async fn hold(gate: Option<Gate>) {
    if let Some((started, resume)) = gate {
        started.notify_one();
        resume.notified().await;
    }
}
impl russh_sftp::server::Handler for Server {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }
    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> Result<Version, StatusCode> {
        let mut version = Version::new();
        let state = self.0.lock().unwrap();
        if !state.no_atomic {
            version.extensions.insert(
                POSIX_RENAME.into(),
                state.atomic_version.clone().unwrap_or_else(|| "1".into()),
            );
        }
        version
            .extensions
            .insert(russh_sftp::extensions::FSYNC.into(), "1".into());
        Ok(version)
    }
    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        let state = self.0.lock().unwrap();
        let data = state.files.get(&path).ok_or(StatusCode::NoSuchFile)?;
        let mut attrs = FileAttributes::empty();
        attrs.size = Some(data.len() as u64);
        attrs.permissions = Some(if state.symlink { 0o120777 } else { 0o100640 });
        Ok(Attrs { id, attrs })
    }
    async fn open(
        &mut self,
        id: u32,
        path: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        let gate = {
            let mut state = self.0.lock().unwrap();
            assert!(!flags.contains(OpenFlags::TRUNCATE));
            if flags.contains(OpenFlags::EXCLUDE) && state.files.contains_key(&path) {
                return Err(StatusCode::Failure);
            }
            state.files.entry(path.clone()).or_default();
            state.flags.insert(path.clone(), flags);
            state.hold_open_after.take()
        };
        hold(gate).await;
        Ok(Handle { id, handle: path })
    }
    async fn write(
        &mut self,
        id: u32,
        path: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        let gate = self.0.lock().unwrap().hold_write.take();
        hold(gate).await;
        let mut state = self.0.lock().unwrap();
        state.writes += 1;
        if state.fail_write
            || state
                .fail_after_writes
                .is_some_and(|limit| state.writes > limit)
        {
            return Err(StatusCode::PermissionDenied);
        }
        let append = state.flags[&path].contains(OpenFlags::APPEND);
        let file = state.files.get_mut(&path).ok_or(StatusCode::NoSuchFile)?;
        let offset = if append { file.len() } else { offset as usize };
        file.resize(file.len().max(offset + data.len()), 0);
        file[offset..offset + data.len()].copy_from_slice(&data);
        Ok(ok(id))
    }
    async fn close(&mut self, id: u32, _: String) -> Result<Status, StatusCode> {
        let mut state = self.0.lock().unwrap();
        state.closes += 1;
        if state.fail_close {
            return Err(StatusCode::Failure);
        }
        Ok(ok(id))
    }
    async fn fsetstat(
        &mut self,
        id: u32,
        _: String,
        attrs: FileAttributes,
    ) -> Result<Status, StatusCode> {
        assert_eq!(attrs.permissions, Some(0o100640));
        if self.0.lock().unwrap().fail_metadata {
            return Err(StatusCode::PermissionDenied);
        }
        Ok(ok(id))
    }
    async fn remove(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        let mut state = self.0.lock().unwrap();
        // Cleanup is never allowed to touch the destination or a backup.
        assert!(path.contains("/.unfour-inline-"));
        state.removes.push(path.clone());
        if state.fail_remove {
            return Err(StatusCode::PermissionDenied);
        }
        state.files.remove(&path).ok_or(StatusCode::NoSuchFile)?;
        Ok(ok(id))
    }
    async fn rename(&mut self, id: u32, old: String, new: String) -> Result<Status, StatusCode> {
        let mut state = self.0.lock().unwrap();
        assert!(old.contains("/.unfour-inline-"));
        state.renames.push((old.clone(), new.clone()));
        if state.race_create {
            state.files.insert(new.clone(), b"concurrent".to_vec());
        }
        if state.fail_commit || state.files.contains_key(&new) {
            return Err(StatusCode::Failure);
        }
        let data = state.files.remove(&old).ok_or(StatusCode::NoSuchFile)?;
        state.files.insert(new, data);
        Ok(ok(id))
    }
    async fn extended(
        &mut self,
        id: u32,
        request: String,
        data: Vec<u8>,
    ) -> Result<Packet, StatusCode> {
        if request == russh_sftp::extensions::FSYNC {
            if self.0.lock().unwrap().fail_sync {
                return Err(StatusCode::Failure);
            }
            return Ok(ok(id).into());
        }
        assert_eq!(request, POSIX_RENAME);
        let gate = self.0.lock().unwrap().hold_commit_before.take();
        hold(gate).await;
        let mut data = data.as_slice();
        let mut string = || {
            let size = u32::from_be_bytes(data[..4].try_into().unwrap()) as usize;
            let value = String::from_utf8(data[4..4 + size].to_vec()).unwrap();
            data = &data[4 + size..];
            value
        };
        let (old, new) = (string(), string());
        assert!(data.is_empty());
        let gate = {
            let mut state = self.0.lock().unwrap();
            state.commits += 1;
            if state.fail_commit {
                return Err(StatusCode::PermissionDenied);
            }
            assert!(old.contains("/.unfour-inline-"));
            let contents = state.files.remove(&old).ok_or(StatusCode::NoSuchFile)?;
            state.files.insert(new, contents);
            state.hold_commit_after.take()
        };
        hold(gate).await;
        Ok(ok(id).into())
    }
}
pub(super) async fn fixture() -> (Arc<InlineSession>, Arc<Mutex<State>>) {
    fixture_with(State::default()).await
}
pub(super) async fn fixture_with(state: State) -> (Arc<InlineSession>, Arc<Mutex<State>>) {
    let state = Arc::new(Mutex::new(state));
    let (client, server) = tokio::io::duplex(128 * 1024);
    russh_sftp::server::run(server, Server(state.clone())).await;
    let session = Arc::new(InlineSession::new(client).await.unwrap());
    session.raw.set_timeout(1);
    (session, state)
}
pub(super) fn input(mode: SshFileWriteMode, content: String) -> SshFileWriteInput {
    SshFileWriteInput {
        workspace_id: Uuid::new_v4().to_string(),
        connection_id: Uuid::new_v4().to_string(),
        path: "/fixture.txt".into(),
        content,
        mode,
        timeout_ms: None,
    }
}
pub(super) fn gate() -> Gate {
    (
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
    )
}
pub(super) async fn cleaned(state: &Arc<Mutex<State>>) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if !state
                .lock()
                .unwrap()
                .files
                .keys()
                .any(|p| p.contains(".unfour-"))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("temporary files should be cleaned");
}
