use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::SqliteConnection;
use tokio::sync::{mpsc, OnceCell, Semaphore};
use unfour_command_bus::{CommandBus, TransactionalCommandHook, DEFAULT_SECRET_SERVICE};
use unfour_core::domain::{
    validate_external_connection_delete, CommandContext, DomainEntityKey, DomainEntityType,
    DomainSnapshot, ExternalApplyPage, ExternalConnectionApply, ExternalWorkspaceApply,
    MutationOrigin, DATABASE_CONNECTION_TYPE, SSH_CONNECTION_TYPE,
};
use unfour_database_engine::{DatabaseConnectionCleanup, DatabaseService};
use unfour_http_engine::ApiClientService;
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::{SshConnectionCleanup, SshService};
use unfour_workspace_engine::WorkspaceService;

use crate::canonical::snapshot_workspace_name;
use crate::conflict_scope;
use crate::{
    canonical_payload, parse_remote_change, parse_snapshot_item, ChangesPage, CloudWorkspace,
    DownloadDecision, OutboxEntry, PushOperation, PushRequest, RemoteChange, SnapshotItem,
    SyncAccountContext, SyncBinding, SyncConflictView, SyncDependencies, SyncEntityType, SyncError,
    SyncOperation, SyncOutboxHook, SyncRepository, SyncStatus, SyncTransport, TransportError,
    PROTOCOL_VERSION,
};

mod recovery;

const PUSH_BATCH_LIMIT: i64 = 50;
const PUSH_BATCH_MAX_BYTES: usize = 512 * 1024;
const PULL_PAGE_LIMIT: usize = 200;
const MAX_REMOTE_PAGES: usize = 10_000;
const LOCAL_DEBOUNCE: Duration = Duration::from_millis(500);
const PERIODIC_PULL: Duration = Duration::from_secs(5 * 60);
const GLOBAL_WORKSPACE_CONCURRENCY: usize = 4;

#[derive(Default)]
struct FlightState {
    running: bool,
    dirty: bool,
}

struct FlightGuard {
    flights: Arc<Mutex<HashMap<String, FlightState>>>,
    key: String,
}

impl FlightGuard {
    fn take_dirty_or_finish(&mut self) -> bool {
        let mut flights = self.flights.lock().expect("sync flight lock poisoned");
        if let Some(state) = flights.get_mut(&self.key) {
            if state.dirty {
                state.dirty = false;
                return true;
            }
        }
        false
    }
}

impl Drop for FlightGuard {
    fn drop(&mut self) {
        if let Ok(mut flights) = self.flights.lock() {
            flights.remove(&self.key);
        }
    }
}

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
    /// Accounts whose `protocol_version_unsupported` dead letters were already
    /// revived by this process (an upgraded build speaks a newer protocol, so
    /// one retry per app run is warranted).
    protocol_revival_done: Arc<Mutex<HashSet<String>>>,
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
            protocol_revival_done: Arc::new(Mutex::new(HashSet::new())),
        };
        (service, hook, receiver)
    }
}

#[derive(Default)]
struct ExternalApplyCleanup {
    ssh_connections: Vec<SshConnectionCleanup>,
    database_connections: Vec<DatabaseConnectionCleanup>,
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

