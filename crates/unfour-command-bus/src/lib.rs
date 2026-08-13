mod activity_summary;
mod storage_paths;
mod transaction;

use activity_summary::{command_activity_kind, truncate_url_preview};
use serde::{Deserialize, Serialize};
pub use storage_paths::{default_database_path, DEFAULT_DATABASE_FILE, DEFAULT_PRODUCT_DATA_DIR};
use unfour_core::ai_reserved;
use unfour_core::models::{
    ApiCollection, ApiCollectionExportArtifact, ApiCollectionExportFormat, ApiCollectionFolder,
    ApiCollectionImportResult, ApiEnvironment, ApiHistoryDetail, ApiHistoryItem, ApiRequestInput,
    ApiResponse, ApiSavedRequest, CredentialCreateInput, CredentialDeleteInput,
    CredentialInspectInput, CredentialMetadata, CredentialRotateInput, DatabaseBrowseInput,
    DatabaseBrowseResult, DatabaseConnection, DatabaseConnectionInput, DatabaseQueryInput,
    DatabaseQueryResult, DatabaseRowMutationInput, DatabaseRowMutationResult, DatabaseSchema,
    DatabaseTableStructure, DatabaseTableStructureInput, DatabaseTestResult, DbQueryHistoryEntry,
    DbQueryHistoryRecordInput, KeyValue, SavedSql, SavedSqlInput, SftpCancelTransferInput,
    SftpDeleteInput, SftpDirectoryListing, SftpFileEntry, SftpOpenResult, SftpPathInput,
    SftpRenameInput, SftpSessionInput, SftpTransferInput, SftpTransferState, SshCloseInput,
    SshCommandHistoryEntry, SshCommandHistoryQuery, SshConnectInput, SshConnection,
    SshConnectionInput, SshDiagnosticInput, SshDiagnosticResult, SshHostFingerprintInfo,
    SshHostKeyInput, SshKnownHostsExportInput, SshKnownHostsExportResult, SshKnownHostsImportInput,
    SshKnownHostsImportResult, SshLogExport, SshLogExportInput, SshReconnectCancelInput,
    SshResizeInput, SshSessionEvent, SshSessionInput, SshSessionSummary, SshTask,
    SshTaskCancelInput, SshTaskCleanupInput, SshTaskCleanupResult, SshTaskDetail, SshTaskRun,
    SshTaskRunInput, SshTaskSaveInput, SshTasksReorderInput, SshTestResult, SystemHealth,
    Workspace, WorkspaceEnvironment, WorkspaceEnvironmentVariable, WorkspaceLayout, WorkspaceState,
    WorkspaceVariable, WorkspaceVariableInput,
};
use unfour_core::AppResult;
use unfour_database_engine::DatabaseService;
use unfour_http_engine::ApiClientService;
use unfour_local_storage::{ActivityLogService, LocalDb};
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::SshService;
use unfour_workspace_engine::WorkspaceService;

mod api_commands;
mod api_script_commands;
mod command_models;
mod core_commands;
mod credential_commands;
mod database_commands;
mod domain_commands;
mod ssh_commands;
mod workspace_variable_commands;

pub use command_models::*;
pub use transaction::{CommandBusExtensions, TransactionalCommandHook};

/// OS keychain service name under which credentials are stored. Must match the
/// value the desktop app passes to `SecretStore::new` (see
/// `apps/desktop/src-tauri/src/lib.rs`) so satellite processes read the same
/// credential entries.
pub const DEFAULT_SECRET_SERVICE: &str = "unfour";

/// Default and maximum number of activity events returned by `ListActivity`.
const DEFAULT_ACTIVITY_LIMIT: i64 = 50;
const MAX_ACTIVITY_LIMIT: i64 = 200;

#[derive(Clone)]
pub struct CommandBus {
    pub(crate) db: LocalDb,
    pub(crate) extensions: CommandBusExtensions,
    pub(crate) api_client: ApiClientService,
    pub(crate) activity_log: ActivityLogService,
    pub(crate) database: DatabaseService,
    pub(crate) secret_store: SecretStore,
    pub(crate) ssh: SshService,
    pub(crate) workspace: WorkspaceService,
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
