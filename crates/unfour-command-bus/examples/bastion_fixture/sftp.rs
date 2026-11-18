//! An intentionally tiny, in-memory SFTP surface for the fixed A4 probe.
//! It cannot access the host filesystem or accept arbitrary file contents.
use russh_sftp::protocol::{Data, FileAttributes, Handle, OpenFlags, Status, StatusCode};

const HANDLE: &str = "a4-memory-probe";
const CONTENT: &[u8] = b"unfour-a4-sftp-probe\n";

#[derive(Default)]
pub struct MemoryFile {
    filename: Option<String>,
    content: Vec<u8>,
    open: bool,
}

fn success(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}

fn probe_path(path: &str) -> bool {
    let Some(uuid) = path.strip_prefix("/tmp/unfour-a4-") else {
        return false;
    };
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, byte)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

impl Drop for MemoryFile {
    fn drop(&mut self) {
        if self.filename.is_some() {
            println!("FIXTURE_SFTP_REMAINING");
        }
    }
}

impl russh_sftp::server::Handler for MemoryFile {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let expected = OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE;
        if !probe_path(&filename)
            || pflags.bits() != expected.bits()
            || attrs.permissions != Some(0o600)
            || self.filename.is_some()
        {
            return Err(StatusCode::PermissionDenied);
        }
        self.filename = Some(filename);
        self.open = true;
        println!("FIXTURE_SFTP_CREATE_OK");
        Ok(Handle {
            id,
            handle: HANDLE.into(),
        })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        if handle != HANDLE || !self.open || offset != 0 || data != CONTENT {
            return Err(StatusCode::PermissionDenied);
        }
        self.content = data;
        println!("FIXTURE_SFTP_WRITE_OK");
        Ok(success(id))
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        if handle != HANDLE || !self.open || len > 65536 || offset > CONTENT.len() as u64 {
            return Err(StatusCode::PermissionDenied);
        }
        if offset as usize >= self.content.len() {
            println!("FIXTURE_SFTP_EOF_OK");
            return Err(StatusCode::Eof);
        }
        let end = self.content.len().min(offset as usize + len as usize);
        println!("FIXTURE_SFTP_READ_OK");
        Ok(Data {
            id,
            data: self.content[offset as usize..end].to_vec(),
        })
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        if handle != HANDLE || !self.open {
            return Err(StatusCode::NoSuchFile);
        }
        self.open = false;
        println!("FIXTURE_SFTP_CLOSE_OK");
        Ok(success(id))
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        if self.filename.as_deref() != Some(&filename) || self.open {
            return Err(StatusCode::PermissionDenied);
        }
        self.filename = None;
        self.content.clear();
        println!("FIXTURE_SFTP_REMOVE_OK");
        Ok(success(id))
    }
}
