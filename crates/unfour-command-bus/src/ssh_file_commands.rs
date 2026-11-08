use super::*;
use crate::transaction::CommandActivity;
use unfour_core::models::{SshFilePatchInput, SshFileWriteInput};

impl CommandBus {
    /// The adapter applies environment/confirmation policy. File contents never
    /// enter command history or activity details.
    pub async fn write_ssh_file(&self, input: SshFileWriteInput) -> AppResult<SshDiagnosticResult> {
        let details =
            serde_json::json!({"path":input.path,"mode":input.mode,"bytes":input.content.len()});
        let workspace = input.workspace_id.clone();
        let result = self.ssh.write_file(input).await?;
        self.record_file_activity(workspace, "ssh.file.write", details, result)
            .await
    }

    pub async fn patch_ssh_file(&self, input: SshFilePatchInput) -> AppResult<SshDiagnosticResult> {
        let details = serde_json::json!({"path":input.path,"searchBytes":input.search.len(),"replaceBytes":input.replace.len()});
        let workspace = input.workspace_id.clone();
        let result = self.ssh.patch_file(input).await?;
        self.record_file_activity(workspace, "ssh.file.patch", details, result)
            .await
    }

    async fn record_file_activity(
        &self,
        workspace: String,
        action: &'static str,
        mut details: serde_json::Value,
        mut result: SshDiagnosticResult,
    ) -> AppResult<SshDiagnosticResult> {
        details["exitStatus"] = serde_json::json!(result.exit_status);
        result.warnings.extend(
            self.record_completion_activity(
                CommandActivity {
                    workspace_id: Some(workspace),
                    action,
                    target: Some(result.connection_id.clone()),
                    details,
                },
                "ssh",
                action,
            )
            .await,
        );
        Ok(result)
    }
}