    async fn connection_snapshots(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DomainSnapshot>, SyncError> {
        let core = self.core().await?;
        let keys = core
            .list_connection_domain_entities(workspace_id.to_string())
            .await
            .map_err(|_| SyncError::Core)?;
        let mut snapshots = Vec::with_capacity(keys.len());
        for key in keys {
            if key.entity_type != DomainEntityType::Connection
                || key.workspace_id != workspace_id
                || key.parent_entity_id.is_some()
            {
                return Err(SyncError::InvalidData);
            }
            snapshots.push(
                core.read_domain_snapshot(&key)
                    .await
                    .map_err(|_| SyncError::Core)?,
            );
        }
        Ok(snapshots)
    }

    async fn connection_snapshot_items(
        &self,
        account: &SyncAccountContext,
        binding: &SyncBinding,
    ) -> Result<Vec<SnapshotItem>, SyncError> {
        let mut fixed_cursor = None;
        let mut page_token = None;
        let mut items = Vec::new();
        for _ in 0..MAX_REMOTE_PAGES {
            let page = self
                .transport
                .snapshot(
                    &binding.cloud_workspace_id,
                    fixed_cursor,
                    page_token.as_deref(),
                )
                .await
                .map_err(SyncError::from)?;
            self.account_is_current(account)?;
            if page.protocol_version != PROTOCOL_VERSION
                || page.cloud_workspace_id != binding.cloud_workspace_id
                || page.at_cursor < 0
                || page.current_cursor < page.at_cursor
            {
                return Err(SyncError::InvalidData);
            }
            if fixed_cursor.is_some_and(|cursor| cursor != page.at_cursor) {
                return Err(SyncError::InvalidData);
            }
            fixed_cursor = Some(page.at_cursor);
            items.extend(
                page.items
                    .into_iter()
                    .filter(|item| item.entity_type == SyncEntityType::Connection),
            );
            page_token = page.next_page_token;
            if page_token.is_none() {
                return Ok(items);
            }
        }
        Err(SyncError::InvalidData)
    }

    async fn bootstrap_connection_v4(
        &self,
        account: &SyncAccountContext,
        binding: &SyncBinding,
        local_snapshots: Vec<DomainSnapshot>,
        remote_items: &[SnapshotItem],
    ) -> Result<(), SyncError> {
        let mut tx = self.repository.pool().begin().await?;
        let Some(plan) = SyncRepository::prepare_connection_v4_on(
            &mut tx,
            binding,
            local_snapshots,
            remote_items,
            self.dependencies.ids.as_ref(),
            self.dependencies.clock.as_ref(),
        )
        .await?
        else {
            tx.commit().await?;
            return Ok(());
        };

        let pages = plan
            .apply_items
            .iter()
            .map(|item| parse_snapshot_item(&binding.local_workspace_id, item))
            .collect::<Result<Vec<_>, _>>()?;
        let cleanup = self
            .apply_external_page_on(
                &mut tx,
                "pro.sync.connection_v4_bootstrap",
                merge_external_pages(pages),
            )
            .await?;
        SyncRepository::complete_connection_v4_on(
            &mut tx,
            binding,
            &plan,
            self.dependencies.clock.as_ref(),
        )
        .await?;
        self.account_is_current(account)?;
        tx.commit().await?;
        self.finish_external_cleanup(cleanup).await;
        Ok(())
    }

    async fn apply_external_page_on(
        &self,
        connection: &mut SqliteConnection,
        action: &'static str,
        page: ExternalApplyPage,
    ) -> Result<ExternalApplyCleanup, SyncError> {
        if external_page_is_empty(&page) {
            return Ok(ExternalApplyCleanup::default());
        }
        let context = CommandContext::new(action, MutationOrigin::External);
        let workspace_deletes: Vec<(String, String)> = page
            .workspaces
            .iter()
            .filter_map(|change| match change {
                ExternalWorkspaceApply::Delete(delete) => Some((
                    delete.entity.workspace_id.clone(),
                    delete.deleted_at.clone(),
                )),
                _ => None,
            })
            .collect();
        let mut cleanup = ExternalApplyCleanup::default();
        for (workspace_id, deleted_at) in &workspace_deletes {
            self.api_client
                .delete_workspace_api_entities_on(
                    connection,
                    &context,
                    workspace_id,
                    Some(deleted_at),
                )
                .await
                .map_err(|_| SyncError::Core)?;
            self.ssh
                .delete_workspace_ssh_task_entities_on(
                    connection,
                    &context,
                    workspace_id,
                    Some(deleted_at),
                )
                .await
                .map_err(|_| SyncError::Core)?;
            let (_, cleanups) = self
                .ssh
                .delete_workspace_connections_on(connection, &context, workspace_id, deleted_at)
                .await
                .map_err(|_| SyncError::Core)?;
            cleanup.ssh_connections.extend(cleanups);
            let (_, cleanups) = self
                .database
                .delete_workspace_connections_on(connection, &context, workspace_id, deleted_at)
                .await
                .map_err(|_| SyncError::Core)?;
            cleanup.database_connections.extend(cleanups);
        }
        self.workspace
            .apply_external_page_on(connection, &context, page.clone())
            .await
            .map_err(|_| SyncError::Core)?;
        for change in page.connections.clone() {
            let connection_type = match &change {
                ExternalConnectionApply::Upsert(record) => record.connection_type.as_str(),
                ExternalConnectionApply::Delete(delete) => {
                    validate_external_connection_delete(delete).map_err(|_| SyncError::Core)?;
                    let row: Option<(String, String)> = sqlx::query_as(
                        "SELECT workspace_id, connection_type FROM connections WHERE id = ?1",
                    )
                    .bind(&delete.entity.entity_id)
                    .fetch_optional(&mut *connection)
                    .await?;
                    let Some((workspace_id, connection_type)) = row else {
                        continue;
                    };
                    if workspace_id != delete.entity.workspace_id {
                        return Err(SyncError::InvalidData);
                    }
                    if connection_type == SSH_CONNECTION_TYPE {
                        SSH_CONNECTION_TYPE
                    } else if connection_type == DATABASE_CONNECTION_TYPE {
                        DATABASE_CONNECTION_TYPE
                    } else {
                        return Err(SyncError::InvalidData);
                    }
                }
            };
            match connection_type {
                SSH_CONNECTION_TYPE => {
                    let outcome = self
                        .ssh
                        .apply_external_connection_on(connection, &context, change)
                        .await
                        .map_err(|_| SyncError::Core)?;
                    if let Some(value) = outcome.value {
                        cleanup.ssh_connections.push(value);
                    }
                }
                DATABASE_CONNECTION_TYPE => {
                    let outcome = self
                        .database
                        .apply_external_connection_on(connection, &context, change)
                        .await
                        .map_err(|_| SyncError::Core)?;
                    if let Some(value) = outcome.value {
                        cleanup.database_connections.push(value);
                    }
                }
                _ => return Err(SyncError::InvalidData),
            }
        }
        self.api_client
            .apply_external_page_on(connection, &context, page.clone())
            .await
            .map_err(|_| SyncError::Core)?;
        self.ssh
            .apply_external_task_page_on(connection, &context, page)
            .await
            .map_err(|_| SyncError::Core)?;
        Ok(cleanup)
    }

    async fn finish_external_cleanup(&self, cleanup: ExternalApplyCleanup) {
        self.ssh
            .cleanup_connection_changes(cleanup.ssh_connections)
            .await;
        self.database
            .cleanup_connection_changes(cleanup.database_connections)
            .await;
    }

    /// Rust is the sole periodic scheduler. Mutation hints are coalesced before
    /// reaching the per-workspace running+dirty singleflight gate.
    pub async fn run_background(&self, mut receiver: mpsc::UnboundedReceiver<String>) {
        let mut interval = tokio::time::interval(PERIODIC_PULL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                workspace = receiver.recv() => {
                    let Some(workspace) = workspace else { break };
                    tokio::time::sleep(LOCAL_DEBOUNCE).await;
                    let mut workspaces = HashSet::from([workspace]);
                    while let Ok(workspace) = receiver.try_recv() { workspaces.insert(workspace); }
                    for workspace in workspaces {
                        let service = self.clone();
                        tokio::spawn(async move { let _ = service.sync_workspace(&workspace).await; });
                    }
                }
                _ = interval.tick() => { let _ = self.sync_all().await; }
            }
        }
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

    fn flight_key(account_id: &str, workspace_id: &str) -> String {
        format!("{account_id}\0{workspace_id}")
    }

    fn is_running(&self, key: &str) -> bool {
        self.flights
            .lock()
            .expect("sync flight lock poisoned")
            .get(key)
            .is_some_and(|state| state.running)
    }

    fn enter_flight(&self, key: &str) -> Option<FlightGuard> {
        let mut flights = self.flights.lock().expect("sync flight lock poisoned");
        let state = flights.entry(key.to_string()).or_default();
        if state.running {
            state.dirty = true;
            None
        } else {
            state.running = true;
            Some(FlightGuard {
                flights: self.flights.clone(),
                key: key.to_string(),
            })
        }
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
        self.repository
            .activate_account(
                &account.account_id,
                account.generation,
                self.dependencies.clock.now(),
            )
            .await
    }

    pub async fn deactivate_account_context(&self) -> Result<(), SyncError> {
        self.repository
            .deactivate_active_account(self.dependencies.clock.now())
            .await
    }

    pub async fn list_cloud_workspaces(&self) -> Result<Vec<CloudWorkspace>, SyncError> {
        let account = self.account().await?;
        let mut workspaces = self
            .transport
            .list_workspaces()
            .await
            .map_err(SyncError::from)?;
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
                Err(TransportError::Unauthorized) => return Err(SyncError::Unauthorized),
                Err(TransportError::ProtocolIncompatible) => {
                    return Err(SyncError::ProtocolIncompatible)
                }
                Err(_) => continue,
            };
            if page.protocol_version != PROTOCOL_VERSION
                || page.cloud_workspace_id != workspace.cloud_workspace_id
                || page.at_cursor != workspace.current_cursor
            {
                continue;
            }
            if let Some(item) = page.items.iter().find(|item| {
                item.entity_type == SyncEntityType::Workspace
                    && item.entity_id == workspace.root_entity_id
            }) {
                workspace.name = snapshot_workspace_name(&workspace.root_entity_id, item)?;
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
        Ok(())
    }

    pub async fn enable(&self, workspace_id: &str) -> Result<(), SyncError> {
        let account = self.account().await?;
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
        let cloud = self
            .transport
            .create_workspace(workspace_id)
            .await
            .map_err(SyncError::from)?;
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
            .await
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

    pub async fn conflicts(&self, workspace_id: &str) -> Result<Vec<SyncConflictView>, SyncError> {
        let account = self.account().await?;
        let conflicts = self
            .repository
            .conflicts(&account.account_id, workspace_id)
            .await?;
        let mut views = Vec::with_capacity(conflicts.len());
        for conflict in conflicts {
            let entity_type = SyncEntityType::parse(&conflict.entity_type)?;
            let mut key =
                DomainEntityKey::new(entity_type.into(), workspace_id, &conflict.entity_id);
            key.parent_entity_id
                .clone_from(&conflict.conflict_parent_entity_id);
            let local_payload = match self.core().await?.read_domain_snapshot(&key).await {
                Ok(snapshot) => canonical_payload(snapshot)?,
                Err(_) => None,
            };
            let remote_payload = conflict
                .conflict_remote_payload_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|_| SyncError::InvalidData)?;
            let local_secret_present = self
                .repository
                .local_secret_present(workspace_id, entity_type, &conflict.entity_id)
                .await?;
            views.push(SyncConflictView {
                cloud_workspace_id: conflict.cloud_workspace_id,
                entity_type: conflict.entity_type,
                entity_id: conflict.entity_id,
                server_version: conflict.server_version,
                operation: conflict
                    .conflict_remote_operation
                    .ok_or(SyncError::InvalidData)?,
                local_payload,
                remote_payload,
                local_secret_present,
            });
        }
        Ok(views)
    }

    pub async fn sync_all(&self) -> Result<(), SyncError> {
        let account = self.account().await?;
        if !self
            .repository
            .global_sync_enabled(&account.account_id)
            .await?
        {
            return Ok(());
        }
        let bindings = self
            .repository
            .enabled_bindings(&account.account_id)
            .await?;
        let mut tasks = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let service = self.clone();
            let account = account.clone();
            tasks.push(tokio::spawn(async move {
                service
                    .sync_workspace_for(account, &binding.local_workspace_id)
                    .await
            }));
        }
        let mut first_error = None;
        for task in tasks {
            match task.await {
                Ok(Err(error)) => {
                    first_error.get_or_insert(error);
                }
                Err(_) => {
                    first_error.get_or_insert(SyncError::Transport);
                }
                _ => {}
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// One-shot per (process, account): dead letters parked because an older
    /// build's protocol was rejected become retryable after an upgrade.
    async fn revive_protocol_dead_letters_once(&self, account_id: &str) -> Result<(), SyncError> {
        let first_time = self
            .protocol_revival_done
            .lock()
            .expect("protocol revival lock poisoned")
            .insert(account_id.to_string());
        if !first_time {
            return Ok(());
        }
        self.repository
            .revive_protocol_dead_letters(account_id, self.dependencies.clock.now())
            .await
            .map(|_| ())
    }

    pub async fn sync_workspace(&self, workspace_id: &str) -> Result<(), SyncError> {
        let account = self.account().await?;
        if !self
            .repository
            .global_sync_enabled(&account.account_id)
            .await?
        {
            return Ok(());
        }
        self.sync_workspace_for(account, workspace_id).await
    }

    async fn sync_workspace_for(
        &self,
        account: SyncAccountContext,
        workspace_id: &str,
    ) -> Result<(), SyncError> {
        self.revive_protocol_dead_letters_once(&account.account_id)
            .await?;
        let key = Self::flight_key(&account.account_id, workspace_id);
        let Some(mut flight) = self.enter_flight(&key) else {
            return Ok(());
        };
        let _permit = self
            .global_limit
            .acquire()
            .await
            .map_err(|_| SyncError::Transport)?;
        let original_account_id = account.account_id.clone();
        let original_generation = account.generation;
        let mut worker_result = Ok(());
        let mut current_account = account;
        'rounds: loop {
            let round_result = self
                .sync_workspace_inner(&current_account, workspace_id)
                .await;
            let mut terminal_error = round_result.err();
            if worker_result.is_ok() {
                if let Some(error) = terminal_error {
                    worker_result = Err(error);
                }
            }

            loop {
                if flight.take_dirty_or_finish() {
                    match self.account().await {
                        Ok(refreshed)
                            if refreshed.account_id == original_account_id
                                && refreshed.generation == original_generation =>
                        {
                            current_account = refreshed;
                            continue 'rounds;
                        }
                        Ok(_) => {
                            terminal_error = Some(SyncError::AccountChanged);
                        }
                        Err(error) => {
                            terminal_error = Some(error);
                        }
                    }
                    if worker_result.is_ok() {
                        worker_result = Err(terminal_error.expect("refresh error is set"));
                    }
                }

                if let Some(error) = terminal_error.take() {
                    if !matches!(error, SyncError::AccountChanged | SyncError::Unauthorized) {
                        let error_code = match self
                            .repository
                            .status(&current_account.account_id, workspace_id, false)
                            .await
                        {
                            Ok(status) if status.dead_count > 0 => {
                                SyncError::DeadLetterBlocked.code()
                            }
                            _ => error.code(),
                        };
                        let _ = self
                            .repository
                            .record_error(
                                &current_account.account_id,
                                workspace_id,
                                current_account.generation,
                                error_code,
                                self.dependencies.clock.now(),
                            )
                            .await;
                    }
                    continue;
                }

                break 'rounds;
            }
        }
        worker_result
    }

    async fn sync_workspace_inner(
        &self,
        account: &SyncAccountContext,
        workspace_id: &str,
    ) -> Result<(), SyncError> {
        self.account_is_current(account)?;
        self.repository
            .claim_generation(
                &account.account_id,
                workspace_id,
                account.generation,
                self.dependencies.clock.now(),
            )
            .await?;
        self.repository
            .recover_expired_leases(&account.account_id, self.dependencies.clock.now())
            .await?;
        let mut binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        if !binding.sync_enabled || binding.state == "paused" {
            return Ok(());
        }

        self.repository
            .bootstrap_ssh_task_v3(
                &binding,
                &self.ssh,
                self.dependencies.ids.as_ref(),
                self.dependencies.clock.as_ref(),
            )
            .await?;
        binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        if binding.connection_v4_bootstrap_state != "completed" {
            let snapshots = self.connection_snapshots(workspace_id).await?;
            // Only a binding still in its original Upload Local phase for an
            // empty cloud workspace may skip the snapshot. Initial counters
            // are not a protocol-provenance bit: a v3 binding can still be
            // uploading while its old cursor has already crossed remote
            // Connection history that v4 must recover.
            let is_empty_cloud_initial_upload = binding.state == "uploading"
                && binding.initial_cursor == Some(0)
                && binding.last_pulled_cursor == 0;
            let remote_items = if is_empty_cloud_initial_upload {
                Vec::new()
            } else {
                self.connection_snapshot_items(account, &binding).await?
            };
            self.bootstrap_connection_v4(account, &binding, snapshots, &remote_items)
                .await?;
        }
        binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;

        // Even a conflicted binding keeps pulling so a 409 delete conflict can
        // be hydrated with the protocol tombstone's deletedAt.
        self.pull(account, &binding).await?;
        binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        if binding.state == "conflict" {
            return Err(SyncError::Conflict);
        }
        let before_push = self
            .repository
            .status(&account.account_id, workspace_id, true)
            .await?;
        if before_push.dead_count > 0 {
            let due = self
                .repository
                .due_outbox(
                    &account.account_id,
                    &binding.cloud_workspace_id,
                    self.dependencies.clock.now(),
                    1,
                )
                .await?;
            if due.is_empty() {
                // Dead parents still block descendants through due_outbox.
                // Unrelated pending survivors remain due and must be pushed;
                // only an empty due queue means the workspace is stuck.
                self.repository
                    .set_binding_state(
                        &binding,
                        "error",
                        Some(SyncError::DeadLetterBlocked.code()),
                        self.dependencies.clock.now(),
                    )
                    .await?;
                return Err(SyncError::DeadLetterBlocked);
            }
        }
        let mut pushed = false;
        while self.push_one_batch(account, &binding).await? {
            pushed = true;
        }
        binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        let before_final_pull = self
            .repository
            .status(&account.account_id, workspace_id, true)
            .await?;
        if before_final_pull.dead_count > 0 {
            self.repository
                .set_binding_state(
                    &binding,
                    "error",
                    Some(SyncError::DeadLetterBlocked.code()),
                    self.dependencies.clock.now(),
                )
                .await?;
            return Err(SyncError::DeadLetterBlocked);
        }
        if binding.initial_confirmed >= binding.initial_total
            && before_final_pull.conflict_count == 0
            && !matches!(binding.state.as_str(), "active" | "paused" | "conflict")
        {
            self.repository
                .set_binding_state(&binding, "reconciling", None, self.dependencies.clock.now())
                .await?;
        }
        // The opening pull already advanced to the server head observed for
        // this round. Pull again only after a push, when the global cursor may
        // also contain interleaved changes from another device.
        if pushed {
            self.pull(account, &binding).await?;
        }
        let status = self
            .repository
            .status(&account.account_id, workspace_id, true)
            .await?;
        let binding = status.binding.ok_or(SyncError::NotFound)?;
        if status.dead_count > 0 {
            self.repository
                .set_binding_state(
                    &binding,
                    "error",
                    Some(SyncError::DeadLetterBlocked.code()),
                    self.dependencies.clock.now(),
                )
                .await?;
            return Err(SyncError::DeadLetterBlocked);
        }
        if status.conflict_count > 0 {
            return Err(SyncError::Conflict);
        }
        let can_activate = binding.initial_confirmed >= binding.initial_total
            && status.pending_count == 0
            && status.uncertain_count == 0
            && status.in_flight_count == 0
            && status.dead_count == 0
            && status.conflict_count == 0
            && !matches!(binding.state.as_str(), "error" | "paused" | "conflict");
        if can_activate {
            self.repository
                .set_binding_state(&binding, "active", None, self.dependencies.clock.now())
                .await?;
        }
        Ok(())
    }

    async fn push_one_batch(
        &self,
        account: &SyncAccountContext,
        binding: &SyncBinding,
    ) -> Result<bool, SyncError> {
        let candidates = self
            .repository
            .due_outbox(
                &account.account_id,
                &binding.cloud_workspace_id,
                self.dependencies.clock.now(),
                PUSH_BATCH_LIMIT,
            )
            .await?;
        if candidates.is_empty() {
            return Ok(false);
        }
        let mut entries = Vec::new();
        let mut operations = Vec::new();
        let mut bytes = 0;
        let mut parked_oversized = false;
        for mut entry in candidates {
            let operation = SyncOperation::parse(&entry.operation)?;
            let needs_snapshot = (operation == SyncOperation::Upsert
                && entry.canonical_payload_json.is_none())
                || (operation == SyncOperation::Delete && entry.deleted_at.is_none());
            if needs_snapshot {
                let entity_type = SyncEntityType::parse(&entry.entity_type)?;
                let mut key = DomainEntityKey::new(
                    DomainEntityType::from(entity_type),
                    &entry.local_workspace_id,
                    &entry.entity_id,
                );
                key.parent_entity_id.clone_from(&entry.parent_entity_id);
                let snapshot = self
                    .core()
                    .await?
                    .read_domain_snapshot(&key)
                    .await
                    .map_err(|_| SyncError::Core)?;
                let Some(materialized) = self
                    .repository
                    .materialize_outbox_entry(&entry, snapshot, self.dependencies.clock.now())
                    .await?
                else {
                    continue;
                };
                entry = materialized;
            }
            let operation = build_push_operation(&entry)?;
            let operation_bytes = serde_json::to_vec(&operation)
                .map_err(|_| SyncError::InvalidData)?
                .len();
            if operation_bytes > PUSH_BATCH_MAX_BYTES {
                // A single operation that can never fit in one push request
                // previously failed the whole round with an opaque error and
                // retried forever. Park it as a standard dead letter instead:
                // it becomes visible in the dead-letter UI and is repairable
                // (retry after shrinking the entity, use-remote, or delete).
                self.repository
                    .mark_not_sent(
                        std::slice::from_ref(&entry),
                        "payload_too_large",
                        false,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                parked_oversized = true;
                continue;
            }
            if !operations.is_empty() && bytes + operation_bytes > PUSH_BATCH_MAX_BYTES {
                break;
            }
            bytes += operation_bytes;
            entries.push(entry);
            operations.push(operation);
        }
        if entries.is_empty() {
            return Ok(parked_oversized);
        }
        self.repository
            .mark_in_flight(&entries, &self.worker_id, self.dependencies.clock.now())
            .await?;
        let request = PushRequest {
            protocol_version: PROTOCOL_VERSION,
            operations,
        };
        let response = self
            .transport
            .push(&binding.cloud_workspace_id, &request)
            .await;
        self.account_is_current(account)?;
        match response {
            Ok(response) => {
                if response.protocol_version != PROTOCOL_VERSION || response.current_cursor < 0 {
                    self.repository
                        .mark_uncertain(&entries, self.dependencies.clock.now())
                        .await?;
                    return Err(SyncError::InvalidData);
                }
                self.repository
                    .apply_push_results(
                        binding,
                        &entries,
                        &response.results,
                        self.dependencies.clock.as_ref(),
                    )
                    .await?;
                Ok(true)
            }
            Err(TransportError::Conflict(details)) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        "base_version_conflict",
                        true,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                self.repository
                    .record_push_conflict(binding, &details, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::Conflict)
            }
            Err(TransportError::Unauthorized) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        "unauthorized",
                        false,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                Err(SyncError::Unauthorized)
            }
            Err(TransportError::ProtocolIncompatible) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        "protocol_version_unsupported",
                        false,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                Err(SyncError::ProtocolIncompatible)
            }
            Err(TransportError::PermanentOperation { code, operation_id }) => {
                let now = self.dependencies.clock.now();
                if !self
                    .repository
                    .mark_batch_permanent_failure(&entries, &operation_id, &code, now)
                    .await?
                {
                    // A stale, malformed, or otherwise untrusted operation
                    // reference cannot safely identify a row in this batch.
                    // Preserve the old all-entries fallback instead of
                    // guessing that any operation committed.
                    self.repository
                        .mark_not_sent(&entries, &code, false, now)
                        .await?;
                }
                Err(SyncError::Permanent)
            }
            Err(TransportError::Permanent(code)) => {
                self.repository
                    .mark_not_sent(&entries, &code, false, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::Permanent)
            }
            Err(TransportError::NotFound) => {
                self.repository
                    .mark_not_sent(&entries, "not_found", false, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::NotFound)
            }
            Err(TransportError::InvalidResponse | TransportError::ResultUnknown) => {
                self.repository
                    .mark_uncertain(&entries, self.dependencies.clock.now())
                    .await?;
                Err(SyncError::Transport)
            }
            Err(TransportError::Retryable) => {
                self.repository
                    .mark_not_sent(
                        &entries,
                        "retryable_transport",
                        true,
                        self.dependencies.clock.now(),
                    )
                    .await?;
                Err(SyncError::Transport)
            }
        }
    }

