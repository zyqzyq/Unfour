use std::future::Future;

use unfour_core::models::{
    ApiRequestInput, ApiResponse, ApiSavedRequest, RequestExecutionResult, ScriptExecutionStatus,
};
use unfour_core::AppResult;

use super::{CommandBusAdapterError, LocalCommandBusAdapter};

impl LocalCommandBusAdapter {
    pub(super) fn run_scripted_api_request(
        &self,
        execution_id: &str,
        future: impl Future<Output = AppResult<RequestExecutionResult>>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        let result = self
            .run(crate::call_control::cooperative(future, || {
                self.bus.cancel_api_request(execution_id)
            }))
            .map_err(|error| CommandBusAdapterError::from_api_app_error(&error))?;
        match result.response {
            Some(mut response) => {
                match result.post_response.status {
                    ScriptExecutionStatus::Failed => response
                        .warnings
                        .push("API_POST_RESPONSE_SCRIPT_FAILED".into()),
                    ScriptExecutionStatus::Timeout => response
                        .warnings
                        .push("API_POST_RESPONSE_SCRIPT_TIMEOUT".into()),
                    _ => {}
                }
                for warning in result.warnings {
                    if !response.warnings.contains(&warning) {
                        response.warnings.push(warning);
                    }
                }
                Ok(response)
            }
            None if result.http_error.is_some() => {
                // Keep the engine classification; never forward raw HTTP diagnostics.
                let code = API_EXECUTION_CODES
                    .iter()
                    .copied()
                    .find(|code| Some(*code) == result.http_error_code.as_deref())
                    .unwrap_or("HTTP_ERROR");
                Err(CommandBusAdapterError {
                    code,
                    message: super::error::api_execution_message(code),
                    details: serde_json::json!({}),
                })
            }
            None => Err(CommandBusAdapterError {
                code: "API_SCRIPT_EXECUTION_FAILED",
                message: "The saved API request pre-request script failed before sending.",
                details: serde_json::json!({}),
            }),
        }
    }

    pub(super) fn update_saved_request(
        &self,
        workspace_id: &str,
        request_id: &str,
        input: ApiRequestInput,
        expected_revision: Option<i64>,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.run(self.bus.update_api_request_if_current(
            workspace_id.to_string(),
            request_id.to_string(),
            input,
            expected_revision,
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API request update operation failed.",
                &error,
            )
        })
    }
}

const API_EXECUTION_CODES: &[&str] = &[
    "API_TIMEOUT",
    "NETWORK_ERROR",
    "HTTP_ERROR",
    "API_CANCELLED",
    "API_RESPONSE_TOO_LARGE",
    "SSH_TUNNEL_UNAVAILABLE",
    "SSH_TUNNEL_CONNECT_FAILED",
    "SSH_TUNNEL_HOST_KEY_CHANGED",
    "SSH_TUNNEL_HOST_KEY_UNTRUSTED",
    "SSH_TUNNEL_AUTH_FAILED",
    "SSH_TUNNEL_TARGET_UNREACHABLE",
    "SSH_TUNNEL_TIMEOUT",
    "SSH_TUNNEL_INVALID_TARGET",
    "SSH_TUNNEL_PROXY_FAILED",
    "SSH_TUNNEL_ROUTE_MISSING",
    "SSH_TUNNEL_INVALID_ROUTE",
    "SSH_TUNNEL_ROUTE_CHANGED",
    "SSH_TUNNEL_TEMPORARY_UNAVAILABLE",
    "SSH_TUNNEL_FORWARD_BUSY",
];
