use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use async_trait::async_trait;
use serde::Serialize;
use tauri::State;
use unfour_account::AccountService;
use unfour_cloud_sync::{
    CloudWorkspace, DesktopSessionCredential, DesktopSessionProvider, DownloadDecision,
    SyncConflictView, SyncDiagnostics, SyncEntityType, SyncError, SyncService, SyncStatus,
    CLOUD_SYNC_ENTITLEMENT,
};

pub struct AccountTokenProvider {
    account: AccountService,
    access: SyncAccessGate,
}

impl AccountTokenProvider {
    pub(crate) fn new(account: AccountService, access: SyncAccessGate) -> Arc<Self> {
        Arc::new(Self { account, access })
    }
}

#[derive(Clone, Default)]
pub(crate) struct SyncAccessGate {
    allowed: Arc<AtomicBool>,
}

impl SyncAccessGate {
    pub(crate) fn allow(&self) {
        self.allowed.store(true, Ordering::SeqCst);
    }

    pub(crate) fn deny(&self) {
        self.allowed.store(false, Ordering::SeqCst);
    }

    pub(crate) fn is_allowed(&self) -> bool {
        self.allowed.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl DesktopSessionProvider for AccountTokenProvider {
    async fn session_for_cloud_sync(&self) -> Result<DesktopSessionCredential, SyncError> {
        if !self.access.is_allowed() {
            return Err(SyncError::Unauthorized);
        }
        let session = self
            .account
            .require_entitlement(CLOUD_SYNC_ENTITLEMENT)
            .await
            .map_err(|_| SyncError::Unauthorized)?;
        if !self.access.is_allowed() {
            return Err(SyncError::Unauthorized);
        }
        DesktopSessionCredential::new(
            session.desktop_session_token().to_string(),
            session.account_id().to_string(),
            session.generation(),
        )
    }

    fn generation(&self) -> u64 {
        self.account.generation()
    }

    fn invalidate_cloud_sync(&self) {
        self.account.invalidate_entitlement_cache();
    }
}

pub struct SyncAppState {
    pub(crate) service: SyncService,
    pub(crate) access: SyncAccessGate,
}

impl SyncAppState {
    pub fn new(service: SyncService, access: SyncAccessGate) -> Self {
        Self { service, access }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCommandError {
    code: &'static str,
    message: &'static str,
}

impl From<SyncError> for SyncCommandError {
    fn from(error: SyncError) -> Self {
        Self {
            code: error.code(),
            message: match error {
                SyncError::Unauthorized => "Cloud Sync requires an active cloud_sync entitlement.",
                SyncError::ProtocolIncompatible => "The Cloud Sync protocol is incompatible.",
                SyncError::NotFound => "The Cloud Sync resource was not found.",
                SyncError::InvalidData => "Cloud Sync rejected invalid data.",
                SyncError::Transport => "Cloud Sync could not confirm the server result.",
                SyncError::Storage => "Cloud Sync local storage failed.",
                SyncError::Core => "Cloud Sync could not apply the workspace change.",
                SyncError::AccountChanged => "The signed-in account changed during Cloud Sync.",
                SyncError::Conflict => "Cloud Sync requires conflict resolution.",
                SyncError::LocalWorkspaceNotEmpty => {
                    "The local workspace is not empty; Cloud Sync refused to overwrite it."
                }
                SyncError::WorkspaceNameConflict => {
                    "An active local workspace already uses the cloud workspace name."
                }
                SyncError::SafeReplaceUnavailable => "Safe cloud replacement is not available.",
                SyncError::CloudWorkspaceNotEmpty => {
                    "The cloud workspace already contains data. This version will not overwrite or merge it."
                }
                SyncError::DeadLetterBlocked => {
                    "Cloud Sync is blocked by a permanently failed operation."
                }
                SyncError::Permanent => "Cloud Sync rejected a permanent request error.",
                SyncError::WorkspaceOwnedByAnotherAccount => {
                    "This local workspace is already owned by another Cloud Sync account."
                }
                SyncError::WorkspaceOwnershipAmbiguous => {
                    "Cloud Sync found multiple historical owners for this workspace and stopped safely."
                }
                SyncError::WorkspaceOwnershipInvariant => {
                    "Cloud Sync workspace ownership metadata is inconsistent."
                }
            },
        }
    }
}

#[tauri::command]
pub async fn cloud_sync_enable(
    workspace_id: String,
    state: State<'_, SyncAppState>,
) -> Result<(), SyncCommandError> {
    state
        .service
        .enable(&workspace_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_disable(
    workspace_id: String,
    state: State<'_, SyncAppState>,
) -> Result<(), SyncCommandError> {
    state
        .service
        .disable(&workspace_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_status(
    workspace_id: String,
    state: State<'_, SyncAppState>,
) -> Result<SyncStatus, SyncCommandError> {
    state
        .service
        .status(&workspace_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_global_status(
    state: State<'_, SyncAppState>,
) -> Result<bool, SyncCommandError> {
    state
        .service
        .global_sync_enabled()
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_set_global_enabled(
    enabled: bool,
    state: State<'_, SyncAppState>,
) -> Result<(), SyncCommandError> {
    state
        .service
        .set_global_sync_enabled(enabled)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_diagnostics(
    workspace_id: String,
    state: State<'_, SyncAppState>,
) -> Result<Option<SyncDiagnostics>, SyncCommandError> {
    state
        .service
        .diagnostics(&workspace_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_now(
    workspace_id: String,
    state: State<'_, SyncAppState>,
) -> Result<(), SyncCommandError> {
    state
        .service
        .sync_workspace(&workspace_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_retry_dead_letter_current_local(
    workspace_id: String,
    operation_id: String,
    state: State<'_, SyncAppState>,
) -> Result<String, SyncCommandError> {
    state
        .service
        .retry_dead_letter_current_local(&workspace_id, &operation_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_use_remote_dead_letter(
    workspace_id: String,
    operation_id: String,
    state: State<'_, SyncAppState>,
) -> Result<(), SyncCommandError> {
    state
        .service
        .use_remote_dead_letter(&workspace_id, &operation_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_all(state: State<'_, SyncAppState>) -> Result<(), SyncCommandError> {
    state.service.sync_all().await.map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_list_workspaces(
    state: State<'_, SyncAppState>,
) -> Result<Vec<CloudWorkspace>, SyncCommandError> {
    state
        .service
        .list_cloud_workspaces()
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_download(
    cloud_workspace_id: String,
    decision: DownloadDecision,
    state: State<'_, SyncAppState>,
) -> Result<String, SyncCommandError> {
    state
        .service
        .download_workspace(&cloud_workspace_id, decision)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_conflicts(
    workspace_id: String,
    state: State<'_, SyncAppState>,
) -> Result<Vec<SyncConflictView>, SyncCommandError> {
    state
        .service
        .conflicts(&workspace_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_keep_local(
    workspace_id: String,
    entity_type: SyncEntityType,
    entity_id: String,
    state: State<'_, SyncAppState>,
) -> Result<(), SyncCommandError> {
    state
        .service
        .keep_local(&workspace_id, entity_type, &entity_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn cloud_sync_use_remote(
    workspace_id: String,
    entity_type: SyncEntityType,
    entity_id: String,
    state: State<'_, SyncAppState>,
) -> Result<(), SyncCommandError> {
    state
        .service
        .use_remote(&workspace_id, entity_type, &entity_id)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use unfour_cloud_sync::{DeadLetterView, SyncBinding};

    #[test]
    fn closed_access_gate_rejects_new_cloud_sessions_before_account_io() {
        tauri::async_runtime::block_on(async {
            let access = SyncAccessGate::default();
            let account = AccountService::new(
                "https://account.example.test",
                "https://account.example.test",
                false,
            )
            .expect("valid account configuration");
            let provider = AccountTokenProvider::new(account, access);

            assert_eq!(
                provider.session_for_cloud_sync().await.unwrap_err(),
                SyncError::Unauthorized
            );
        });
    }

    #[test]
    fn command_errors_do_not_include_remote_payloads_or_tokens() {
        let error = SyncCommandError::from(SyncError::Unauthorized);
        let encoded = serde_json::to_string(&error).expect("serialize");
        assert_eq!(error.code, "cloud_sync_unauthorized");
        for forbidden in ["sessionToken", "remotePayload", "secretValue"] {
            assert!(!encoded.contains(forbidden));
        }
    }

    #[test]
    fn sync_status_ipc_exposes_dead_and_delivery_counts() {
        let status = SyncStatus {
            binding: Some(SyncBinding {
                account_id: "account".into(),
                local_workspace_id: "workspace".into(),
                cloud_workspace_id: "cloud".into(),
                last_pulled_cursor: 1,
                sync_enabled: true,
                state: "error".into(),
                initial_cursor: Some(0),
                initial_total: 1,
                initial_confirmed: 0,
                initialization_checkpoint: None,
                ssh_task_v3_bootstrap_state: "completed".into(),
                connection_v4_bootstrap_state: "completed".into(),
                generation: 1,
                last_success_at: None,
                last_error: Some("cloud_sync_dead_letter_blocked".into()),
                consecutive_failure_count: 2,
            }),
            pending_count: 2,
            uncertain_count: 3,
            in_flight_count: 4,
            dead_count: 5,
            dead_letters: vec![DeadLetterView {
                operation_id: "operation".into(),
                entity_type: "workspaceVariable".into(),
                entity_id: "variable".into(),
                entity_name: Some("API_HOST".into()),
                error_code: "invalid_sync_entity".into(),
            }],
            conflict_count: 6,
            running: false,
        };
        let value = serde_json::to_value(status).expect("serialize sync status");
        assert_eq!(value["pendingCount"], 2);
        assert_eq!(value["uncertainCount"], 3);
        assert_eq!(value["inFlightCount"], 4);
        assert_eq!(value["deadCount"], 5);
        assert_eq!(value["deadLetters"][0]["entityName"], "API_HOST");
        assert_eq!(
            value["binding"]["lastError"],
            "cloud_sync_dead_letter_blocked"
        );
    }
}
