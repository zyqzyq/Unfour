use super::*;
use russh_sftp::protocol::{Status, StatusCode};
use std::sync::{Arc, Mutex};

struct RenameServer {
    calls: Arc<Mutex<Vec<(String, String)>>>,
    lose_reply_at: usize,
    directory: std::path::PathBuf,
}

impl russh_sftp::server::Handler for RenameServer {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }

    async fn rename(&mut self, id: u32, from: String, to: String) -> Result<Status, StatusCode> {
        // Apply the mutation to isolated real files before withholding its ACK.
        std::fs::rename(
            self.directory.join(from.trim_start_matches('/')),
            self.directory.join(to.trim_start_matches('/')),
        )
        .map_err(|_| StatusCode::Failure)?;
        let count = {
            let mut calls = self.calls.lock().unwrap();
            calls.push((from, to));
            calls.len()
        };
        if count == self.lose_reply_at {
            std::future::pending::<()>().await;
        }
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: String::new(),
        })
    }
}

#[tokio::test]
async fn lost_upload_rename_ack_is_uncertain_and_never_rolls_back_or_replays() {
    for lose_reply_at in [1, 2] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("target"), b"original").unwrap();
        std::fs::write(directory.path().join("upload"), b"uploaded").unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let handler = RenameServer {
            calls: calls.clone(),
            lose_reply_at,
            directory: directory.path().to_owned(),
        };
        tokio::spawn(async move {
            let (server, _) = listener.accept().await.unwrap();
            russh_sftp::server::run(server, handler).await;
        });
        let client = tokio::net::TcpStream::connect(address).await.unwrap();
        let sftp = russh_sftp::client::SftpSession::new(client).await.unwrap();
        sftp.set_timeout(1);
        let result = replace_remote_file(&sftp, "/upload", "/target", "attempt", true).await;
        assert_eq!(
            result,
            Err(TaskStepError::OutcomeUncertain { cancelled: false })
        );
        assert_eq!(calls.lock().unwrap().len(), lose_reply_at);
        assert_eq!(
            std::fs::read(directory.path().join("target.unfour-task-backup-attempt")).unwrap(),
            b"original"
        );
        if lose_reply_at == 1 {
            assert!(!directory.path().join("target").exists());
            assert_eq!(
                std::fs::read(directory.path().join("upload")).unwrap(),
                b"uploaded"
            );
        } else {
            assert_eq!(
                std::fs::read(directory.path().join("target")).unwrap(),
                b"uploaded"
            );
            assert!(!directory.path().join("upload").exists());
        }
    }
}
