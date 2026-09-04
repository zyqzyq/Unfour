//! Cloud Sync runtime construction and account/workspace lifecycle API.
//!
//! Worker scheduling and network phases live in private submodules. Core data
//! application is centralized in `external_apply`; repository methods persist
//! sync metadata on the same transaction without changing the public facade.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Notify, OnceCell, Semaphore};
use unfour_command_bus::{CommandBus, TransactionalCommandHook, DEFAULT_SECRET_SERVICE};
use unfour_database_engine::DatabaseService;
use unfour_http_engine::ApiClientService;
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::SshService;
use unfour_workspace_engine::WorkspaceService;

use crate::canonical::snapshot_workspace_name;
use crate::{
    CloudWorkspace, SyncAccountContext, SyncDependencies, SyncEntityType, SyncError,
    SyncOutboxHook, SyncRepository, SyncStatus, SyncTransport, TransportError, PROTOCOL_VERSION,
};
use worker::FlightState;

mod bootstrap;
mod conflicts;
mod external_apply;
mod pull;
mod push;
mod recovery;
mod snapshots;
mod worker;

const MAX_REMOTE_PAGES: usize = 10_000;
const GLOBAL_WORKSPACE_CONCURRENCY: usize = 4;

#[derive(Clone)]
pub struct SyncService {
    repository: SyncRepository,
    db: LocalDb,
    core: Arc<OnceCell<CommandBus>>,
    workspace: WorkspaceService,
    api_client: ApiClientService,
    ssh: SshService,
    database: DatabaseService,
    transport: Arc<dyn SyncTransport>,
    dependencies: SyncDependencies,
    flights: Arc<Mutex<HashMap<String, FlightState>>>,
    global_limit: Arc<Semaphore>,
    worker_id: String,
    /// Set only when a verified account transition may have made previously
    /// account-paused bindings runnable. The desktop access gate consumes it
    /// after it is opened, so activation never races the entitlement check.
    activation_sync_pending: Arc<Mutex<Option<SyncAccountContext>>>,
    /// Accounts whose `protocol_version_unsupported` dead letters were already
    /// revived by this process (an upgraded build speaks a newer protocol, so
    /// one retry per app run is warranted).
    protocol_revival_done: Arc<Mutex<HashSet<String>>>,
    retry_scheduler_wakeup: Arc<Notify>,
}

pub struct SyncRuntime;

impl SyncRuntime {
    pub fn build(
        db: LocalDb,
        transport: Arc<dyn SyncTransport>,
    ) -> (
        SyncService,
        Arc<dyn TransactionalCommandHook>,
        mpsc::UnboundedReceiver<String>,
    ) {
        Self::build_with_dependencies(db, transport, SyncDependencies::default())
    }

    pub fn build_with_dependencies(
        db: LocalDb,
        transport: Arc<dyn SyncTransport>,
        dependencies: SyncDependencies,
    ) -> (
        SyncService,
        Arc<dyn TransactionalCommandHook>,
        mpsc::UnboundedReceiver<String>,
    ) {
        let repository = SyncRepository::new(db.pool().clone());
        let (trigger, receiver) = mpsc::unbounded_channel();
        let hook = Arc::new(SyncOutboxHook::new(
            dependencies.ids.clone(),
            dependencies.clock.clone(),
            Some(trigger),
        ));
        let worker_id = dependencies.ids.next_id();
        let secret_store = SecretStore::new(DEFAULT_SECRET_SERVICE);
        let service = SyncService {
            repository,
            db: db.clone(),
            core: Arc::new(OnceCell::new()),
            workspace: WorkspaceService::new(db.clone()),
            api_client: ApiClientService::new(db.clone()),
            ssh: SshService::new(db.clone(), secret_store.clone()),
            database: DatabaseService::new(db).with_secret_store(secret_store),
            transport,
            dependencies,
            flights: Arc::new(Mutex::new(HashMap::new())),
            global_limit: Arc::new(Semaphore::new(GLOBAL_WORKSPACE_CONCURRENCY)),
            worker_id,
            activation_sync_pending: Arc::new(Mutex::new(None)),
            protocol_revival_done: Arc::new(Mutex::new(HashSet::new())),
            retry_scheduler_wakeup: Arc::new(Notify::new()),
        };
        (service, hook, receiver)
    }
}

impl SyncService {
    pub fn repository(&self) -> &SyncRepository {
        &self.repository
    }

    async fn core(&self) -> Result<&CommandBus, SyncError> {
        self.core
            .get_or_try_init(|| async {
                CommandBus::from_existing_db_without_seeding(self.db.clone())
                    .await
                    .map_err(|_| SyncError::Core)
            })
            .await
    }

    async fn account(&self) -> Result<SyncAccountContext, SyncError> {
        let account = self
            .transport
            .account_context()
            .await
            .map_err(SyncError::from)?;
        self.repository
            .activate_account(
                &account.account_id,
                account.generation,
                self.dependencies.clock.now(),
            )
            .await?;
        Ok(account)
    }

