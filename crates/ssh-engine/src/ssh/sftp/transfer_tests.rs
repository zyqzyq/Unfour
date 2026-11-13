use super::protocol_fixture::fixture;
use super::*;
use russh_sftp::protocol::{Attrs, Data, FileAttributes, Handle, OpenFlags, Status, StatusCode};

// Simulate an independent writer at the point where the client has finished
// copying but has not yet published its result.
#[derive(Default)]
struct TransferServer {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    create_remote_on_close: Option<String>,
    create_local_on_read: Option<std::path::PathBuf>,
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}

impl russh_sftp::server::Handler for TransferServer {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        let files = self.files.lock().unwrap();
        let bytes = files.get(&path).ok_or(StatusCode::NoSuchFile)?;
        let mut attrs = FileAttributes::empty();
        attrs.size = Some(bytes.len() as u64);
        Ok(Attrs { id, attrs })
    }

    async fn open(
        &mut self,
        id: u32,
        path: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        let mut files = self.files.lock().unwrap();
        if flags.contains(OpenFlags::WRITE) {
            assert_eq!(
                flags.bits(),
                (OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE).bits()
            );
            if files.contains_key(&path) {
                return Err(StatusCode::Failure);
            }
            files.insert(path.clone(), vec![]);
        } else if !files.contains_key(&path) {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(Handle { id, handle: path })
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, StatusCode> {
        if let Some(path) = self.create_local_on_read.take() {
            std::fs::write(path, b"created during download").unwrap();
        }
        let files = self.files.lock().unwrap();
        let bytes = files.get(&handle).ok_or(StatusCode::NoSuchFile)?;
        let start = offset as usize;
        if start >= bytes.len() {
            return Err(StatusCode::Eof);
        }
        Ok(Data {
            id,
            data: bytes[start..bytes.len().min(start + len as usize)].to_vec(),
        })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        let mut files = self.files.lock().unwrap();
        let file = files.get_mut(&handle).ok_or(StatusCode::NoSuchFile)?;
        assert_eq!(offset, file.len() as u64);
        file.extend(data);
        Ok(ok(id))
    }

    async fn close(&mut self, id: u32, _: String) -> Result<Status, StatusCode> {
        if let Some(path) = self.create_remote_on_close.take() {
            self.files
                .lock()
                .unwrap()
                .insert(path, b"created during upload".to_vec());
        }
        Ok(ok(id))
    }

    async fn rename(&mut self, id: u32, from: String, to: String) -> Result<Status, StatusCode> {
        let mut files = self.files.lock().unwrap();
        if files.contains_key(&to) {
            return Err(StatusCode::Failure);
        }
        let bytes = files.remove(&from).ok_or(StatusCode::NoSuchFile)?;
        files.insert(to, bytes);
        Ok(ok(id))
    }

    async fn remove(&mut self, id: u32, path: String) -> Result<Status, StatusCode> {
        self.files
            .lock()
            .unwrap()
            .remove(&path)
            .ok_or(StatusCode::NoSuchFile)?;
        Ok(ok(id))
    }
}

#[tokio::test]
async fn download_preserves_foreign_files_on_metadata_failure_and_publication_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("report.csv");
    let foreign_part = dir.path().join("report.csv.part");
    std::fs::write(&foreign_part, b"independent partial download").unwrap();
    let server = TransferServer {
        create_local_on_read: Some(target.clone()),
        ..Default::default()
    };
    let files = server.files.clone();
    let (service, input, proxy) = fixture(server).await;
    let (sftp, _, generation) = service.sftp_context(&input).await.unwrap();
    let (_cancel, rx) = tokio::sync::watch::channel(false);
    let download = |overwrite| {
        service.run_download(
            "download",
            &input.session_id,
            generation,
            sftp.clone(),
            &input.path,
            target.to_str().unwrap(),
            overwrite,
            rx.clone(),
        )
    };
    assert!(download(false).await.is_err());
    assert_eq!(
        std::fs::read(&foreign_part).unwrap(),
        b"independent partial download"
    );
    assert!(!target.exists());

    files
        .lock()
        .unwrap()
        .insert(input.path.clone(), b"contents".to_vec());
    assert!(download(false).await.is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"created during download");
    download(true).await.unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"contents");
    assert_eq!(
        std::fs::read(&foreign_part).unwrap(),
        b"independent partial download"
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    proxy.abort();
}

#[tokio::test]
async fn upload_checks_no_overwrite_on_commit_and_only_cleans_owned_staging() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"upload").unwrap();
    let server = TransferServer {
        create_remote_on_close: Some("/fixture.txt".into()),
        ..Default::default()
    };
    let files = server.files.clone();
    let (service, input, proxy) = fixture(server).await;
    let (sftp, _, generation) = service.sftp_context(&input).await.unwrap();
    let (_cancel, rx) = tokio::sync::watch::channel(false);
    let upload = |overwrite| {
        service.run_upload(
            "upload",
            &input.session_id,
            generation,
            sftp.clone(),
            source.to_str().unwrap(),
            &input.path,
            overwrite,
            rx.clone(),
        )
    };
    let staging = upload_temp_path(&input.path, "upload");
    files
        .lock()
        .unwrap()
        .insert(staging.clone(), b"foreign staging".to_vec());
    assert!(upload(false).await.is_err());
    assert_eq!(
        files.lock().unwrap().remove(&staging).unwrap(),
        b"foreign staging"
    );

    assert!(upload(false).await.is_err());
    assert_eq!(files.lock().unwrap()[&input.path], b"created during upload");
    assert_eq!(files.lock().unwrap().len(), 1);
    upload(true).await.unwrap();
    assert_eq!(
        files.lock().unwrap().remove(&input.path).unwrap(),
        b"upload"
    );
    upload(false).await.unwrap();
    assert_eq!(files.lock().unwrap()[&input.path], b"upload");
    assert_eq!(files.lock().unwrap().len(), 1);
    proxy.abort();
}
