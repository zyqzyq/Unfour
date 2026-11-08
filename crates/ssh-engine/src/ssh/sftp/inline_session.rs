//! The high-level SFTP client does not expose POSIX rename. Keep inline file
//! operations on one raw SFTP channel so overwrite never needs a backup gap.
use super::*;
use russh_sftp::{
    client::{error::Error, rawsession::Limits, RawSftpSession},
    protocol::{Packet, StatusCode},
};

pub(super) const POSIX_RENAME: &str = "posix-rename@openssh.com";

pub(in crate::ssh) struct InlineSession {
    pub(in crate::ssh) raw: RawSftpSession,
    pub(super) atomic_replace: bool,
    pub(super) fsync: bool,
    limits: Limits,
}

impl InlineSession {
    pub(in crate::ssh) async fn new<S>(stream: S) -> AppResult<Self>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let mut raw = RawSftpSession::new_with_config(stream, sftp_client_config());
        let version = raw
            .init()
            .await
            .map_err(|e| sftp_error("initialize inline SFTP", e))?;
        let supports = |name: &str| version.extensions.get(name).is_some_and(|v| v == "1");
        let limits = if supports(russh_sftp::extensions::LIMITS) {
            Limits::from(
                raw.limits()
                    .await
                    .map_err(|e| sftp_error("read SFTP limits", e))?,
            )
        } else {
            Limits::default()
        };
        raw.set_limits(limits);
        Ok(Self {
            raw,
            atomic_replace: supports(POSIX_RENAME),
            fsync: supports(russh_sftp::extensions::FSYNC),
            limits,
        })
    }

    pub(super) fn write_chunk_size(&self, handle: &str) -> AppResult<usize> {
        // Includes packet length, type, id, handle length, offset and data length.
        let overhead = 25 + handle.len() as u64;
        let packet = self
            .limits
            .packet_len
            .unwrap_or(34_000)
            .saturating_sub(overhead);
        let size = packet
            .min(self.limits.write_len.unwrap_or(32_768))
            .min(TRANSFER_BUFFER_SIZE as u64);
        if size == 0 {
            return Err(AppError::Config(
                "SFTP write packet limit is too small".into(),
            ));
        }
        Ok(size as usize)
    }

    pub(super) async fn replace(&self, from: &str, to: &str) -> Result<(), Error> {
        // OpenSSH PROTOCOL section 4.3: two SSH strings, POSIX rename semantics.
        let mut data = Vec::new();
        for path in [from, to] {
            data.extend_from_slice(&(path.len() as u32).to_be_bytes());
            data.extend_from_slice(path.as_bytes());
        }
        match self.raw.extended(POSIX_RENAME, data).await? {
            Packet::Status(status) if status.status_code == StatusCode::Ok => Ok(()),
            Packet::Status(status) => Err(Error::Status(status)),
            _ => Err(Error::UnexpectedPacket),
        }
    }
}
