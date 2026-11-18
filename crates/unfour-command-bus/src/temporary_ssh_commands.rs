use super::*;
#[cfg(all(test, feature = "ssh-native"))]
mod tests;
pub use unfour_ssh_engine::ssh::temporary::{
    LaunchReceipt, TemporaryProbeKind, TemporaryProbeReport, TemporaryRequest, TemporarySummary,
};

impl CommandBus {
    pub fn receive_temporary_ssh(
        &self,
        receipt: &LaunchReceipt,
        request: TemporaryRequest,
    ) -> AppResult<String> {
        self.ssh.receive_temporary(receipt, request)
    }
    pub fn list_temporary_ssh(&self, workspace_id: &str) -> AppResult<Vec<TemporarySummary>> {
        self.ssh.list_temporary(workspace_id)
    }
    pub async fn claim_temporary_ssh(
        &self,
        workspace_id: &str,
        handle: &str,
    ) -> AppResult<TemporarySummary> {
        self.ssh.claim_temporary(workspace_id, handle).await
    }
    pub fn start_temporary_ssh(&self, workspace_id: &str, handle: &str) -> AppResult<()> {
        self.ssh.start_temporary(workspace_id, handle)
    }
    pub fn trust_temporary_ssh(
        &self,
        workspace_id: &str,
        handle: &str,
        fingerprint: &str,
    ) -> AppResult<()> {
        self.ssh.trust_temporary(workspace_id, handle, fingerprint)
    }
    pub fn cancel_temporary_ssh(&self, workspace_id: &str, handle: &str) -> AppResult<()> {
        self.ssh.cancel_temporary(workspace_id, handle)
    }
    pub fn close_temporary_ssh_workspace(&self, workspace_id: &str) -> AppResult<()> {
        self.ssh.close_temporary_workspace(workspace_id)
    }
    pub fn start_temporary_ssh_probe(
        &self,
        workspace_id: &str,
        handle: &str,
        kind: TemporaryProbeKind,
        confirmed: bool,
    ) -> AppResult<TemporaryProbeReport> {
        self.ssh
            .start_temporary_probe(workspace_id, handle, kind, confirmed)
    }
    pub fn temporary_ssh_probe(
        &self,
        workspace_id: &str,
        handle: &str,
    ) -> AppResult<Option<TemporaryProbeReport>> {
        self.ssh.temporary_probe(workspace_id, handle)
    }
    pub fn start_temporary_ssh_tcp_probe(
        &self,
        workspace_id: &str,
        handle: &str,
        host: &str,
        port: u16,
        confirmed: bool,
    ) -> AppResult<TemporaryProbeReport> {
        self.ssh
            .start_temporary_tcp_probe(workspace_id, handle, host, port, confirmed)
    }

    /// Explicit execution on the already authenticated temporary target.
    /// Its transport, run results and output never become durable task bindings.
    pub async fn run_temporary_ssh_task(
        &self,
        input: SshTaskRunInput,
        handle: &str,
    ) -> AppResult<SshTaskRun> {
        self.run_temporary_ssh_task_with_mode(input, handle, "exec")
            .await
    }

    /// Terminal reuse is an explicit per-run choice, never an automatic fallback.
    pub async fn run_temporary_ssh_task_with_mode(
        &self,
        mut input: SshTaskRunInput,
        handle: &str,
        execution_mode: &str,
    ) -> AppResult<SshTaskRun> {
        if !matches!(execution_mode, "exec" | "terminal") {
            return Err(unfour_core::AppError::Validation(
                "SSH_TASK_EXECUTION_MODE_INVALID".into(),
            ));
        }
        if input.connection_id.is_some() {
            return Err(unfour_core::AppError::Validation(
                "SSH_TASK_TEMPORARY_TARGET_CONFLICT".into(),
            ));
        }
        for guard in self.extensions.ssh_task_execution_guards() {
            guard.validate(&input.workspace_id, &input.task_id).await?;
        }
        self.prepare_ssh_task_context_for_target(&mut input, false)
            .await?;
        // The engine revalidates the task revision and live workspace ownership
        // after asynchronous preparation. Deliberately no completion activity.
        self.ssh
            .run_temporary_task_with_mode(input, handle, execution_mode)
            .await
    }
    pub fn cancel_temporary_ssh_probe(
        &self,
        workspace_id: &str,
        handle: &str,
        run_id: &str,
    ) -> AppResult<()> {
        self.ssh
            .cancel_temporary_probe(workspace_id, handle, run_id)
    }
    pub fn temporary_ssh_output(
        &self,
        workspace_id: &str,
        handle: &str,
    ) -> AppResult<Vec<SshSessionEvent>> {
        self.ssh.temporary_output(workspace_id, handle)
    }
    pub async fn temporary_ssh_input(&self, input: SshSessionInput) -> AppResult<SshSessionEvent> {
        #[cfg(feature = "ssh-native")]
        return self.ssh.temporary_input(input).await;
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = input;
            Err(unfour_core::AppError::Validation(
                "TEMPORARY_NATIVE_REQUIRED".into(),
            ))
        }
    }
    pub async fn temporary_ssh_resize(&self, input: SshResizeInput) -> AppResult<SshSessionEvent> {
        #[cfg(feature = "ssh-native")]
        return self.ssh.temporary_resize(input).await;
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = input;
            Err(unfour_core::AppError::Validation(
                "TEMPORARY_NATIVE_REQUIRED".into(),
            ))
        }
    }
}