    async fn pull(
        &self,
        account: &SyncAccountContext,
        binding: &SyncBinding,
    ) -> Result<(), SyncError> {
        let mut cursor = binding.last_pulled_cursor;
        let mut pinned_current = None;
        for _ in 0..MAX_REMOTE_PAGES {
            let page = self
                .transport
                .changes(&binding.cloud_workspace_id, cursor, PULL_PAGE_LIMIT)
                .await
                .map_err(SyncError::from)?;
            self.account_is_current(account)?;
            validate_changes_page(binding, cursor, pinned_current, &page)?;
            pinned_current = Some(page.current_cursor);
            let now = self.dependencies.clock.now().to_rfc3339();
            let mut tx = self.repository.pool().begin().await?;
            let mut safe_pages = Vec::new();
            let mut safe_changes = Vec::new();
            let ssh_task_aggregate_deletes = page
                .changes
                .iter()
                .filter(|change| {
                    change.entity_type == SyncEntityType::SshTask
                        && change.operation == SyncOperation::Delete
                })
                .map(|change| (change.operation_id.as_str(), change.entity_id.as_str()))
                .collect::<HashSet<_>>();
            for change in &page.changes {
                let aggregate_delete_root = (change.entity_type == SyncEntityType::SshTaskStep
                    && change.operation == SyncOperation::Delete)
                    .then(|| change.parent_entity_id.as_deref())
                    .flatten()
                    .filter(|task_id| {
                        ssh_task_aggregate_deletes
                            .contains(&(change.operation_id.as_str(), *task_id))
                    })
                    .map(|task_id| (SyncEntityType::SshTask, task_id));
                if SyncRepository::prepare_remote_change_with_aggregate_root_on(
                    &mut tx,
                    binding,
                    change,
                    aggregate_delete_root,
                    &now,
                )
                .await?
                {
                    safe_pages.push(parse_remote_change(&binding.local_workspace_id, change)?);
                    safe_changes.push(change);
                }
            }
            let external = merge_external_pages(safe_pages);
            let cleanup = self
                .apply_external_page_on(&mut tx, "pro.sync.pull", external)
                .await?;
            for change in safe_changes {
                SyncRepository::record_applied_remote_on(&mut tx, binding, change, &now).await?;
            }
            SyncRepository::advance_cursor_on(&mut tx, binding, page.next_cursor, &now).await?;
            tx.commit().await?;
            self.finish_external_cleanup(cleanup).await;
            cursor = page.next_cursor;
            if page.next_cursor == page.current_cursor {
                return Ok(());
            }
        }
        Err(SyncError::InvalidData)
    }

