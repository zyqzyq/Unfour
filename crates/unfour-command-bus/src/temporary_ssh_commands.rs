use super::*;
#[cfg(all(test, feature = "ssh-native"))]
mod tests;
pub use unfour_ssh_engine::ssh::temporary::{LaunchReceipt, TemporaryRequest, TemporarySummary};

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
