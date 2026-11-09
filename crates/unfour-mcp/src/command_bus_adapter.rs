use unfour_core::models::{
    FlowDefinition, FlowRun, FlowRunInput, FlowRunPage, FlowRunSummary, FlowSummary,
};
use unfour_core::models::{SshFilePatchInput, SshFileWriteInput};
mod contract;
mod implementation;
use unfour_core::models::{ApiCollectionFolder, SavedSql, SavedSqlInput, Workspace};
mod error;
mod unified_runtime;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crate::StorageMode;
use tokio::runtime::{Builder, Runtime};
use unfour_command_bus::{CommandBus, CommandBusExtensions, ReadCommand, ReadCommandResult};
use unfour_core::models::{
    ApiCollection, ApiEnvironment, ApiRequestInput, ApiResponse, ApiSavedRequest,
    CredentialCreateInput, CredentialMetadata, DatabaseConnection, DatabaseConnectionInput,
    DatabaseExportTableInput, DatabaseExportTableResult, DatabaseQueryInput, DatabaseQueryResult,
    DatabaseSchema, DatabaseTableList, DatabaseTableStructure, DatabaseTableStructureInput,
    DatabaseTestResult, KeyValue, SshCommandHistoryEntry, SshCommandHistoryQuery, SshConnection,
    SshConnectionInput, SshDiagnosticInput, SshDiagnosticResult, SshTask, SshTaskCancelInput,
    SshTaskCleanupInput, SshTaskCleanupResult, SshTaskDetail, SshTaskRun, SshTaskRunInput,
    SshTaskSaveInput, SshTasksReorderInput, SystemHealth, WorkspaceEnvironment,
    WorkspaceEnvironmentVariable, WorkspaceVariable, WorkspaceVariableInput,
};

use unified_runtime::unified_command_bus;

pub use contract::CommandBusAdapter;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandBusAdapterError {
    pub code: &'static str,
    pub message: &'static str,
    pub details: serde_json::Value,
}

pub struct LocalCommandBusAdapter {
    // Wrapped in an `Option` behind a `Mutex` so `shutdown` can take
    // ownership of the runtime for a *bounded* shutdown. The default
    // `Runtime::Drop` blocks until every spawned task finishes, which is what
    // lets a lingering SSH supervisor / pool keep-alive hang the process after
    // the stdio loop ends. Taking it out lets us call `shutdown_timeout`.
    runtime: Mutex<Option<Runtime>>,
    bus: CommandBus,
}

type AdapterResult = Result<Arc<LocalCommandBusAdapter>, CommandBusAdapterError>;

impl LocalCommandBusAdapter {
    pub fn default_storage_read_only() -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_default_storage_read_only())
    }

    /// Opens the Community-compatible storage path without Cloud Sync.
    ///
    /// This is retained for explicit Community/test integrations. Production
    /// mutable MCP must use [`Self::from_storage_mode`] so the unified
    /// migration and `SyncOutboxHook` are installed.
    #[deprecated(
        note = "raw Community/test constructor; use from_storage_mode for production mutable MCP"
    )]
    pub fn default_storage() -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_default_storage())
    }

    pub fn from_storage_mode(mode: StorageMode) -> AdapterResult {
        Self::from_command_bus_future(unified_command_bus(mode))
    }

    pub fn from_env() -> AdapterResult {
        Self::from_storage_mode(StorageMode::from_env())
    }

    pub fn default_storage_with_extensions(extensions: CommandBusExtensions) -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_default_storage_with_extensions(
            extensions,
        ))
    }

    pub fn default_database_path() -> Result<std::path::PathBuf, CommandBusAdapterError> {
        unfour_command_bus::default_database_path()
            .map_err(|_| CommandBusAdapterError::initialization_failed())
    }

    pub fn from_storage_dir_read_only(storage_dir: impl AsRef<Path>) -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_storage_dir_read_only(storage_dir))
    }

    /// Opens a raw Community/test storage directory without Cloud Sync.
    /// Production mutable MCP must use the unified storage-mode constructor.
    #[deprecated(
        note = "raw Community/test constructor; use from_storage_mode for production mutable MCP"
    )]
    pub fn from_storage_dir(storage_dir: impl AsRef<Path>) -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_storage_dir(storage_dir))
    }

    pub fn from_storage_dir_with_extensions(
        storage_dir: impl AsRef<Path>,
        extensions: CommandBusExtensions,
    ) -> AdapterResult {
        Self::from_command_bus_future(CommandBus::from_existing_storage_dir_with_extensions(
            storage_dir,
            extensions,
        ))
    }

    /// Creates an isolated raw test bus without Cloud Sync.
    /// Production mutable MCP must use `from_storage_mode(StorageMode::Ephemeral)`.
    #[deprecated(note = "raw test constructor; use from_storage_mode for production mutable MCP")]
    pub fn ephemeral() -> AdapterResult {
        Self::from_command_bus_future(CommandBus::ephemeral())
    }

    fn from_command_bus_future<E>(
        command_bus: impl std::future::Future<Output = Result<CommandBus, E>>,
    ) -> AdapterResult {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| CommandBusAdapterError::initialization_failed())?;
        let bus = runtime
            .block_on(command_bus)
            .map_err(|_| CommandBusAdapterError::initialization_failed())?;

        Ok(Arc::new(Self {
            runtime: Mutex::new(Some(runtime)),
            bus,
        }))
    }
}

impl LocalCommandBusAdapter {
    /// Run a future to completion on the internal multi-thread runtime. Every
    /// adapter method goes through this so the `Runtime` stays behind an
    /// interior-mutable `Option`, which lets [`LocalCommandBusAdapter::shutdown`]
    /// take ownership of it for a bounded shutdown.
    fn run<F: std::future::Future>(&self, future: F) -> F::Output {
        let handle = self
            .runtime
            .lock()
            .expect("command-bus runtime lock poisoned")
            .as_ref()
            .expect("command-bus runtime already shut down")
            .handle()
            .clone();
        handle.block_on(future)
    }

    fn run_execution<T>(
        &self,
        future: impl std::future::Future<Output = unfour_core::AppResult<T>>,
    ) -> unfour_core::AppResult<T> {
        self.run(crate::call_control::bounded(future))
    }

    /// Cancel all background tokio tasks (SSH supervisors, DB/API pool
    /// keep-alives, fire-and-forget flush tasks) and stop the runtime, bounded
    /// so a stuck task can never block process exit. Idempotent: a second call
    /// is a no-op.
    ///
    /// This is the unified shutdown signal for the stdio process: it releases
    /// SSH sessions, database connections, and the API runtime before the
    /// adapter is dropped.
    pub fn shutdown(&self) {
        let runtime = {
            self.runtime
                .lock()
                .expect("command-bus runtime lock poisoned")
                .take()
        };

        if let Some(runtime) = runtime {
            runtime.shutdown_timeout(Duration::from_secs(2));
        }
    }
}

impl Drop for LocalCommandBusAdapter {
    fn drop(&mut self) {
        // Bounded backstop: if `shutdown` was never called (e.g. a panic
        // before the normal exit path), still guarantee the runtime cannot hang
        // the process on a lingering background task.
        if let Ok(guard) = self.runtime.get_mut() {
            if let Some(runtime) = guard.take() {
                runtime.shutdown_timeout(Duration::from_secs(2));
            }
        }
    }
}

#[cfg(test)]
#[allow(deprecated)]
#[path = "command_bus_adapter_tests/mod.rs"]
mod command_bus_adapter_tests;
