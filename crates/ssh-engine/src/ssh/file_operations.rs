use super::*;
use unfour_core::models::{SshFilePatchInput, SshFileWriteInput, SSH_FILE_MAX_CONTENT_BYTES};

mod patch;
#[cfg(test)]
mod tests;
#[cfg(feature = "ssh-native")]
mod transport;

fn validate_file_input(
    workspace: &str,
    connection: &str,
    path: &str,
    bytes: usize,
) -> AppResult<()> {
    validate_workspace_id(workspace)?;
    validate_connection_id(connection)?;
    if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
        return Err(AppError::Validation("invalid remote file path".into()));
    }
    if bytes > SSH_FILE_MAX_CONTENT_BYTES {
        return Err(AppError::Validation("SSH_FILE_CONTENT_TOO_LARGE".into()));
    }
    Ok(())
}

impl SshService {
    pub async fn write_file(&self, input: SshFileWriteInput) -> AppResult<SshDiagnosticResult> {
        validate_file_input(
            &input.workspace_id,
            &input.connection_id,
            &input.path,
            input.content.len(),
        )?;
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        validate_connection_ready_for_session(&connection)?;
        #[cfg(feature = "ssh-native")]
        {
            let timeout = std::time::Duration::from_millis(
                input.timeout_ms.unwrap_or(30_000).clamp(1_000, 60_000),
            );
            tokio::time::timeout(timeout, self.write_file_native(&connection, &input))
                .await
                .map_err(|_| {
                    AppError::Timeout(
                        "SSH file write timed out; inspect the target before retrying".into(),
                    )
                })?
        }
        #[cfg(not(feature = "ssh-native"))]
        Err(AppError::Unsupported(
            "SSH file writes require ssh-native".into(),
        ))
    }

    pub async fn patch_file(&self, input: SshFilePatchInput) -> AppResult<SshDiagnosticResult> {
        validate_file_input(
            &input.workspace_id,
            &input.connection_id,
            &input.path,
            input.search.len().saturating_add(input.replace.len()),
        )?;
        if input.search.is_empty() {
            return Err(AppError::Validation("patch search cannot be empty".into()));
        }
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        validate_connection_ready_for_session(&connection)?;
        #[cfg(feature = "ssh-native")]
        {
            let timeout = std::time::Duration::from_millis(
                input.timeout_ms.unwrap_or(30_000).clamp(1_000, 60_000),
            );
            tokio::time::timeout(timeout, self.patch_file_native(&connection, &input))
                .await
                .map_err(|_| {
                    AppError::Timeout(
                        "SSH file patch timed out; inspect the target before retrying".into(),
                    )
                })?
        }
        #[cfg(not(feature = "ssh-native"))]
        Err(AppError::Unsupported(
            "SSH file patches require ssh-native".into(),
        ))
    }
}