    fn account_is_current(&self, account: &SyncAccountContext) -> Result<(), SyncError> {
        (self.transport.account_generation() == account.generation)
            .then_some(())
            .ok_or(SyncError::AccountChanged)
    }

    pub async fn pause_current_account(&self) -> Result<(), SyncError> {
        self.repository
            .deactivate_active_account(self.dependencies.clock.now())
            .await
    }

    pub async fn activate_account_context(&self) -> Result<(), SyncError> {
        self.account().await.map(|_| ())
    }

    /// Commits an account already verified by the remote account endpoint.
    /// Subsequent network operations still call `account()` and revalidate.
    pub async fn activate_verified_account_context(
        &self,
        account: SyncAccountContext,
    ) -> Result<(), SyncError> {
        let context_changed = self
            .repository
            .activate_account(
                &account.account_id,
                account.generation,
                self.dependencies.clock.now(),
            )
            .await?;
        if context_changed {
            *self
                .activation_sync_pending
                .lock()
                .expect("activation sync lock poisoned") = Some(account);
        }
        Ok(())
    }

    async fn record_remote_problem(
        &self,
        account_id: &str,
        cloud_workspace_id: Option<&str>,
        problem: &crate::RemoteSyncProblem,
    ) {
        let _ = self
            .repository
            .record_remote_problem(
                account_id,
                cloud_workspace_id,
                problem,
                self.dependencies.clock.now(),
            )
            .await;
    }

    fn wake_retry_scheduler(&self) {
        self.retry_scheduler_wakeup.notify_one();
    }

    async fn finish_transport<T>(
        &self,
        account_id: &str,
        cloud_workspace_id: Option<&str>,
        result: Result<T, TransportError>,
    ) -> Result<T, SyncError> {
        match result {
            Ok(value) => Ok(value),
            Err(TransportError::Remote(problem)) => {
                self.record_remote_problem(account_id, cloud_workspace_id, &problem)
                    .await;
                Err(problem.sync_error())
            }
            Err(TransportError::RemoteConflict { problem, .. }) => {
                self.record_remote_problem(account_id, cloud_workspace_id, &problem)
                    .await;
                Err(SyncError::Conflict)
            }
            Err(error) => Err(SyncError::from(error)),
        }
    }

    /// Starts the first post-activation sync only after the caller has opened
    /// its credential/entitlement gate. Repeated account-state refreshes do
    /// not schedule duplicate sync-all runs.
    pub fn schedule_account_sync(&self) {
        let pending = self
            .activation_sync_pending
            .lock()
            .expect("activation sync lock poisoned")
            .take();
        if pending.is_some() {
            let service = self.clone();
            tokio::spawn(async move {
                let _ = service.sync_all().await;
            });
        }
    }

    pub async fn deactivate_account_context(&self) -> Result<(), SyncError> {
        self.repository
            .deactivate_active_account(self.dependencies.clock.now())
            .await
    }

    pub async fn list_cloud_workspaces(&self) -> Result<Vec<CloudWorkspace>, SyncError> {
        let account = self.account().await?;
        let listed = self.transport.list_workspaces().await;
        let mut workspaces = self
            .finish_transport(&account.account_id, None, listed)
            .await?;
        for workspace in &mut workspaces {
            if workspace
                .name
                .as_deref()
                .is_some_and(|name| !name.trim().is_empty())
            {
                continue;
            }
            if let Some(name) = self
                .repository
                .local_workspace_name(&workspace.root_entity_id)
                .await?
            {
                workspace.name = Some(name);
                continue;
            }
            let page = match self
                .transport
                .snapshot(
                    &workspace.cloud_workspace_id,
                    Some(workspace.current_cursor),
                    None,
                )
                .await
            {
                Ok(page) => page,
                Err(TransportError::Remote(problem)) => {
                    self.record_remote_problem(
                        &account.account_id,
                        Some(&workspace.cloud_workspace_id),
                        &problem,
                    )
                    .await;
                    match problem.sync_error() {
                        error @ (SyncError::Unauthorized
                        | SyncError::EntitlementRequired
                        | SyncError::ProtocolIncompatible) => return Err(error),
                        _ => continue,
                    }
                }
                Err(TransportError::RemoteConflict { problem, .. }) => {
                    self.record_remote_problem(
                        &account.account_id,
                        Some(&workspace.cloud_workspace_id),
                        &problem,
                    )
                    .await;
                    continue;
                }
                Err(TransportError::Unauthorized) => return Err(SyncError::Unauthorized),
                Err(TransportError::EntitlementRequired) => {
                    return Err(SyncError::EntitlementRequired)
                }
                Err(TransportError::ProtocolIncompatible) => {
                    return Err(SyncError::ProtocolIncompatible)
                }
                Err(_) => continue,
            };
            if page.protocol_version != PROTOCOL_VERSION
                || page.cloud_workspace_id != workspace.cloud_workspace_id
                || page.at_cursor != workspace.current_cursor
            {
                let _ = self
                    .repository
                    .record_local_diagnostic(
                        &account.account_id,
                        Some(&workspace.cloud_workspace_id),
                        "permanent",
                        "snapshot_invalid_response",
                        crate::SyncPhase::Snapshot,
                        self.dependencies.clock.now(),
                    )
                    .await;
                continue;
            }
            if let Some(item) = page.items.iter().find(|item| {
                item.entity_type == SyncEntityType::Workspace
                    && item.entity_id == workspace.root_entity_id
            }) {
                match snapshot_workspace_name(&workspace.root_entity_id, item) {
                    Ok(name) => workspace.name = name,
                    Err(_) => {
                        let _ = self
                            .repository
                            .record_local_diagnostic(
                                &account.account_id,
                                Some(&workspace.cloud_workspace_id),
                                "permanent",
                                "snapshot_invalid_response",
                                crate::SyncPhase::Snapshot,
                                self.dependencies.clock.now(),
                            )
                            .await;
                    }
                }
            }
        }
        self.account_is_current(&account)?;
        Ok(workspaces)
    }

