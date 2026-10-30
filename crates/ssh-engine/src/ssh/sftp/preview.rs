use super::*;
use unfour_core::models::SftpTextPreview;

#[cfg(any(feature = "ssh-native", test))]
const MAX_PREVIEW_BYTES: usize = 1024 * 1024;

#[cfg(all(test, feature = "ssh-native"))]
mod native_tests;
#[cfg(test)]
mod tests;

impl SshService {
    pub async fn sftp_preview_text(&self, input: SftpPathInput) -> AppResult<SftpTextPreview> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        let path = normalize_remote_path(&input.path)?;

        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = path;
            Err(AppError::Unsupported(
                "SFTP requires the native SSH transport".into(),
            ))
        }
        #[cfg(feature = "ssh-native")]
        {
            let (sftp, _, generation) = self
                .sftp_context(&input)
                .await
                .map_err(|_| preview_error("SFTP_PREVIEW_DISCONNECTED"))?;
            let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
                let metadata = sftp
                    .symlink_metadata(path.clone())
                    .await
                    .map_err(remote_error)?;
                check_metadata(&metadata)?;
                // open() uses READ only; no local file, transfer, shell, or persistence path.
                let mut file = sftp.open(path).await.map_err(remote_error)?;
                let result = async {
                    // Check the opened handle too, in case the directory entry changed.
                    check_metadata(&file.metadata().await.map_err(remote_error)?)?;
                    read_bounded_text(&mut file).await
                }
                .await;
                // shutdown closes the remote handle, it does not write file contents.
                let closed = file.shutdown().await.map_err(read_error);
                if closed
                    .as_ref()
                    .err()
                    .is_some_and(|error| error.code() == "SFTP_PREVIEW_DISCONNECTED")
                {
                    return Err(preview_error("SFTP_PREVIEW_DISCONNECTED"));
                }
                let preview = result?;
                closed?;
                Ok(preview)
            })
            .await
            .unwrap_or_else(|_| Err(preview_error("SFTP_PREVIEW_TIMEOUT")));
            let (_, _, current_generation) = self
                .sftp_context(&input)
                .await
                .map_err(|_| preview_error("SFTP_PREVIEW_DISCONNECTED"))?;
            if current_generation != generation {
                return Err(preview_error("SFTP_PREVIEW_DISCONNECTED"));
            }
            result
        }
    }
}

#[cfg(any(feature = "ssh-native", test))]
fn preview_error(code: &str) -> AppError {
    // Only fixed local codes; never propagate remote error messages or file bytes.
    AppError::Validation(code.to_owned())
}

#[cfg(feature = "ssh-native")]
fn check_metadata(metadata: &russh_sftp::client::fs::Metadata) -> AppResult<()> {
    if !metadata.file_type().is_file() {
        return Err(preview_error("SFTP_PREVIEW_NOT_FILE"));
    }
    check_size(metadata.len())
}

#[cfg(any(feature = "ssh-native", test))]
fn check_size(size: u64) -> AppResult<()> {
    if size > MAX_PREVIEW_BYTES as u64 {
        return Err(preview_error("SFTP_PREVIEW_TOO_LARGE"));
    }
    Ok(())
}

#[cfg(any(feature = "ssh-native", test))]
async fn read_bounded_text(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> AppResult<SftpTextPreview> {
    use tokio::io::AsyncReadExt;
    // One sentinel byte detects growth or incorrect/missing metadata. The driver
    // receives bounded read buffers, so it cannot download the whole large file.
    let mut bytes = Vec::new();
    reader
        .take((MAX_PREVIEW_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(read_error)?;
    check_size(bytes.len() as u64)?;
    let size = bytes.len() as u64;
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return Err(preview_error("SFTP_PREVIEW_ENCODING"));
    }
    if bytes
        .iter()
        .any(|byte| matches!(byte, 0..=8 | 11 | 14..=31 | 127))
    {
        return Err(preview_error("SFTP_PREVIEW_BINARY"));
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let content = std::str::from_utf8(bytes)
        .map_err(|_| preview_error("SFTP_PREVIEW_ENCODING"))?
        .to_owned();
    Ok(SftpTextPreview { content, size })
}

#[cfg(any(feature = "ssh-native", test))]
fn read_error(error: std::io::Error) -> AppError {
    use std::io::ErrorKind;
    // russh-sftp's AsyncRead erases typed status errors into io::Error::Other.
    // Recognize its fixed status prefix without exposing the server's message.
    let message = error.to_string();
    let code = if error.kind() == ErrorKind::PermissionDenied
        || message.starts_with("Permission denied:")
    {
        "SFTP_PREVIEW_PERMISSION_DENIED"
    } else if error.kind() == ErrorKind::TimedOut || message == "Timeout" {
        "SFTP_PREVIEW_TIMEOUT"
    } else if matches!(
        error.kind(),
        ErrorKind::BrokenPipe
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
            | ErrorKind::UnexpectedEof
    ) || message.starts_with("Connection lost:")
        || message.starts_with("No connection:")
        || message.starts_with("I/O:")
        || message.contains("RecvError:")
        || message.contains("SendError:")
        || message == "session closed"
        || message == "sender dropped"
    {
        "SFTP_PREVIEW_DISCONNECTED"
    } else {
        "SFTP_PREVIEW_READ_FAILED"
    };
    preview_error(code)
}

#[cfg(feature = "ssh-native")]
fn remote_error(error: russh_sftp::client::error::Error) -> AppError {
    use russh_sftp::{client::error::Error, protocol::StatusCode};
    let code = match error {
        Error::Status(status) => match status.status_code {
            StatusCode::PermissionDenied => "SFTP_PREVIEW_PERMISSION_DENIED",
            StatusCode::NoSuchFile => "SFTP_PREVIEW_NOT_FOUND",
            StatusCode::NoConnection | StatusCode::ConnectionLost => "SFTP_PREVIEW_DISCONNECTED",
            _ => "SFTP_PREVIEW_READ_FAILED",
        },
        Error::Timeout => "SFTP_PREVIEW_TIMEOUT",
        Error::IO(_) | Error::UnexpectedBehavior(_) => "SFTP_PREVIEW_DISCONNECTED",
        _ => "SFTP_PREVIEW_READ_FAILED",
    };
    preview_error(code)
}
