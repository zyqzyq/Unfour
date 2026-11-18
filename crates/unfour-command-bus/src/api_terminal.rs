//! Compose the HTTP owner with an exclusive, transient terminal operation.
use crate::CommandBus;
use tokio_util::sync::CancellationToken;
use unfour_core::{
    models::{ApiRequestInput, ApiResponse, RequestExecutionResult},
    network::ApiRoutingInput,
    AppError, AppResult,
};
use unfour_ssh_engine::ssh::temporary::TemporaryTerminalOperation;

pub(super) fn validate_mode(handle: Option<&str>, mode: &str, confirmed: bool) -> AppResult<()> {
    let code = match mode {
        "tcp" => return Ok(()),
        "terminalCurl" if handle.is_none_or(|handle| handle.trim().is_empty()) => {
            "SSH_TERMINAL_HTTP_TARGET_REQUIRED"
        }
        "terminalCurl" if !confirmed => "SSH_TERMINAL_HTTP_CONFIRMATION_REQUIRED",
        "terminalCurl" => return Ok(()),
        _ => "SSH_TERMINAL_HTTP_MODE_INVALID",
    };
    Err(AppError::Validation(code.into()))
}

impl CommandBus {
    /// Preserve the legacy direct/TCP contract for adapters and saved replay.
    pub async fn send_api_request_via(
        &self,
        execution_id: &str,
        input: ApiRequestInput,
        environment_id_override: Option<String>,
        saved_id: Option<&str>,
        preview: Option<&ApiRoutingInput>,
        temporary_handle: Option<&str>,
    ) -> AppResult<RequestExecutionResult> {
        self.send_api_request_via_with_mode(
            execution_id,
            input,
            environment_id_override,
            saved_id,
            preview,
            temporary_handle,
            "tcp",
            false,
        )
        .await
    }

    pub(super) async fn send_terminal_curl(
        &self,
        input: ApiRequestInput,
        cancellation: CancellationToken,
        secrets: &[String],
        operation: TemporaryTerminalOperation,
    ) -> AppResult<ApiResponse> {
        let prepared = unfour_http_engine::prepare_terminal_curl(input, secrets)?;
        let started = std::time::Instant::now();
        let result = operation
            .run_curl(
                &prepared.request,
                prepared.timeout_ms(),
                cancellation,
                prepared.max_output_bytes(),
            )
            .await?;
        self.api_client
            .complete_terminal_curl(
                prepared,
                result.stdout,
                result.exit_code,
                started.elapsed(),
                result.terminal_restored,
                result.timing,
            )
            .await
    }
}
