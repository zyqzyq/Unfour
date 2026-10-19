use crate::{transaction::CommandActivity, CommandBus};

impl CommandBus {
    /// An external effect has already completed or started. Activity failure
    /// cannot undo it and must not turn its result into a retryable command error.
    pub(crate) async fn record_completion_activity(
        &self,
        activity: CommandActivity,
        domain: &str,
        operation: &str,
    ) -> Vec<String> {
        match self
            .activity_log
            .record(
                activity.workspace_id.as_deref(),
                activity.action,
                activity.target.as_deref(),
                activity.details,
            )
            .await
        {
            Ok(_) => Vec::new(),
            Err(error) => {
                // Never log the raw storage error or SQL/command/credential values.
                unfour_diag::log_operation_event(
                    "activity_record_failed",
                    domain,
                    operation,
                    "error",
                    None,
                    Some(unfour_diag::app_error_kind(&error)),
                    serde_json::json!({}),
                );
                vec!["ACTIVITY_RECORD_FAILED".into()]
            }
        }
    }
}
