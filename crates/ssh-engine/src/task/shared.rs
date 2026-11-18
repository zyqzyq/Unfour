//! Ownership of channels opened on a borrowed, already-authenticated transport.
use super::*;
use std::{sync::Arc, time::Duration};
use tokio::sync::{watch, Semaphore};

#[derive(Clone)]
pub(in crate::ssh) struct SharedTaskSource {
    pub handle: Arc<russh::client::Handle<SshClientHandler>>,
    pub parent: watch::Receiver<bool>,
    pub cancel: watch::Receiver<bool>,
    pub pending: Arc<Semaphore>,
}

impl SharedTaskSource {
    pub(super) async fn channel(
        &self,
    ) -> Result<russh::Channel<russh::client::Msg>, TaskStepError> {
        let stopped = || TaskStepError::Cancelled;
        if *self.parent.borrow() || *self.cancel.borrow() {
            return Err(stopped());
        }
        let permit = self
            .pending
            .clone()
            .try_acquire_owned()
            .map_err(|_| task_error("SSH_TASK_CHANNEL_BUSY"))?;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let handle = self.handle.clone();
        let mut parent = self.parent.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if sender.is_closed() {
                return;
            }
            let result = tokio::select! {
                biased;
                _ = parent.wait_for(|closed| *closed) => return,
                result = handle.channel_open_session() => result.map_err(|error| task_error(match error {
                    russh::Error::ChannelOpenFailure(_) => "SSH_TASK_CHANNEL_REJECTED",
                    russh::Error::ConnectionTimeout | russh::Error::KeepaliveTimeout | russh::Error::InactivityTimeout => "SSH_TASK_CHANNEL_TIMEOUT",
                    _ => "SSH_TASK_TRANSPORT_CLOSED",
                })),
            };
            if let Err(Ok(channel)) = sender.send(result) {
                let _ = tokio::time::timeout(Duration::from_secs(2), channel.close()).await;
            }
        });
        let mut parent = self.parent.clone();
        let mut cancel = self.cancel.clone();
        tokio::select! {
            biased;
            _ = parent.wait_for(|closed| *closed) => Err(stopped()),
            _ = cancel.wait_for(|closed| *closed) => Err(stopped()),
            result = tokio::time::timeout(Duration::from_secs(15), receiver) => {
                result.map_err(|_| task_error("SSH_TASK_CHANNEL_TIMEOUT"))?
                    .map_err(|_| task_error("SSH_TASK_TRANSPORT_CLOSED"))?
            },
        }
    }
}

pub(super) fn task_error(code: &str) -> TaskStepError {
    TaskStepError::Failed {
        message: code.into(),
        exit_code: None,
    }
}

/// The library's SFTP writer cannot be interrupted while awaiting SSH window
/// credit. A bounded bridge and independent close handle own its lifetime.
pub(super) struct TaskSftpBridge {
    task: Option<tokio::task::JoinHandle<()>>,
    writer: Arc<russh::ChannelWriteHalf<russh::client::Msg>>,
    closed: bool,
}
impl TaskSftpBridge {
    pub(super) fn new(
        channel: russh::Channel<russh::client::Msg>,
    ) -> (russh::ChannelReadHalf, Self) {
        let (reader, writer) = channel.split();
        let writer = Arc::new(writer);
        (
            reader,
            Self {
                task: None,
                writer,
                closed: false,
            },
        )
    }
    pub(super) async fn request(&self) -> Result<(), TaskStepError> {
        self.writer
            .request_subsystem(true, "sftp")
            .await
            .map_err(|_| task_error("SSH_TASK_SFTP_FAILED"))
    }
    pub(super) fn start(&mut self, mut reader: russh::ChannelReadHalf) -> tokio::io::DuplexStream {
        let output = self.writer.clone();
        let (stream, relay) = tokio::io::duplex(64 * 1024);
        self.task = Some(tokio::spawn(async move {
            let (mut input, mut response) = tokio::io::split(relay);
            let mut channel_read = reader.make_reader();
            let mut channel_write = output.make_writer();
            tokio::select! {
                _ = tokio::io::copy(&mut channel_read, &mut response) => {},
                _ = tokio::io::copy(&mut input, &mut channel_write) => {},
            }
        }));
        stream
    }
    pub(super) async fn close(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
        let _ = tokio::time::timeout(Duration::from_secs(2), self.writer.close()).await;
        self.closed = true;
    }
}
impl Drop for TaskSftpBridge {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
        if !self.closed {
            let writer = self.writer.clone();
            tokio::spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(2), writer.close()).await;
            });
        }
    }
}