    pub async fn download_workspace(
        &self,
        cloud_workspace_id: &str,
        decision: DownloadDecision,
    ) -> Result<String, SyncError> {
        if decision != DownloadDecision::DownloadToNewWorkspace {
            return Err(SyncError::SafeReplaceUnavailable);
        }
        let account = self.account().await?;
        let cloud = self
            .transport
            .list_workspaces()
            .await
            .map_err(SyncError::from)?
            .into_iter()
            .find(|workspace| workspace.cloud_workspace_id == cloud_workspace_id)
            .ok_or(SyncError::NotFound)?;
        self.account_is_current(&account)?;
        if self
            .repository
            .binding_by_cloud(&account.account_id, cloud_workspace_id)
            .await?
            .is_some()
        {
            return Err(SyncError::InvalidData);
        }

        let stage_id = self.dependencies.ids.next_id();
        let result = self
            .download_and_apply_staged(&account, &cloud, &stage_id)
            .await;
        let _ = self.repository.clear_snapshot_stage(&stage_id).await;
        result
    }

    async fn download_and_apply_staged(
        &self,
        account: &SyncAccountContext,
        cloud: &CloudWorkspace,
        stage_id: &str,
    ) -> Result<String, SyncError> {
        let mut fixed_cursor = None;
        let mut page_token = None;
        let mut root_workspace_name = None;
        for _ in 0..MAX_REMOTE_PAGES {
            let page = self
                .transport
                .snapshot(
                    &cloud.cloud_workspace_id,
                    fixed_cursor,
                    page_token.as_deref(),
                )
                .await
                .map_err(SyncError::from)?;
            self.account_is_current(account)?;
            if page.protocol_version != PROTOCOL_VERSION
                || page.cloud_workspace_id != cloud.cloud_workspace_id
                || page.at_cursor < 0
                || page.current_cursor < page.at_cursor
            {
                return Err(SyncError::InvalidData);
            }
            if fixed_cursor.is_some_and(|cursor| cursor != page.at_cursor) {
                return Err(SyncError::InvalidData);
            }
            fixed_cursor = Some(page.at_cursor);
            for item in &page.items {
                if let Some(name) = snapshot_workspace_name(&cloud.root_entity_id, item)? {
                    if root_workspace_name.replace(name).is_some() {
                        return Err(SyncError::InvalidData);
                    }
                }
            }
            self.repository
                .stage_snapshot_page(
                    stage_id,
                    &account.account_id,
                    &cloud.cloud_workspace_id,
                    page.at_cursor,
                    &page.items,
                    &self.dependencies.clock.now().to_rfc3339(),
                )
                .await?;
            page_token = page.next_page_token;
            if page_token.is_none() {
                break;
            }
        }
        let at_cursor = fixed_cursor.ok_or(SyncError::InvalidData)?;
        let root_workspace_name = root_workspace_name.ok_or(SyncError::InvalidData)?;
        let now = self.dependencies.clock.now().to_rfc3339();
        let mut tx = self.repository.pool().begin().await?;
        if SyncRepository::local_workspace_exists_on(&mut tx, &cloud.root_entity_id).await? {
            return Err(SyncError::LocalWorkspaceNotEmpty);
        }
        if SyncRepository::active_workspace_name_exists_on(&mut tx, &root_workspace_name).await? {
            return Err(SyncError::WorkspaceNameConflict);
        }
        let rows: Vec<(String, String, Option<String>, i64, i64, String)> = sqlx::query_as(
            r#"SELECT entity_type, entity_id, parent_entity_id, server_version,
                      payload_schema_version, payload_json
               FROM cloud_sync_snapshot_staging WHERE stage_id = ?1
               ORDER BY topology_rank, entity_type, entity_id"#,
        )
        .bind(stage_id)
        .fetch_all(&mut *tx)
        .await?;
        if rows.is_empty() {
            return Err(SyncError::InvalidData);
        }
        let mut pages = Vec::with_capacity(rows.len());
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let item = SnapshotItem {
                entity_type: SyncEntityType::parse(&row.0)?,
                entity_id: row.1,
                parent_entity_id: row.2,
                server_version: row.3,
                payload_schema_version: row.4,
                payload: serde_json::from_str(&row.5).map_err(|_| SyncError::InvalidData)?,
            };
            pages.push(parse_snapshot_item(&cloud.root_entity_id, &item)?);
            items.push(item);
        }
        let cleanup = self
            .apply_external_page_on(&mut tx, "pro.sync.snapshot", merge_external_pages(pages))
            .await?;
        SyncRepository::insert_download_binding_on(
            &mut tx,
            &account.account_id,
            account.generation,
            &cloud.root_entity_id,
            &cloud.cloud_workspace_id,
            at_cursor,
            &now,
        )
        .await?;
        for item in &items {
            SyncRepository::record_snapshot_state_on(
                &mut tx,
                &account.account_id,
                &cloud.cloud_workspace_id,
                item,
                &now,
            )
            .await?;
        }
        tx.commit().await?;
        self.finish_external_cleanup(cleanup).await;
        let binding = self
            .repository
            .binding(&account.account_id, &cloud.root_entity_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        self.repository
            .set_binding_state(&binding, "active", None, self.dependencies.clock.now())
            .await?;
        Ok(cloud.root_entity_id.clone())
    }

