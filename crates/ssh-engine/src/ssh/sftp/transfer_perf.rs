//! Transfer throughput helpers for russh-sftp.
//!
//! AsyncRead waits for each READ ACK; uploads already pipeline via write_nowait.
//! We raise chunk size / write concurrency and pipeline downloads across handles.

#[cfg(feature = "ssh-native")]
use std::collections::BTreeMap;
#[cfg(feature = "ssh-native")]
use std::io::SeekFrom;
#[cfg(feature = "ssh-native")]
use std::sync::Arc;
#[cfg(feature = "ssh-native")]
use std::time::{Duration, Instant};

#[cfg(feature = "ssh-native")]
use russh_sftp::client::fs::File;
#[cfg(feature = "ssh-native")]
use russh_sftp::client::{Config, SftpSession};
#[cfg(feature = "ssh-native")]
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
#[cfg(feature = "ssh-native")]
use tokio::sync::watch;
#[cfg(feature = "ssh-native")]
use tokio::task::JoinSet;

#[cfg(feature = "ssh-native")]
use super::support::{ensure_not_cancelled, transfer_sftp_error, TransferRunError};

/// Match OpenSSH-sized packets; larger chunks cut RTT count on unpipelined reads.
#[cfg(feature = "ssh-native")]
pub(super) const TRANSFER_BUFFER_SIZE: usize = 256 * 1024;
#[cfg(feature = "ssh-native")]
pub(super) const DOWNLOAD_PIPELINE: usize = 8;
#[cfg(feature = "ssh-native")]
const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(100);
#[cfg(feature = "ssh-native")]
const PROGRESS_MIN_BYTES: u64 = 512 * 1024;
#[cfg(feature = "ssh-native")]
const GENERATION_CHECK_BYTES: u64 = 1024 * 1024;

#[cfg(feature = "ssh-native")]
pub(super) fn sftp_client_config() -> Config {
    Config {
        max_packet_len: 256 * 1024,
        // OpenSSH defaults to ~64 outstanding requests; russh-sftp pipelines writes.
        max_concurrent_writes: 64,
        request_timeout_secs: 30,
    }
}

#[cfg(feature = "ssh-native")]
pub(super) struct ProgressThrottle {
    last_emit: Instant,
    last_bytes: u64,
}

#[cfg(feature = "ssh-native")]
impl ProgressThrottle {
    pub(super) fn new() -> Self {
        Self {
            last_emit: Instant::now()
                .checked_sub(PROGRESS_MIN_INTERVAL)
                .unwrap_or_else(Instant::now),
            last_bytes: 0,
        }
    }

    pub(super) fn should_emit(&mut self, transferred: u64) -> bool {
        let now = Instant::now();
        let bytes_delta = transferred.saturating_sub(self.last_bytes);
        if bytes_delta >= PROGRESS_MIN_BYTES
            || now.duration_since(self.last_emit) >= PROGRESS_MIN_INTERVAL
        {
            self.last_emit = now;
            self.last_bytes = transferred;
            true
        } else {
            false
        }
    }
}

#[cfg(feature = "ssh-native")]
pub(super) struct GenerationThrottle {
    last_bytes: u64,
}

#[cfg(feature = "ssh-native")]
impl GenerationThrottle {
    pub(super) fn new() -> Self {
        Self { last_bytes: 0 }
    }

    pub(super) fn should_check(&mut self, transferred: u64) -> bool {
        if transferred.saturating_sub(self.last_bytes) >= GENERATION_CHECK_BYTES {
            self.last_bytes = transferred;
            true
        } else {
            false
        }
    }
}

