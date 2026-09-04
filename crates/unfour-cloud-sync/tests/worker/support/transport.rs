//! Scripted transport responses, failures and deterministic in-flight barriers.

use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Barrier;
use unfour_cloud_sync::{
    ChangesPage, CloudWorkspace, PushRequest, PushResponse, PushResult, PushResultStatus,
    RemoteSyncProblem, RemoteSyncProblemCategory, SnapshotPage, SyncAccountContext, SyncPhase,
    SyncTransport, TransportError, PROTOCOL_VERSION,
};

#[derive(Default)]
pub(crate) struct MockTransport {
    pub(crate) pushes: Mutex<Vec<PushRequest>>,
    pub(crate) changes: Mutex<VecDeque<ChangesPage>>,
    pub(crate) snapshots: Mutex<VecDeque<SnapshotPage>>,
    pub(crate) roots: Mutex<Vec<String>>,
    pub(crate) created_workspace_id: Mutex<Option<String>>,
    pub(crate) create_workspace_calls: AtomicUsize,
    pub(crate) account_id: Mutex<String>,
    pub(crate) generation: AtomicU64,
    pub(crate) fail_pushes: AtomicUsize,
    pub(crate) permanent_pushes: AtomicUsize,
    pub(crate) workspace_deleted_pushes: AtomicUsize,
    pub(crate) permanent_operation: Mutex<Option<(String, String)>>,
    pub(crate) unknown_permanent_operation: Mutex<Option<(String, String)>>,
    pub(crate) unauthorized_pushes: AtomicUsize,
    pub(crate) entitlement_pushes: AtomicUsize,
    pub(crate) fail_on_push_number: AtomicUsize,
    pub(crate) no_op_pushes: AtomicUsize,
    pub(crate) active_calls: AtomicUsize,
    pub(crate) max_active_calls: AtomicUsize,
    pub(crate) cursor: AtomicU64,
    pub(crate) account_calls: AtomicUsize,
    pub(crate) fail_account_on_call: AtomicUsize,
    pub(crate) changes_calls: AtomicUsize,
    pub(crate) snapshot_calls: AtomicUsize,
    pub(crate) push_barrier: Mutex<Option<Arc<Barrier>>>,
    pub(crate) changes_barrier: Mutex<Option<Arc<Barrier>>>,
    pub(crate) snapshot_barrier: Mutex<Option<Arc<Barrier>>>,
}

struct ActiveCall<'a>(&'a MockTransport);
impl Drop for ActiveCall<'_> {
    fn drop(&mut self) {
        self.0.active_calls.fetch_sub(1, Ordering::SeqCst);
    }
}

impl MockTransport {
    pub(crate) fn new() -> Self {
        Self {
            roots: Mutex::new(vec!["workspace-remote".into()]),
            account_id: Mutex::new("account-a".into()),
            ..Self::default()
        }
    }

    async fn enter(&self) -> ActiveCall<'_> {
        let active = self.active_calls.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active_calls.fetch_max(active, Ordering::SeqCst);
        ActiveCall(self)
    }

    pub(crate) fn switch_account(&self, account_id: &str) {
        let mut current = self.account_id.lock().unwrap();
        *current = account_id.into();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.cursor.store(0, Ordering::SeqCst);
    }

    pub(crate) fn fail_operation_once(&self, entity_id: &str, code: &str) {
        *self.permanent_operation.lock().unwrap() = Some((entity_id.to_string(), code.to_string()));
    }

    pub(crate) fn fail_unknown_operation_once(&self, operation_id: &str, code: &str) {
        *self.unknown_permanent_operation.lock().unwrap() =
            Some((operation_id.to_string(), code.to_string()));
    }

    pub(crate) fn terminal_page(&self, after: i64, cloud_workspace_id: &str) -> ChangesPage {
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: cloud_workspace_id.into(),
            current_cursor: after,
            next_cursor: after,
            changes: Vec::new(),
        }
    }
}

#[async_trait]
impl SyncTransport for MockTransport {
    async fn account_context(&self) -> Result<SyncAccountContext, TransportError> {
        let call = self.account_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_account_on_call.load(Ordering::SeqCst) == call {
            return Err(TransportError::Unauthorized);
        }
        let account_id = self.account_id.lock().unwrap();
        Ok(SyncAccountContext {
            account_id: account_id.clone(),
            generation: self.generation.load(Ordering::SeqCst),
        })
    }

