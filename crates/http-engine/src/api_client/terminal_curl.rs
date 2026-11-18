//! HTTP semantics for an explicitly selected, already logged-in terminal.
//! SSH owns the terminal transport; this module never acquires an SSH session.
use super::*;

mod request;
mod response;
#[cfg(test)]
mod tests;

const MAX_REQUEST_BYTES: usize = 512 * 1024;
const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 128 * 1024;
const MAX_OUTPUT_BYTES: usize = MAX_RESPONSE_BYTES + MAX_HEADER_BYTES + 1024;
// An API request must not occupy the user's interactive terminal indefinitely.
// Ordinary direct/TCP requests retain their separate unlimited-timeout policy.
const DEFAULT_TERMINAL_TIMEOUT_MS: u64 = 30_000;

/// Contains request credentials in memory. Intentionally has no Debug or serde.
pub struct PreparedTerminalCurl {
    pub request: unfour_core::terminal_curl_contract::TerminalCurlRequest,
    executed: ApiRequestInput,
    secrets: Vec<String>,
    snapshot: unfour_core::models::ApiExecutedRequestSnapshot,
}

impl PreparedTerminalCurl {
    /// Bound raw decoded bytes, independently of the terminal's wire encoding.
    pub fn max_output_bytes(&self) -> usize {
        MAX_OUTPUT_BYTES
    }

    pub fn timeout_ms(&self) -> Option<u64> {
        self.executed.timeout_ms
    }
}

/// Called only after the ordinary API pipeline resolves variables and auth.
pub fn prepare_terminal_curl(
    input: ApiRequestInput,
    secrets: &[String],
) -> AppResult<PreparedTerminalCurl> {
    request::prepare(input, secrets)
}

impl ApiClientService {
    /// A fully received HTTP response remains authoritative even if restoring
    /// the parent shell fails, or recording local history fails afterwards.
    pub async fn complete_terminal_curl(
        &self,
        prepared: PreparedTerminalCurl,
        stdout: Vec<u8>,
        exit_code: i32,
        duration: Duration,
        terminal_restored: bool,
        transport: unfour_core::models::TerminalOperationTiming,
    ) -> AppResult<ApiResponse> {
        let received = response::parse(&prepared, &stdout, exit_code)?;
        let duration_ms = duration.as_millis();
        let mut warnings = Vec::new();
        if (300..400).contains(&received.status) {
            warnings.push("SSH_TERMINAL_HTTP_REDIRECT_NOT_FOLLOWED".into());
        }
        if !terminal_restored {
            warnings.push("SSH_TERMINAL_HTTP_TERMINAL_RECOVERY_REQUIRED".into());
        }
        let history_id = match self
            .insert_history(
                &prepared.executed,
                received.status,
                duration_ms,
                &received.headers,
                &received.body,
                &prepared.secrets,
            )
            .await
        {
            Ok(id) => id,
            Err(_) => {
                warnings.push("API_HISTORY_RECORD_FAILED".into());
                String::new()
            }
        };
        Ok(ApiResponse {
            terminal_timing: Some(unfour_core::models::TerminalCurlTiming {
                transport,
                curl_total_ms: received.curl_total_ms,
                curl_start_transfer_ms: received.curl_start_transfer_ms,
            }),
            diagnostic_projection: Some(history_redaction::diagnostic_response(
                &received.headers,
                &received.body,
                &prepared.secrets,
            )),
            warnings,
            request_snapshot: Some(prepared.snapshot),
            history_id,
            status: received.status,
            status_text: reqwest::StatusCode::from_u16(received.status)
                .ok()
                .and_then(|status| status.canonical_reason())
                .unwrap_or("")
                .into(),
            headers: received.headers,
            body: received.body,
            duration_ms,
        })
    }
}

fn failure(code: &'static str) -> AppError {
    AppError::Validation(code.into())
}

fn unsupported() -> AppError {
    failure("SSH_TERMINAL_HTTP_UNSUPPORTED_REQUEST")
}

fn invalid_response() -> AppError {
    failure("SSH_TERMINAL_HTTP_INVALID_RESPONSE")
}