#[cfg(feature = "ssh-native")]
async fn read_chunk_at(
    mut file: File,
    offset: u64,
    length: usize,
) -> Result<(u64, Vec<u8>, File), TransferRunError> {
    file.seek(SeekFrom::Start(offset))
        .await
        .map_err(|error| TransferRunError::Failed(error.to_string()))?;
    let mut buffer = vec![0_u8; length];
    let mut read = 0_usize;
    while read < length {
        let n = file
            .read(&mut buffer[read..])
            .await
            .map_err(|error| TransferRunError::Failed(error.to_string()))?;
        if n == 0 {
            break;
        }
        read += n;
    }
    buffer.truncate(read);
    Ok((offset, buffer, file))
}

#[cfg(feature = "ssh-native")]
fn launch_download_chunks(
    sftp_open_files: &mut Vec<File>,
    inflight: &mut JoinSet<Result<(u64, Vec<u8>, File), TransferRunError>>,
    next_offset: &mut u64,
    total: u64,
) {
    while *next_offset < total {
        let Some(file) = sftp_open_files.pop() else {
            break;
        };
        let offset = *next_offset;
        let length = usize::try_from((total - offset).min(TRANSFER_BUFFER_SIZE as u64))
            .unwrap_or(TRANSFER_BUFFER_SIZE);
        *next_offset += length as u64;
        inflight.spawn(async move { read_chunk_at(file, offset, length).await });
    }
}

/// Pipelined download: keep several SFTP READ requests in flight on separate handles.
#[cfg(feature = "ssh-native")]
pub(super) async fn copy_remote_to_local_pipelined(
    sftp: Arc<SftpSession>,
    remote_path: &str,
    local: &mut tokio::fs::File,
    total: u64,
    cancel_rx: &mut watch::Receiver<bool>,
    mut on_progress: impl FnMut(u64),
) -> Result<(), TransferRunError> {
    if total == 0 {
        return Ok(());
    }

    let pipeline =
        DOWNLOAD_PIPELINE.min(total.div_ceil(TRANSFER_BUFFER_SIZE as u64).max(1) as usize);
    let mut open_set: JoinSet<Result<File, TransferRunError>> = JoinSet::new();
    for _ in 0..pipeline {
        ensure_not_cancelled(cancel_rx)?;
        let sftp = sftp.clone();
        let path = remote_path.to_string();
        open_set.spawn(async move { sftp.open(path).await.map_err(transfer_sftp_error) });
    }
    let mut idle_files = Vec::with_capacity(pipeline);
    while let Some(opened) = open_set.join_next().await {
        ensure_not_cancelled(cancel_rx)?;
        idle_files.push(opened.map_err(|error| TransferRunError::Failed(error.to_string()))??);
    }

    let mut next_offset = 0_u64;
    let mut write_offset = 0_u64;
    let mut pending = BTreeMap::<u64, Vec<u8>>::new();
    let mut inflight: JoinSet<Result<(u64, Vec<u8>, File), TransferRunError>> = JoinSet::new();

    launch_download_chunks(&mut idle_files, &mut inflight, &mut next_offset, total);

    while inflight.len() > 0 || write_offset < total {
        ensure_not_cancelled(cancel_rx)?;
        let Some(joined) = inflight.join_next().await else {
            break;
        };
        let (offset, data, file) =
            joined.map_err(|error| TransferRunError::Failed(error.to_string()))??;
        idle_files.push(file);
        let chunk_len = data.len() as u64;
        pending.insert(offset, data);

        while let Some(chunk) = pending.remove(&write_offset) {
            ensure_not_cancelled(cancel_rx)?;
            local
                .write_all(&chunk)
                .await
                .map_err(|error| TransferRunError::Failed(error.to_string()))?;
            write_offset = write_offset.saturating_add(chunk.len() as u64);
            on_progress(write_offset);
        }

        if chunk_len == 0 && offset < total {
            return Err(TransferRunError::Failed(
                "remote file ended before advertised size".to_string(),
            ));
        }

        launch_download_chunks(&mut idle_files, &mut inflight, &mut next_offset, total);
    }

    if write_offset != total {
        return Err(TransferRunError::Failed(
            "downloaded size did not match remote file size".to_string(),
        ));
    }
    Ok(())
}