    pub async fn keep_local(
        &self,
        workspace_id: &str,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<(), SyncError> {
        let account = self.account().await?;
        let binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        let conflict = self
            .repository
            .conflict(
                &account.account_id,
                &binding.cloud_workspace_id,
                entity_type,
                entity_id,
            )
            .await?;
        if matches!(
            entity_type,
            SyncEntityType::Connection
                | SyncEntityType::ApiCollection
                | SyncEntityType::ApiFolder
                | SyncEntityType::ApiRequest
        ) {
            let scoped = self
                .repository
                .scoped_conflicts(&binding, &conflict)
                .await?;
            let mut snapshots = Vec::with_capacity(scoped.len());
            for item in &scoped {
                let scoped_type = SyncEntityType::parse(&item.entity_type)?;
                let mut key =
                    DomainEntityKey::new(scoped_type.into(), workspace_id, &item.entity_id);
                key.parent_entity_id
                    .clone_from(&item.conflict_parent_entity_id);
                snapshots.push(
                    self.core()
                        .await?
                        .read_domain_snapshot(&key)
                        .await
                        .map_err(|_| SyncError::Core)?,
                );
            }
            self.repository
                .keep_local_snapshots(
                    &binding,
                    &conflict,
                    snapshots,
                    self.dependencies.ids.as_ref(),
                    self.dependencies.clock.as_ref(),
                )
                .await?;
        } else {
            self.repository
                .keep_local(
                    &binding,
                    &conflict,
                    self.dependencies.ids.as_ref(),
                    self.dependencies.clock.as_ref(),
                )
                .await?;
        }
        self.sync_workspace_for(account, workspace_id).await
    }

    pub async fn use_remote(
        &self,
        workspace_id: &str,
        entity_type: SyncEntityType,
        entity_id: &str,
    ) -> Result<(), SyncError> {
        let account = self.account().await?;
        let binding = self
            .repository
            .binding(&account.account_id, workspace_id)
            .await?
            .ok_or(SyncError::NotFound)?;
        let conflict = self
            .repository
            .conflict(
                &account.account_id,
                &binding.cloud_workspace_id,
                entity_type,
                entity_id,
            )
            .await?;
        let operation = SyncOperation::parse(
            conflict
                .conflict_remote_operation
                .as_deref()
                .ok_or(SyncError::InvalidData)?,
        )?;
        let change = RemoteChange {
            cursor: binding.last_pulled_cursor,
            operation_id: conflict
                .conflict_operation_id
                .clone()
                .unwrap_or_else(|| "conflict-resolution".into()),
            entity_type,
            entity_id: entity_id.to_string(),
            parent_entity_id: conflict.conflict_parent_entity_id.clone(),
            operation,
            server_version: conflict.server_version,
            payload_schema_version: crate::PAYLOAD_SCHEMA_VERSION,
            payload: conflict
                .conflict_remote_payload_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|_| SyncError::InvalidData)?,
            deleted_at: conflict.conflict_deleted_at.clone(),
        };
        let external = parse_remote_change(workspace_id, &change)?;
        let now = self.dependencies.clock.now().to_rfc3339();
        let mut tx = self.repository.pool().begin().await?;
        SyncRepository::assert_binding_generation_on(&mut tx, &binding).await?;
        let scoped_conflicts = conflict_scope::conflicts_on(&mut tx, &binding, &conflict).await?;
        conflict_scope::abandon_intents_on(&mut tx, &binding, &conflict).await?;
        let cleanup = self
            .apply_external_page_on(&mut tx, "pro.sync.conflict.use_remote", external)
            .await?;
        for scoped_conflict in &scoped_conflicts {
            SyncRepository::clear_conflict_on(&mut tx, &binding, scoped_conflict, false, &now)
                .await?;
        }
        tx.commit().await?;
        self.finish_external_cleanup(cleanup).await;
        Ok(())
    }
}

