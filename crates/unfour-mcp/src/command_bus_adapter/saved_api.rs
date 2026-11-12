use std::future::Future;

use unfour_core::models::{ApiRequestInput, ApiResponse, ApiSavedRequest, RequestExecutionResult};
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
            .map_err(|error| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus scripted API send operation failed.",
                    &error,
                )
            })?;
        match result.response {
            Some(mut response) => {
                for warning in result.warnings {
                    if !response.warnings.contains(&warning) {
                        response.warnings.push(warning);
                    }
                }
                Ok(response)
            }
            None if result.http_error.is_some() => Err(CommandBusAdapterError {
                code: "HTTP_ERROR",
                message: "The command-bus API send operation failed.",
                details: serde_json::json!({}),
            }),
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
