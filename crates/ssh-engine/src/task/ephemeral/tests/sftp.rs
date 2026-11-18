use super::*;
use russh_sftp::protocol::{Attrs, Data, FileAttributes, Handle, OpenFlags, Status, StatusCode};
pub(super) struct Server(pub Arc<Mutex<State>>);
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
    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        self.lstat(id, path).await
    }
    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        let state = self.0.lock().unwrap();
        let file = state.files.get(&path).ok_or(StatusCode::NoSuchFile)?;
        let mut attrs = FileAttributes::empty();
        attrs.size = Some(file.len() as u64);
        attrs.permissions = Some(0o100600);
        Ok(Attrs { id, attrs })
    }
    async fn open(
        &mut self,
        id: u32,
        path: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        let mut state = self.0.lock().unwrap();
        if flags.contains(OpenFlags::EXCLUDE) && state.files.contains_key(&path) {
            return Err(StatusCode::Failure);
        }
        if flags.contains(OpenFlags::CREATE) {
            state.files.entry(path.clone()).or_default();
        }
        if !state.files.contains_key(&path) {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(Handle { id, handle: path })
    }
    async fn write(
        &mut self,
        id: u32,
        path: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        let mut state = self.0.lock().unwrap();
        let file = state.files.get_mut(&path).ok_or(StatusCode::NoSuchFile)?;
        let offset = offset as usize;
        file.resize(file.len().max(offset + data.len()), 0);
        file[offset..offset + data.len()].copy_from_slice(&data);
        Ok(ok(id))
    }
    async fn read(
        &mut self,
        id: u32,
        path: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, StatusCode> {
        let state = self.0.lock().unwrap();
        let file = state.files.get(&path).ok_or(StatusCode::NoSuchFile)?;
        let offset = offset as usize;
        if offset >= file.len() {
            return Err(StatusCode::Eof);
        }
        Ok(Data {
            id,
            data: file[offset..file.len().min(offset + len as usize)].to_vec(),
        })
    }
    async fn close(&mut self, id: u32, _: String) -> Result<Status, StatusCode> {
        Ok(ok(id))
    }
    async fn rename(&mut self, id: u32, from: String, to: String) -> Result<Status, StatusCode> {
        let mut state = self.0.lock().unwrap();
        if state.files.contains_key(&to) {
            return Err(StatusCode::Failure);
        }
        let file = state.files.remove(&from).ok_or(StatusCode::NoSuchFile)?;
        state.files.insert(to, file);
        Ok(ok(id))
    }
    async fn remove(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        self.0
            .lock()
            .unwrap()
            .files
            .remove(&path)
            .ok_or(StatusCode::NoSuchFile)?;
        Ok(ok(id))
    }
}