fn build_push_operation(entry: &OutboxEntry) -> Result<PushOperation, SyncError> {
    let operation = SyncOperation::parse(&entry.operation)?;
    let payload = entry
        .canonical_payload_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| SyncError::InvalidData)?;
    if (operation == SyncOperation::Upsert) != payload.is_some()
        || (operation == SyncOperation::Delete) != entry.deleted_at.is_some()
    {
        return Err(SyncError::InvalidData);
    }
    Ok(PushOperation {
        operation_id: entry.operation_id.clone(),
        entity_type: SyncEntityType::parse(&entry.entity_type)?,
        entity_id: entry.entity_id.clone(),
        parent_entity_id: entry.parent_entity_id.clone(),
        operation,
        base_version: entry.base_version,
        payload_schema_version: entry.payload_schema_version,
        payload,
    })
}

fn validate_changes_page(
    binding: &SyncBinding,
    requested_cursor: i64,
    pinned_current: Option<i64>,
    page: &ChangesPage,
) -> Result<(), SyncError> {
    let cursor_sequence_is_complete = if page.changes.is_empty() {
        page.next_cursor == requested_cursor && page.current_cursor == requested_cursor
    } else {
        page.changes.first().map(|change| change.cursor) == requested_cursor.checked_add(1)
            && page.changes.last().map(|change| change.cursor) == Some(page.next_cursor)
            && page
                .changes
                .windows(2)
                .all(|pair| pair[0].cursor.checked_add(1) == Some(pair[1].cursor))
    };
    let valid = page.protocol_version == PROTOCOL_VERSION
        && page.cloud_workspace_id == binding.cloud_workspace_id
        && page.current_cursor >= requested_cursor
        && page.next_cursor >= requested_cursor
        && page.next_cursor <= page.current_cursor
        && pinned_current.is_none_or(|current| current == page.current_cursor)
        && cursor_sequence_is_complete
        && page
            .changes
            .iter()
            .all(|change| change.cursor > requested_cursor && change.cursor <= page.next_cursor);
    valid.then_some(()).ok_or(SyncError::InvalidData)
}

