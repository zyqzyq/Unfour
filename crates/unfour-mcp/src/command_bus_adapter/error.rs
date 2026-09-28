use super::CommandBusAdapterError;
use unfour_core::AppError;

impl CommandBusAdapterError {
    pub(super) fn from_flow_error(error: &AppError) -> Self {
        if matches!(error, AppError::Validation(reason) if reason == "FLOW_INVALID_REFERENCE") {
            return Self {
                code: "FLOW_INVALID_REFERENCE",
                message: "Invalid Flow reference. Use a JSON pointer rooted at inputs, steps or probe and an object containing only a string $ref.",
                details: serde_json::json!({}),
            };
        }
        if matches!(error, AppError::Validation(reason) if reason == "FLOW_UNSAFE_REFERENCE") {
            return Self {
                code: "FLOW_UNSAFE_REFERENCE",
                message: "A referenced output is not guaranteed on every path to this step. Use a common upstream output or move the consumer into the matching branch.",
                details: serde_json::json!({}),
            };
        }
        if matches!(error, AppError::Validation(reason) if reason == "FLOW_CONFIRMATION_STALE") {
            return Self {
                code: "FLOW_CONFIRMATION_STALE",
                message: "Flow changed after confirmation. Read the Flow and request a new confirmation before retrying.",
                details: serde_json::json!({}),
            };
        }
        if matches!(error, AppError::Validation(reason) if reason == "FLOW_REVISION_CONFLICT") {
            return Self {
                code: "FLOW_REVISION_CONFLICT",
                message: "The Flow revision changed; reload before retrying.",
                details: serde_json::json!({}),
            };
        }
        Self::from_app_error("The command-bus Flow operation failed.", error)
    }

    /// Surface the `AppError` classification with a safe, operation-specific
    /// message. Display text may embed hosts, DSNs, or other sensitive detail.
    pub(super) fn from_app_error(message: &'static str, error: &AppError) -> Self {
        Self {
            code: error.code(),
            message,
            details: error
                .database_error_details()
                .unwrap_or_else(|| serde_json::json!({})),
        }
    }

    pub(super) fn from_database_app_error(message: &'static str, error: &AppError) -> Self {
        let mut result = Self::from_app_error(message, error);
        // Validation text can contain local paths or credentials; allow only fixed database text.
        let safe_reason = match error {
            AppError::Unsupported(_) | AppError::ReadOnly(_) => error.safe_reason(),
            AppError::Validation(reason) if reason == "structure export requires SQL format" => {
                error.safe_reason()
            }
            _ => None,
        };
        if let Some(reason) = safe_reason {
            result.details = serde_json::json!({ "reason": reason });
        }
        result
    }

    pub(super) fn from_ssh_app_error(message: &'static str, error: &AppError) -> Self {
        let message = match error {
            AppError::SshTaskIncompleteRemoteState => {
                "The SSH task cannot run because its latest remote state requires a newer compatible client."
            }
            AppError::Validation(reason) if reason.contains("control characters") => {
                "SSH command validation failed: control characters/newlines are not allowed."
            }
            AppError::Validation(reason) if reason.contains("4096") => {
                "SSH command validation failed: command exceeds 4096 characters."
            }
            AppError::Validation(_) => "SSH command validation failed before execution.",
            _ => message,
        };
        Self {
            code: error.code(),
            message,
            details: serde_json::json!({}),
        }
    }

    pub(super) fn initialization_failed() -> Self {
        Self {
            code: "COMMAND_BUS_INITIALIZATION_FAILED",
            message: "The command-bus adapter could not be initialized.",
            details: serde_json::json!({}),
        }
    }
}
