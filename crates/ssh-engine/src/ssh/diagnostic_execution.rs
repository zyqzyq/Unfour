use super::*;

impl SshService {
    /// Run a single, read-only diagnostic command over SSH and capture its
    /// output. The command is validated against a fixed allowlist of read-only
    /// utilities (no shell, no chaining, no write/control operations) before it
    /// is executed. Captured output is line-redacted for sensitive material.
    /// Requires the `ssh-native` feature; otherwise returns an unsupported error.
    pub async fn run_diagnostic(
        &self,
        input: SshDiagnosticInput,
    ) -> AppResult<SshDiagnosticResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let command = validate_diagnostic_command(&input.command)?;
        let timeout = std::time::Duration::from_millis(
            input.timeout_ms.unwrap_or(15_000).clamp(1_000, 60_000),
        );
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        validate_connection_ready_for_session(&connection)?;

        #[cfg(feature = "ssh-native")]
        {
            self.run_diagnostic_native(&connection, &command, timeout)
                .await
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (timeout, &connection, &command);
            Err(AppError::Unsupported(
                "ssh diagnostics require a build with the ssh-native feature".to_string(),
            ))
        }
    }

    /// Run a single non-interactive SSH command over a fresh native connection.
    /// This lower-level execution primitive intentionally performs only basic
    /// command-shape validation; policy, environment gating, and high-risk
    /// confirmation live in the command-bus/MCP adapter path before this method
    /// is called.
    pub async fn run_command(&self, input: SshDiagnosticInput) -> AppResult<SshDiagnosticResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let command = validate_one_shot_command(&input.command)?;
        let timeout = std::time::Duration::from_millis(
            input.timeout_ms.unwrap_or(15_000).clamp(1_000, 60_000),
        );
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        validate_connection_ready_for_session(&connection)?;

        #[cfg(feature = "ssh-native")]
        {
            self.run_diagnostic_native(&connection, &command, timeout)
                .await
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (timeout, &connection, &command);
            Err(AppError::Unsupported(
                "ssh command execution requires a build with the ssh-native feature".to_string(),
            ))
        }
    }
    /// Open a fresh native connection, run a single command via `exec` (no PTY),
    /// capture stdout/stderr to EOF (bounded and timed), then disconnect.
    #[cfg(feature = "ssh-native")]
    pub(super) async fn run_diagnostic_native(
        &self,
        connection: &SshConnection,
        command: &str,
        timeout: std::time::Duration,
    ) -> AppResult<SshDiagnosticResult> {
        let config = Arc::new(native_client_config());
        let handler = SshClientHandler {
            host_key_store: HostKeyStore::new(self.db.pool().clone()),
            workspace_id: connection.workspace_id.clone(),
            host: connection.host.clone(),
            port: connection.port,
        };
        let addr = format!("{}:{}", connection.host, connection.port);
        let connect_timeout = std::time::Duration::from_secs(15);
        let mut handle = match tokio::time::timeout(
            connect_timeout,
            russh::client::connect(config, addr.as_str(), handler),
        )
        .await
        {
            Ok(Ok(handle)) => handle,
            Ok(Err(error)) => {
                return Err(AppError::Config(format!(
                    "ssh connection to {}:{} failed: {}",
                    connection.host,
                    connection.port,
                    sanitize_ssh_error(&error)
                )));
            }
            Err(_) => {
                return Err(AppError::Config(format!(
                    "ssh connection to {}:{} timed out after {}s",
                    connection.host,
                    connection.port,
                    connect_timeout.as_secs()
                )));
            }
        };

        self.authenticate_native(&mut handle, connection, None)
            .await?;
        let mut channel = handle
            .channel_open_session()
            .await
            .map_err(|error| AppError::Config(format!("failed to open ssh channel: {}", error)))?;
        channel
            .exec(true, command.as_bytes())
            .await
            .map_err(|error| {
                AppError::Config(format!("failed to run ssh diagnostic command: {}", error))
            })?;

        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let mut exit_status: Option<i32> = None;
        let mut truncated = false;

        let capture = async {
            loop {
                match channel.wait().await {
                    Some(russh::ChannelMsg::Data { data }) => {
                        append_capped(&mut stdout, &data[..], &mut truncated);
                    }
                    Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                        if ext == 1 {
                            append_capped(&mut stderr, &data[..], &mut truncated);
                        } else {
                            append_capped(&mut stdout, &data[..], &mut truncated);
                        }
                    }
                    Some(russh::ChannelMsg::ExitStatus { exit_status: code }) => {
                        exit_status = Some(code as i32);
                    }
                    Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
        };

        let timed_out = tokio::time::timeout(timeout, capture).await.is_err();
        if timed_out {
            truncated = true;
        }

        let _ = channel.close().await;
        let _ = handle
            .disconnect(
                russh::Disconnect::ByApplication,
                "diagnostic complete",
                "en",
            )
            .await;

        let (stdout_text, _) = redact_sensitive_lines(&String::from_utf8_lossy(&stdout));
        let (stderr_text, _) = redact_sensitive_lines(&String::from_utf8_lossy(&stderr));

        Ok(SshDiagnosticResult {
            warnings: Vec::new(),
            connection_id: connection.id.clone(),
            command: command.to_string(),
            stdout: stdout_text,
            stderr: stderr_text,
            exit_status,
            truncated,
        })
    }
}