    fn account_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    async fn list_workspaces(&self) -> Result<Vec<CloudWorkspace>, TransportError> {
        let _active = self.enter().await;
        Ok(self
            .roots
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, root)| CloudWorkspace {
                cloud_workspace_id: format!("cloud-{}", index + 1),
                root_entity_id: root.clone(),
                name: None,
                current_cursor: self.cursor.load(Ordering::SeqCst) as i64,
                created_at: "2026-07-28T00:00:00Z".into(),
                updated_at: "2026-07-28T00:00:00Z".into(),
            })
            .collect())
    }

    async fn create_workspace(
        &self,
        root_entity_id: &str,
    ) -> Result<CloudWorkspace, TransportError> {
        self.create_workspace_calls.fetch_add(1, Ordering::SeqCst);
        let _active = self.enter().await;
        Ok(CloudWorkspace {
            cloud_workspace_id: self
                .created_workspace_id
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "cloud-created".into()),
            root_entity_id: root_entity_id.into(),
            name: None,
            current_cursor: self.cursor.load(Ordering::SeqCst) as i64,
            created_at: "2026-07-28T00:00:00Z".into(),
            updated_at: "2026-07-28T00:00:00Z".into(),
        })
    }

    async fn push(
        &self,
        _cloud_workspace_id: &str,
        request: &PushRequest,
    ) -> Result<PushResponse, TransportError> {
        let barrier = self.push_barrier.lock().unwrap().take();
        if let Some(barrier) = barrier {
            barrier.wait().await;
            barrier.wait().await;
        }
        let _active = self.enter().await;
        self.pushes.lock().unwrap().push(request.clone());
        let push_number = self.pushes.lock().unwrap().len();
        if self.fail_on_push_number.load(Ordering::SeqCst) == push_number {
            self.fail_on_push_number.store(0, Ordering::SeqCst);
            return Err(TransportError::ResultUnknown);
        }
        if let Some((operation_id, code)) = self.unknown_permanent_operation.lock().unwrap().take()
        {
            return Err(TransportError::PermanentOperation { code, operation_id });
        }
        let operation_failure =
            self.permanent_operation
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|(entity_id, code)| {
                    request
                        .operations
                        .iter()
                        .find(|operation| operation.entity_id == *entity_id)
                        .map(|operation| (operation.operation_id.clone(), code.clone()))
                });
        if let Some((operation_id, code)) = operation_failure {
            self.permanent_operation.lock().unwrap().take();
            return Err(TransportError::PermanentOperation { code, operation_id });
        }
        if self
            .workspace_deleted_pushes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                (value > 0).then(|| value - 1)
            })
            .is_ok()
        {
            return Err(TransportError::Remote(RemoteSyncProblem {
                server_error_code: "sync_workspace_deleted".into(),
                request_id: Some("server-request-deleted".into()),
                http_status: Some(409),
                phase: SyncPhase::Push,
                operation_id: None,
                operation_index: None,
                entity_type: None,
                entity_id: None,
                category: RemoteSyncProblemCategory::Workspace,
            }));
        }
        if self
            .permanent_pushes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                (value > 0).then(|| value - 1)
            })
            .is_ok()
        {
            return Err(TransportError::Remote(RemoteSyncProblem {
                server_error_code: "invalid_sync_entity".into(),
                request_id: Some("server-request-invalid".into()),
                http_status: Some(400),
                phase: SyncPhase::Push,
                operation_id: request
                    .operations
                    .first()
                    .map(|operation| operation.operation_id.clone()),
                operation_index: Some(0),
                entity_type: request
                    .operations
                    .first()
                    .map(|operation| operation.entity_type.as_str().to_string()),
                entity_id: request
                    .operations
                    .first()
                    .map(|operation| operation.entity_id.clone()),
                category: RemoteSyncProblemCategory::OperationPermanent,
            }));
        }
        if self
            .unauthorized_pushes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                (value > 0).then(|| value - 1)
            })
            .is_ok()
        {
            return Err(TransportError::Unauthorized);
        }
        if self
            .entitlement_pushes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                (value > 0).then(|| value - 1)
            })
            .is_ok()
        {
            return Err(TransportError::EntitlementRequired);
        }
        if self
            .fail_pushes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                (value > 0).then(|| value - 1)
            })
            .is_ok()
        {
            self.cursor
                .fetch_add(request.operations.len() as u64, Ordering::SeqCst);
            return Err(TransportError::ResultUnknown);
        }
        let no_op = self
            .no_op_pushes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                (value > 0).then(|| value - 1)
            })
            .is_ok();
        let current = if no_op {
            self.cursor.load(Ordering::SeqCst) as i64
        } else {
            self.cursor
                .fetch_add(request.operations.len() as u64, Ordering::SeqCst) as i64
                + request.operations.len() as i64
        };
        Ok(PushResponse {
            protocol_version: PROTOCOL_VERSION,
            current_cursor: current,
            results: request
                .operations
                .iter()
                .enumerate()
                .map(|(index, operation)| PushResult {
                    operation_id: operation.operation_id.clone(),
                    server_version: operation.base_version + 1,
                    cursor: if no_op {
                        current
                    } else {
                        current - request.operations.len() as i64 + index as i64 + 1
                    },
                    status: if no_op {
                        PushResultStatus::NoOp
                    } else {
                        PushResultStatus::Applied
                    },
                })
                .collect(),
        })
    }

    async fn changes(
        &self,
        cloud_workspace_id: &str,
        after_cursor: i64,
        _limit: usize,
    ) -> Result<ChangesPage, TransportError> {
        self.changes_calls.fetch_add(1, Ordering::SeqCst);
        let barrier = self.changes_barrier.lock().unwrap().take();
        if let Some(barrier) = barrier {
            barrier.wait().await;
            barrier.wait().await;
        }
        let _active = self.enter().await;
        Ok(self
            .changes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| self.terminal_page(after_cursor, cloud_workspace_id)))
    }

    async fn snapshot(
        &self,
        _cloud_workspace_id: &str,
        _at_cursor: Option<i64>,
        _page_token: Option<&str>,
    ) -> Result<SnapshotPage, TransportError> {
        self.snapshot_calls.fetch_add(1, Ordering::SeqCst);
        let barrier = self.snapshot_barrier.lock().unwrap().take();
        if let Some(barrier) = barrier {
            barrier.wait().await;
            barrier.wait().await;
        }
        let _active = self.enter().await;
        self.snapshots
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(TransportError::InvalidResponse)
    }
}
