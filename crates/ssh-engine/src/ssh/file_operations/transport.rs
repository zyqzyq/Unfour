use super::super::sftp::inline_session::InlineSession;
use super::*;

impl SshService {
    pub(super) async fn write_file_native(
        &self,
        connection: &SshConnection,
        input: &SshFileWriteInput,
    ) -> AppResult<SshDiagnosticResult> {
        let mut handle = FileConnection(Some(
            self.open_authenticated_connection(connection, None).await?,
        ));
        let channel = handle
            .0
            .as_ref()
            .unwrap()
            .channel_open_session()
            .await
            .map_err(|_| AppError::Config("failed to open file channel".into()))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|_| AppError::Config("failed to request SFTP subsystem".into()))?;
        let sftp = Arc::new(InlineSession::new(channel.into_stream()).await?);
        let result = super::super::sftp::inline_write::write_inline(sftp.clone(), input).await;
        let _ = sftp.raw.close_session();
        handle.finish().await;
        result?;
        Ok(SshDiagnosticResult {
            connection_id: input.connection_id.clone(),
            command: "sftp.write_file".into(),
            stdout: String::new(),
            stderr: String::new(),
            exit_status: Some(0),
            truncated: false,
            warnings: vec![],
        })
    }

    pub(super) async fn patch_file_native(
        &self,
        connection: &SshConnection,
        input: &SshFilePatchInput,
    ) -> AppResult<SshDiagnosticResult> {
        let mut handle = FileConnection(Some(
            self.open_authenticated_connection(connection, None).await?,
        ));
        let channel = handle
            .0
            .as_ref()
            .unwrap()
            .channel_open_session()
            .await
            .map_err(|_| AppError::Config("failed to open patch channel".into()))?;
        channel
            .exec(true, patch::command().as_bytes())
            .await
            .map_err(|_| AppError::Config("failed to execute patch helper".into()))?;
        let (mut reader, writer) = channel.split();
        let payload = patch::payload(input);
        let send = async {
            writer
                .data(payload.as_slice())
                .await
                .map_err(|_| AppError::Config("failed to send patch data".into()))?;
            writer
                .eof()
                .await
                .map_err(|_| AppError::Config("failed to finish patch input".into()))
        };
        let receive = async {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let mut truncated = false;
            let mut exit_status = None;
            while let Some(message) = reader.wait().await {
                match message {
                    russh::ChannelMsg::Data { data } => {
                        append_capped(&mut stdout, &data, &mut truncated)
                    }
                    russh::ChannelMsg::ExtendedData { data, .. } => {
                        append_capped(&mut stderr, &data, &mut truncated)
                    }
                    russh::ChannelMsg::ExitStatus { exit_status: code } => {
                        exit_status = Some(code as i32)
                    }
                    // EOF can precede exit status. Wait for channel close.
                    russh::ChannelMsg::Close => break,
                    _ => {}
                }
            }
            if exit_status.is_none() {
                return Err(AppError::Config(
                    "patch channel closed without exit status; inspect target before retrying"
                        .into(),
                ));
            }
            Ok(SshDiagnosticResult {
                connection_id: input.connection_id.clone(),
                command: "ssh.patch_file".into(),
                stdout: redact_sensitive_lines(&String::from_utf8_lossy(&stdout)).0,
                stderr: redact_sensitive_lines(&String::from_utf8_lossy(&stderr)).0,
                exit_status,
                truncated,
                warnings: vec![],
            })
        };
        let result = tokio::try_join!(send, receive).map(|(_, result)| result);
        let _ = writer.close().await;
        handle.finish().await;
        result
    }
}

/// Close the native connection even when the adapter drops a cancelled future.
struct FileConnection(Option<russh::client::Handle<SshClientHandler>>);
impl FileConnection {
    async fn finish(&mut self) {
        if let Some(handle) = self.0.take() {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                handle.disconnect(
                    russh::Disconnect::ByApplication,
                    "file operation complete",
                    "en",
                ),
            )
            .await;
        }
    }
}
impl Drop for FileConnection {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            tokio::spawn(async move {
                // Original-file safety must not depend on delayed cleanup.
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    handle.disconnect(
                        russh::Disconnect::ByApplication,
                        "file operation complete",
                        "en",
                    ),
                )
                .await;
            });
        }
    }
}
