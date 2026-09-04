//! Periodic scheduling, singleflight ownership and the ordered reconciliation round.
//! Keep error finalization and dirty-trigger draining inside the same flight.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

use super::SyncService;
use crate::{SyncAccountContext, SyncError};

const LOCAL_DEBOUNCE: Duration = Duration::from_millis(500);
const PERIODIC_PULL: Duration = Duration::from_secs(5 * 60);
const SCHEDULER_ERROR_COOLDOWN: ChronoDuration = ChronoDuration::seconds(30);

fn retry_delay(next_attempt_at: &str, now: DateTime<Utc>) -> Option<Duration> {
    DateTime::parse_from_rfc3339(next_attempt_at)
        .ok()
        .map(|at| {
            at.with_timezone(&Utc)
                .signed_duration_since(now)
                .to_std()
                .unwrap_or(Duration::ZERO)
        })
}

#[derive(Default)]
pub(super) struct FlightState {
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

impl SyncService {
    /// Rust is the sole periodic scheduler. Mutation hints are coalesced before
    /// reaching the per-workspace running+dirty singleflight gate.
    pub async fn run_background(&self, mut receiver: mpsc::UnboundedReceiver<String>) {
        let mut interval = tokio::time::interval(PERIODIC_PULL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let scheduled = self.repository.next_scheduled_retry().await.ok().flatten();
            // Arm against the live session, not the DB generation captured with
            // the retry row. `next_scheduled_retry` can race an account switch:
            // the row still names the old owner while runtime context already
            // stores the new generation, which would make a DB-generation fence
            // pass and push for the previous account.
            let armed_generation = self.transport.account_generation();
            let scheduled = match scheduled {
                Some((account_id, workspace_id, generation, at))
                    if generation as u64 == armed_generation =>
                {
                    Some((account_id, workspace_id, generation, at))
                }
                Some(_) => {
                    let _ = self.account().await;
                    None
                }
                None => None,
            };
            let retry_delay = scheduled
                .as_ref()
                .and_then(|(_, _, _, at)| retry_delay(at, self.dependencies.clock.now()))
                .unwrap_or(PERIODIC_PULL);
            let retry_timer = tokio::time::sleep(retry_delay);
            tokio::pin!(retry_timer);
            tokio::select! {
                workspace = receiver.recv() => {
                    let Some(workspace) = workspace else { break };
                    tokio::time::sleep(LOCAL_DEBOUNCE).await;
                    let mut workspaces = HashSet::from([workspace]);
                    while let Ok(workspace) = receiver.try_recv() { workspaces.insert(workspace); }
                    for workspace in workspaces {
                        let _ = self.sync_workspace_scheduled(&workspace).await;
                    }
                }
                _ = interval.tick() => { let _ = self.sync_all().await; }
                _ = self.retry_scheduler_wakeup.notified() => {}
                _ = &mut retry_timer => {
                    if self.transport.account_generation() != armed_generation {
                        let _ = self.account().await;
                        continue;
                    }
                    if let Some((account_id, workspace_id, _, _)) = scheduled {
                        if self.sync_workspace_scheduled(&workspace_id).await.is_err() {
                            let now = self.dependencies.clock.now();
                            let _ = self.repository.defer_due_retries(
                                &account_id,
                                &workspace_id,
                                now + SCHEDULER_ERROR_COOLDOWN,
                                now,
                            ).await;
                        }
                    }
                }
            }
        }
    }

    pub(super) fn flight_key(account_id: &str, workspace_id: &str) -> String {
        format!("{account_id}\0{workspace_id}")
    }

    pub(super) fn is_running(&self, key: &str) -> bool {
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
        self.repository
            .prepare_manual_retry(
                &account.account_id,
                workspace_id,
                self.dependencies.clock.now(),
            )
            .await?;
        self.sync_workspace_for(account, workspace_id).await
    }

    async fn sync_workspace_scheduled(&self, workspace_id: &str) -> Result<(), SyncError> {
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

    pub(super) async fn sync_workspace_for(
        &self,
        account: SyncAccountContext,
        workspace_id: &str,
    ) -> Result<(), SyncError> {
        match self
            .repository
            .resolve_cloud_sync_owner(workspace_id)
            .await?
        {
            Some(owner)
                if owner.account_id == account.account_id
                    && self
                        .repository
                        .binding(&account.account_id, workspace_id)
                        .await?
                        .is_some_and(|binding| {
                            binding.cloud_workspace_id == owner.cloud_workspace_id
                        }) => {}
            Some(owner) if owner.account_id != account.account_id => {
                return Err(SyncError::WorkspaceOwnedByAnotherAccount)
            }
            Some(_) => return Err(SyncError::WorkspaceOwnershipInvariant),
            None => return Err(SyncError::NotFound),
        }
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
                    if !matches!(error, SyncError::AccountChanged) {
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
        if binding.state == "error" {
            match binding.last_error.as_deref() {
                Some("cloud_sync_workspace_deleted") => return Err(SyncError::WorkspaceDeleted),
                Some("cloud_sync_not_found") => return Err(SyncError::NotFound),
                Some("cloud_sync_snapshot_required") => return Err(SyncError::SnapshotRequired),
                Some("cloud_sync_permanent_failure") => return Err(SyncError::Permanent),
                _ => {}
            }
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
        self.repository
            .bootstrap_api_v2(
                &binding,
                &self.api_client,
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

        // Versioned bootstrap handles known protocol migrations. The generic
        // repair pass closes the durable-local-intent gap for live entities
        // created by an older client or by a mutation path that escaped the
        // hook. It is local-only, generation-fenced, idempotent, and must run
        // before pull/push can observe an incomplete outbox.
        self.repository
            .reconcile_missing_local_sync_state(
                &binding,
                &self.api_client,
                &self.ssh,
                &self.database,
                self.dependencies.ids.as_ref(),
                self.dependencies.clock.as_ref(),
            )
            .await?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn retry_deadline_uses_one_paused_time_sleep() {
        let now = DateTime::parse_from_rfc3339("2026-09-04T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let delay = retry_delay("2026-09-04T00:00:05Z", now).unwrap();
        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);

        tokio::time::advance(Duration::from_secs(4)).await;
        assert!(!sleep.as_mut().is_elapsed());
        tokio::time::advance(Duration::from_secs(1)).await;
        sleep.await;
    }
}