    pub async fn global_sync_enabled(&self) -> Result<bool, SyncError> {
        let account = self.account().await?;
        self.repository
            .global_sync_enabled(&account.account_id)
            .await
    }

    pub async fn set_global_sync_enabled(&self, enabled: bool) -> Result<(), SyncError> {
        let account = self.account().await?;
        self.repository
            .set_global_sync_enabled(&account.account_id, enabled, self.dependencies.clock.now())
            .await?;
        if enabled {
            let service = self.clone();
            tokio::spawn(async move {
                let _ = service.sync_all().await;
            });
        }
        self.wake_retry_scheduler();
        Ok(())
    }

    pub async fn enable(&self, workspace_id: &str) -> Result<(), SyncError> {
        let account = self.account().await?;
        if let Some(owner) = self
            .repository
            .resolve_cloud_sync_owner(workspace_id)
            .await?
        {
            if owner.account_id != account.account_id {
                return Err(SyncError::WorkspaceOwnedByAnotherAccount);
            }
        }
        if let Some(binding) = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
        {
            self.repository
                .set_enabled(
                    &account.account_id,
                    workspace_id,
                    true,
                    self.dependencies.clock.now(),
                )
                .await?;
            if binding.sync_enabled
                && binding.state == "active"
                && binding.ssh_task_v3_bootstrap_state == "completed"
                && binding.connection_v4_bootstrap_state == "completed"
                && self
                    .repository
                    .api_v2_bootstrap_completed(&account.account_id, workspace_id)
                    .await?
            {
                return Ok(());
            }
            if !self
                .repository
                .global_sync_enabled(&account.account_id)
                .await?
            {
                return Ok(());
            }
            return self.sync_workspace_for(account, workspace_id).await;
        }
        let created = self.transport.create_workspace(workspace_id).await;
        let cloud = self
            .finish_transport(&account.account_id, None, created)
            .await?;
        self.account_is_current(&account)?;
        if cloud.cloud_workspace_id.trim().is_empty()
            || cloud.root_entity_id != workspace_id
            || cloud.current_cursor < 0
        {
            return Err(SyncError::InvalidData);
        }
        if cloud.current_cursor > 0 {
            return Err(SyncError::CloudWorkspaceNotEmpty);
        }
        self.repository
            .create_binding_with_initial_outbox_and_domain_entities(
                &account.account_id,
                account.generation,
                workspace_id,
                &cloud.cloud_workspace_id,
                cloud.current_cursor,
                Some(&self.api_client),
                Some(&self.ssh),
                self.dependencies.ids.as_ref(),
                self.dependencies.clock.as_ref(),
            )
            .await?;
        if !self
            .repository
            .global_sync_enabled(&account.account_id)
            .await?
        {
            return Ok(());
        }
        self.sync_workspace_for(account, workspace_id).await
    }

    pub async fn disable(&self, workspace_id: &str) -> Result<(), SyncError> {
        let account = self.account().await?;
        self.repository
            .set_enabled(
                &account.account_id,
                workspace_id,
                false,
                self.dependencies.clock.now(),
            )
            .await?;
        self.wake_retry_scheduler();
        Ok(())
    }

    pub async fn status(&self, workspace_id: &str) -> Result<SyncStatus, SyncError> {
        let account = self.account().await?;
        let key = Self::flight_key(&account.account_id, workspace_id);
        self.repository
            .status(&account.account_id, workspace_id, self.is_running(&key))
            .await
    }

    pub async fn diagnostics(
        &self,
        workspace_id: &str,
    ) -> Result<Option<crate::SyncDiagnostics>, SyncError> {
        let account = self.account().await?;
        self.repository
            .diagnostics(&account.account_id, workspace_id)
            .await
    }
}