fn merge_external_pages(pages: Vec<ExternalApplyPage>) -> ExternalApplyPage {
    let mut merged = ExternalApplyPage::default();
    for mut page in pages {
        merged.workspaces.append(&mut page.workspaces);
        merged.connections.append(&mut page.connections);
        merged
            .workspace_variables
            .append(&mut page.workspace_variables);
        merged
            .workspace_environments
            .append(&mut page.workspace_environments);
        merged
            .workspace_environment_variables
            .append(&mut page.workspace_environment_variables);
        merged.api_collections.append(&mut page.api_collections);
        merged.api_folders.append(&mut page.api_folders);
        merged.api_requests.append(&mut page.api_requests);
        merged.ssh_tasks.append(&mut page.ssh_tasks);
        merged.ssh_task_steps.append(&mut page.ssh_task_steps);
    }
    merged
}

fn external_page_is_empty(page: &ExternalApplyPage) -> bool {
    page.workspaces.is_empty()
        && page.connections.is_empty()
        && page.workspace_variables.is_empty()
        && page.workspace_environments.is_empty()
        && page.workspace_environment_variables.is_empty()
        && page.api_collections.is_empty()
        && page.api_folders.is_empty()
        && page.api_requests.is_empty()
        && page.ssh_tasks.is_empty()
        && page.ssh_task_steps.is_empty()
}
