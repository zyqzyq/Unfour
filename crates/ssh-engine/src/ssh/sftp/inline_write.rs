//! Staging never removes/moves the destination. Commit is one atomic operation.
use super::inline_session::InlineSession;
use super::*;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use unfour_core::models::{SshFileWriteInput, SshFileWriteMode};

pub(crate) async fn write_inline(
    sftp: Arc<InlineSession>,
    input: &SshFileWriteInput,
) -> AppResult<()> {
    let existing = match sftp.raw.lstat(input.path.clone()).await {
        Ok(attrs) if attrs.attrs.is_regular() => Some(attrs.attrs),
        Ok(_) => {
            return Err(AppError::Validation(
                "remote target must be a regular file".into(),
            ))
        }
        Err(russh_sftp::client::error::Error::Status(status))
            if status.status_code == StatusCode::NoSuchFile =>
        {
            None
        }
        Err(error) => return Err(sftp_error("inspect inline write target", error)),
    };
    let append = input.mode == SshFileWriteMode::Append;
    if input.mode == SshFileWriteMode::Create && existing.is_some() {
        return Err(AppError::Validation("remote target already exists".into()));
    }
    if input.mode == SshFileWriteMode::Overwrite && existing.is_some() && !sftp.atomic_replace {
        return Err(AppError::Unsupported(
            "SSH_FILE_ATOMIC_REPLACE_UNSUPPORTED".into(),
        ));
    }
    // A short sibling name avoids exceeding NAME_MAX for long destination names.
    let parent = input
        .path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or(".");
    let temporary = format!("{parent}/.unfour-inline-{}", Uuid::new_v4());
    let path = if append { &input.path } else { &temporary };
    let flags = OpenFlags::WRITE
        | OpenFlags::CREATE
        | if append {
            OpenFlags::APPEND
        } else {
            OpenFlags::EXCLUDE
        };
    let mut attrs = FileAttributes::empty();
    attrs.permissions = Some(0o600);
    let handle = sftp
        .raw
        .open(path.clone(), flags, attrs)
        .await
        .map_err(|error| sftp_error("open inline write target", error))?
        .handle;
    // Cleanup only owns a successfully opened handle and its unique staging
    // name. An uncertain OPEN response can leak staging, never delete another file.
    let mut cleanup = InlineCleanup {
        sftp: sftp.clone(),
        handle: Some(handle.clone()),
        temporary: (!append).then_some(temporary.clone()),
    };
    let result = async {
        let chunk_size = sftp.write_chunk_size(&handle)?;
        // Ordered, acknowledged writes also preserve append ordering. A failed
        // append may add a prefix, but cannot truncate the original bytes.
        for (index, chunk) in input.content.as_bytes().chunks(chunk_size).enumerate() {
            sftp.raw
                .write(handle.clone(), (index * chunk_size) as u64, chunk.to_vec())
                .await
                .map_err(|error| sftp_error("write inline file data", error))?;
        }
        if !append {
            if let Some(metadata) = &existing {
                let mut attrs = FileAttributes::empty();
                attrs.permissions = metadata.permissions;
                attrs.uid = metadata.uid;
                attrs.gid = metadata.gid;
                sftp.raw
                    .fsetstat(handle.clone(), attrs)
                    .await
                    .map_err(|error| sftp_error("preserve inline write metadata", error))?;
            }
        }
        if sftp.fsync {
            sftp.raw
                .fsync(handle.clone())
                .await
                .map_err(|error| sftp_error("sync inline file", error))?;
        }
        sftp.raw
            .close(handle)
            .await
            .map_err(|error| sftp_error("close inline file", error))?;
        cleanup.handle = None;
        if append {
            return Ok(());
        }
        if input.mode == SshFileWriteMode::Overwrite && existing.is_some() {
            // Never fall back to target -> backup -> temporary -> target.
            sftp.replace(&temporary, &input.path)
                .await
                .map_err(|error| sftp_error("atomically replace inline file", error))?;
        } else {
            // SFTP v3 rename is no-replace; concurrent creation must fail.
            sftp.raw
                .rename(temporary.clone(), input.path.clone())
                .await
                .map_err(|error| sftp_error("commit new inline file", error))?;
        }
        cleanup.temporary = None;
        Ok(())
    }
    .await;
    // On ordinary errors, await bounded cleanup before closing the connection.
    // If cancelled during any await, Drop retries cleanup of staging only.
    cleanup.finish().await;
    result
}

struct InlineCleanup {
    sftp: Arc<InlineSession>,
    handle: Option<String>,
    temporary: Option<String>,
}
impl InlineCleanup {
    async fn finish(&mut self) {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            if let Some(handle) = &self.handle {
                let _ = self.sftp.raw.close(handle.clone()).await;
                self.handle = None;
            }
            if let Some(path) = &self.temporary {
                let _ = self.sftp.raw.remove(path.clone()).await;
                self.temporary = None;
            }
        })
        .await;
    }
}
impl Drop for InlineCleanup {
    fn drop(&mut self) {
        if self.handle.is_none() && self.temporary.is_none() {
            return;
        }
        let sftp = self.sftp.clone();
        let handle = self.handle.take();
        let temporary = self.temporary.take();
        tokio::spawn(async move {
            // Do not move a Drop guard into this task: runtime shutdown may
            // drop the task immediately, recursively spawning cleanup tasks.
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                if let Some(handle) = handle {
                    let _ = sftp.raw.close(handle).await;
                }
                if let Some(path) = temporary {
                    let _ = sftp.raw.remove(path).await;
                }
            })
            .await;
        });
    }
}

#[cfg(test)]
mod tests;
