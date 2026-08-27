use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::Barrier;
use unfour_cloud_sync::{
    ChangesPage, Clock, CloudWorkspace, DownloadDecision, PushRequest, PushResponse, PushResult,
    PushResultStatus, RemoteChange, SnapshotItem, SnapshotPage, SyncAccountContext,
    SyncDependencies, SyncEntityType, SyncError, SyncOperation, SyncRuntime, SyncTransport,
    TransportError, PAYLOAD_SCHEMA_VERSION, PROTOCOL_VERSION,
};
use unfour_command_bus::{CommandBus, CommandBusExtensions};
use unfour_core::models::{ApiRequestInput, KeyValue, WorkspaceVariableInput};
use unfour_local_storage::LocalDb;

#[derive(Default)]
struct MockTransport {
    pushes: Mutex<Vec<PushRequest>>,
    changes: Mutex<VecDeque<ChangesPage>>,
    snapshots: Mutex<VecDeque<SnapshotPage>>,
    roots: Mutex<Vec<String>>,
    account_id: Mutex<String>,
    generation: AtomicU64,
    fail_pushes: AtomicUsize,
    permanent_pushes: AtomicUsize,
    permanent_operation: Mutex<Option<(String, String)>>,
    unknown_permanent_operation: Mutex<Option<(String, String)>>,
    unauthorized_pushes: AtomicUsize,
    fail_on_push_number: AtomicUsize,
    no_op_pushes: AtomicUsize,
    delay_ms: AtomicUsize,
    active_calls: AtomicUsize,
    max_active_calls: AtomicUsize,
    cursor: AtomicU64,
    account_calls: AtomicUsize,
    fail_account_on_call: AtomicUsize,
    changes_calls: AtomicUsize,
    snapshot_calls: AtomicUsize,
    push_barrier: Mutex<Option<Arc<Barrier>>>,
    changes_barrier: Mutex<Option<Arc<Barrier>>>,
    snapshot_barrier: Mutex<Option<Arc<Barrier>>>,
}

#[derive(Default)]
struct PausingClock {
    pause_next: AtomicBool,
    paused: Mutex<bool>,
    resume: Condvar,
}

impl PausingClock {
    fn pause_next(&self) {
        self.pause_next.store(true, Ordering::SeqCst);
    }

    async fn wait_until_paused(&self) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if *self.paused.lock().unwrap() {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("clock did not pause during error finalization");
    }

    fn is_paused(&self) -> bool {
        *self.paused.lock().unwrap()
    }

    fn resume(&self) {
        *self.paused.lock().unwrap() = false;
        self.resume.notify_all();
    }
}

impl Clock for PausingClock {
    fn now(&self) -> chrono::DateTime<Utc> {
        if self.pause_next.swap(false, Ordering::SeqCst) {
            let mut paused = self.paused.lock().unwrap();
            *paused = true;
            while *paused {
                paused = self.resume.wait(paused).unwrap();
            }
        }
        Utc::now()
    }
}

struct ActiveCall<'a>(&'a MockTransport);
impl Drop for ActiveCall<'_> {
    fn drop(&mut self) {
        self.0.active_calls.fetch_sub(1, Ordering::SeqCst);
    }
}

impl MockTransport {
    fn new() -> Self {
        Self {
            roots: Mutex::new(vec!["workspace-remote".into()]),
            account_id: Mutex::new("account-a".into()),
            ..Self::default()
        }
    }

    async fn enter(&self) -> ActiveCall<'_> {
        let active = self.active_calls.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active_calls.fetch_max(active, Ordering::SeqCst);
        let delay = self.delay_ms.load(Ordering::SeqCst);
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay as u64)).await;
        }
        ActiveCall(self)
    }

    fn switch_account(&self, account_id: &str) {
        *self.account_id.lock().unwrap() = account_id.into();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.cursor.store(0, Ordering::SeqCst);
    }

    fn fail_operation_once(&self, entity_id: &str, code: &str) {
        *self.permanent_operation.lock().unwrap() = Some((entity_id.to_string(), code.to_string()));
    }

    fn fail_unknown_operation_once(&self, operation_id: &str, code: &str) {
        *self.unknown_permanent_operation.lock().unwrap() =
            Some((operation_id.to_string(), code.to_string()));
    }

    fn terminal_page(&self, after: i64, cloud_workspace_id: &str) -> ChangesPage {
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
        Ok(SyncAccountContext {
            account_id: self.account_id.lock().unwrap().clone(),
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
        let _active = self.enter().await;
        Ok(CloudWorkspace {
            cloud_workspace_id: "cloud-created".into(),
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
            .permanent_pushes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                (value > 0).then(|| value - 1)
            })
            .is_ok()
        {
            return Err(TransportError::Permanent("invalid_sync_entity".into()));
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

fn variable(id: Option<String>, key: &str, value: &str, secret: bool) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id,
        key: key.into(),
        value: value.into(),
        is_secret: secret,
        is_enabled: true,
        description: Some("description".into()),
        sort_order: 0,
    }
}

fn saved_api_request(
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<String>,
) -> ApiRequestInput {
    ApiRequestInput {
        workspace_id: workspace_id.into(),
        name: Some("List accounts".into()),
        parent_folder_id,
        collection_id: Some(collection_id.into()),
        auth_json: Some(r#"{"type":"bearer","token":"raw-auth-secret"}"#.into()),
        method: "GET".into(),
        url: "https://example.test/accounts?api_key=raw-url-secret".into(),
        headers: vec![KeyValue {
            key: "Authorization".into(),
            value: "Bearer raw-header-secret".into(),
            enabled: true,
        }],
        query: vec![KeyValue {
            key: "token".into(),
            value: "raw-query-secret".into(),
            enabled: true,
        }],
        body: Some(r#"{"token":"raw-body-secret"}"#.into()),
        body_kind: "json".into(),
        timeout_ms: Some(12_345),
        pre_request_script: Some("console.log('pre')".into()),
        post_response_script: Some("console.log('post')".into()),
        script_schema_version: 1,
        temporary_variables: vec![KeyValue {
            key: "runtime".into(),
            value: "raw-runtime-secret".into(),
            enabled: true,
        }],
    }
}

async fn database() -> LocalDb {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let db = LocalDb::from_pool(pool);
    db.migrate().await.unwrap();
    unfour_cloud_sync_storage::migrate(db.pool()).await.unwrap();
    enable_test_accounts(&db).await;
    db
}

async fn concurrent_database() -> LocalDb {
    static DATABASE_ID: AtomicUsize = AtomicUsize::new(0);
    let id = DATABASE_ID.fetch_add(1, Ordering::SeqCst);
    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite:file:cloud-sync-worker-{id}?mode=memory&cache=shared"
    ))
    .unwrap()
    .create_if_missing(true)
    .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    let db = LocalDb::from_pool(pool);
    db.migrate().await.unwrap();
    unfour_cloud_sync_storage::migrate(db.pool()).await.unwrap();
    enable_test_accounts(&db).await;
    db
}

async fn enable_test_accounts(db: &LocalDb) {
    // Worker tests exercise active synchronization; production accounts still
    // use the persisted default-off setting created by account activation.
    for account_id in ["account-a", "account-b"] {
        sqlx::query(
            "INSERT INTO cloud_sync_account_settings (account_id, sync_enabled, updated_at) VALUES (?1, 1, '2026-07-29T00:00:00Z')",
        )
        .bind(account_id)
        .execute(db.pool())
        .await
        .unwrap();
    }
}

fn workspace_payload(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name, "environmentType": "dev", "mcpPolicy": "auto",
        "createdAt": "2026-07-28T00:00:00Z", "updatedAt": "2026-07-28T00:00:00Z",
        "deletedAt": null
    })
}

fn variable_payload(value: &str) -> serde_json::Value {
    serde_json::json!({
        "key": "KEY", "value": value, "isSecret": false, "isEnabled": true,
        "description": null, "sortOrder": 0, "createdAt": "2026-07-28T00:00:00Z",
        "updatedAt": "2026-07-28T00:00:00Z", "deletedAt": null
    })
}

fn remote_variable_change(
    workspace_id: &str,
    cursor: i64,
    operation_id: &str,
    entity_id: &str,
    key: &str,
    value: &str,
) -> RemoteChange {
    let mut payload = variable_payload(value);
    payload["key"] = serde_json::json!(key);
    RemoteChange {
        cursor,
        operation_id: operation_id.into(),
        entity_type: SyncEntityType::WorkspaceVariable,
        entity_id: entity_id.into(),
        parent_entity_id: Some(workspace_id.into()),
        operation: SyncOperation::Upsert,
        server_version: 1,
        payload_schema_version: 1,
        payload: Some(payload),
        deleted_at: None,
    }
}

#[path = "worker/api_hierarchy.rs"]
mod api_hierarchy;
#[path = "worker/connections.rs"]
mod connections;
#[path = "worker/dead_letter_recovery.rs"]
mod dead_letter_recovery;
#[path = "worker/hierarchy_conflicts.rs"]
mod hierarchy_conflicts;
#[path = "worker/ssh_tasks.rs"]
mod ssh_tasks;
#[tokio::test]
async fn initial_upload_is_topological_and_matches_all_four_v1_payloads() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    seed.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "TOKEN", "must-never-sync", true),
    )
    .await
    .unwrap();
    let environment = seed
        .workspace_environment_create(workspace_id.clone(), "Test".into())
        .await
        .unwrap();
    seed.workspace_environment_variable_create(
        workspace_id.clone(),
        environment.id.clone(),
        variable(None, "HOST", "example.test", false),
    )
    .await
    .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();

    let pushes = transport.pushes.lock().unwrap();
    let operations = pushes
        .iter()
        .flat_map(|request| request.operations.iter())
        .collect::<Vec<_>>();
    assert_eq!(
        operations.first().unwrap().entity_type,
        SyncEntityType::Workspace
    );
    assert_eq!(
        operations.last().unwrap().entity_type,
        SyncEntityType::WorkspaceEnvironmentVariable
    );
    for operation in &operations {
        let payload = operation.payload.as_ref().unwrap();
        for forbidden in [
            "id",
            "workspaceId",
            "environmentId",
            "secretValue",
            "revision",
            "isDefault",
            "lastOpenedAt",
            "isActive",
        ] {
            assert!(payload.get(forbidden).is_none());
        }
        match operation.entity_type {
            SyncEntityType::Workspace => {
                assert!(operation.parent_entity_id.is_none());
                assert_payload_keys(
                    payload,
                    &[
                        "createdAt",
                        "deletedAt",
                        "environmentType",
                        "mcpPolicy",
                        "name",
                        "updatedAt",
                    ],
                );
            }
            SyncEntityType::WorkspaceVariable => {
                assert_eq!(
                    operation.parent_entity_id.as_deref(),
                    Some(workspace_id.as_str())
                );
                assert_payload_keys(
                    payload,
                    &[
                        "createdAt",
                        "deletedAt",
                        "description",
                        "isEnabled",
                        "isSecret",
                        "key",
                        "sortOrder",
                        "updatedAt",
                    ],
                );
            }
            SyncEntityType::WorkspaceEnvironment => {
                assert_eq!(
                    operation.parent_entity_id.as_deref(),
                    Some(workspace_id.as_str())
                );
                assert_payload_keys(
                    payload,
                    &["createdAt", "deletedAt", "name", "sortOrder", "updatedAt"],
                );
            }
            SyncEntityType::WorkspaceEnvironmentVariable => {
                assert_eq!(
                    operation.parent_entity_id.as_deref(),
                    Some(environment.id.as_str())
                );
                assert_payload_keys(
                    payload,
                    &[
                        "createdAt",
                        "deletedAt",
                        "description",
                        "isEnabled",
                        "isSecret",
                        "key",
                        "sortOrder",
                        "updatedAt",
                        "value",
                    ],
                );
            }
            SyncEntityType::Connection
            | SyncEntityType::ApiCollection
            | SyncEntityType::ApiFolder
            | SyncEntityType::ApiRequest
            | SyncEntityType::SshTask
            | SyncEntityType::SshTaskStep => {
                panic!("unexpected feature entity in workspace-only fixture")
            }
        }
    }
    let serialized = serde_json::to_string(&*pushes).unwrap();
    assert!(!serialized.contains("must-never-sync"));
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.binding.unwrap().state, "active");
    let stored: String = sqlx::query_scalar(
        "SELECT group_concat(coalesce(canonical_payload_json, ''), '') FROM cloud_sync_outbox",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!stored.contains("must-never-sync"));
}

#[tokio::test]
async fn api_initial_upload_uses_core_snapshots_and_existing_push_pipeline() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let collection = seed
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    let root = seed
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Root".into(),
        )
        .await
        .unwrap();
    let child = seed
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            Some(root.id.clone()),
            "Child".into(),
        )
        .await
        .unwrap();
    let request = seed
        .save_api_request(saved_api_request(
            &workspace_id,
            &collection.id,
            Some(child.id.clone()),
        ))
        .await
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();

    {
        let pushes = transport.pushes.lock().unwrap();
        let operations = pushes
            .iter()
            .flat_map(|push| push.operations.iter())
            .collect::<Vec<_>>();
        let collection_op = operations
            .iter()
            .find(|operation| operation.entity_id == collection.id)
            .expect("collection push");
        assert_eq!(collection_op.entity_type, SyncEntityType::ApiCollection);
        assert_eq!(
            collection_op.parent_entity_id.as_deref(),
            Some(workspace_id.as_str())
        );
        assert_payload_keys(
            collection_op.payload.as_ref().unwrap(),
            &["createdAt", "description", "name", "updatedAt"],
        );

        let root_op = operations
            .iter()
            .find(|operation| operation.entity_id == root.id)
            .expect("root folder push");
        assert_eq!(root_op.entity_type, SyncEntityType::ApiFolder);
        assert_eq!(
            root_op.parent_entity_id.as_deref(),
            Some(collection.id.as_str())
        );
        let child_op = operations
            .iter()
            .find(|operation| operation.entity_id == child.id)
            .expect("child folder push");
        assert_eq!(child_op.parent_entity_id.as_deref(), Some(root.id.as_str()));

        let request_op = operations
            .iter()
            .find(|operation| operation.entity_id == request.id)
            .expect("request push");
        assert_eq!(request_op.entity_type, SyncEntityType::ApiRequest);
        assert_eq!(
            request_op.parent_entity_id.as_deref(),
            Some(child.id.as_str())
        );
        assert_payload_keys(
            request_op.payload.as_ref().unwrap(),
            &[
                "authJson",
                "body",
                "bodyKind",
                "collectionId",
                "createdAt",
                "headers",
                "method",
                "name",
                "parentFolderId",
                "postResponseScript",
                "preRequestScript",
                "query",
                "scriptSchemaVersion",
                "sortOrder",
                "updatedAt",
                "url",
            ],
        );
        for forbidden in [
            "id",
            "workspaceId",
            "revision",
            "syncStatus",
            "remoteId",
            "timeoutMs",
            "temporaryVariables",
        ] {
            assert!(request_op
                .payload
                .as_ref()
                .unwrap()
                .get(forbidden)
                .is_none());
        }
        let serialized = serde_json::to_string(&*pushes).unwrap();
        for secret in [
            "raw-auth-secret",
            "raw-header-secret",
            "raw-query-secret",
            "raw-url-secret",
            "raw-body-secret",
            "raw-runtime-secret",
        ] {
            assert!(!serialized.contains(secret), "push leaked {secret}");
        }
    }
    assert_eq!(service.status(&workspace_id).await.unwrap().dead_count, 0);
}

#[tokio::test]
async fn api_request_uses_existing_uncertain_retry_and_dead_letter_paths() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Retry".into())
        .await
        .unwrap();
    service.sync_workspace(&workspace_id).await.unwrap();

    let request = bus
        .save_api_request(saved_api_request(&workspace_id, &collection.id, None))
        .await
        .unwrap();
    transport.fail_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Transport)
    ));
    let uncertain = service.status(&workspace_id).await.unwrap();
    assert_eq!(uncertain.uncertain_count, 1);
    assert_eq!(uncertain.dead_count, 0);
    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = NULL WHERE entity_id = ?1")
        .bind(&request.id)
        .execute(db.pool())
        .await
        .unwrap();
    transport.permanent_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Permanent)
    ));
    let dead = service.status(&workspace_id).await.unwrap();
    assert_eq!(dead.uncertain_count, 0);
    assert_eq!(dead.dead_count, 1);
    assert_eq!(dead.dead_letters[0].entity_type, "apiRequest");
    assert_eq!(dead.dead_letters[0].entity_id, request.id);
    assert_eq!(dead.dead_letters[0].error_code, "invalid_sync_entity");
}

fn assert_payload_keys(payload: &serde_json::Value, expected: &[&str]) {
    let mut actual = payload
        .as_object()
        .expect("canonical payload object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn pending_local_intent_blocks_remote_upsert_and_delete_without_losing_local_data() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "KEY", "local-value", false),
        )
        .await
        .unwrap();
    let cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor + 1,
        next_cursor: cursor + 1,
        changes: vec![RemoteChange {
            cursor: cursor + 1,
            operation_id: "remote-upsert".into(),
            entity_type: SyncEntityType::WorkspaceVariable,
            entity_id: local.id.clone(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Upsert,
            server_version: 2,
            payload_schema_version: 1,
            payload: Some(variable_payload("remote-value")),
            deleted_at: None,
        }],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    let value: String = sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = ?1")
        .bind(&local.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(value, "local-value");
    assert_eq!(
        service.conflicts(&workspace_id).await.unwrap()[0]
            .remote_payload
            .as_ref()
            .unwrap()["value"],
        "remote-value"
    );

    service
        .keep_local(&workspace_id, SyncEntityType::WorkspaceVariable, &local.id)
        .await
        .unwrap();
    bus.workspace_variable_update(
        workspace_id.clone(),
        local.id.clone(),
        variable(Some(local.id.clone()), "KEY", "new-local", false),
    )
    .await
    .unwrap();
    let cursor = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: cursor + 1,
        next_cursor: cursor + 1,
        changes: vec![RemoteChange {
            cursor: cursor + 1,
            operation_id: "remote-delete".into(),
            entity_type: SyncEntityType::WorkspaceVariable,
            entity_id: local.id.clone(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Delete,
            server_version: 4,
            payload_schema_version: 1,
            payload: None,
            deleted_at: Some("2026-07-28T01:00:00Z".into()),
        }],
    });
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Conflict)
    ));
    let row: (String, Option<String>) =
        sqlx::query_as("SELECT value, deleted_at FROM workspace_variables WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(row, ("new-local".into(), None));
}

#[tokio::test]
async fn push_global_cursor_does_not_skip_interleaved_remote_changes() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "LOCAL", "local-value", false),
        )
        .await
        .unwrap();
    let operation_id: String =
        sqlx::query_scalar("SELECT operation_id FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    sqlx::query(
        "UPDATE cloud_sync_workspace_bindings SET last_pulled_cursor = 10 WHERE local_workspace_id = ?1",
    )
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    transport.cursor.store(11, Ordering::SeqCst);
    transport
        .changes
        .lock()
        .unwrap()
        .push_back(transport.terminal_page(10, "cloud-created"));
    let push_barrier = Arc::new(Barrier::new(2));
    *transport.push_barrier.lock().unwrap() = Some(push_barrier.clone());
    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };

    push_barrier.wait().await;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: 12,
        next_cursor: 12,
        changes: vec![
            remote_variable_change(
                &workspace_id,
                11,
                "remote-interleaved",
                "remote-variable",
                "REMOTE",
                "remote-value",
            ),
            remote_variable_change(
                &workspace_id,
                12,
                &operation_id,
                &local.id,
                "LOCAL",
                "local-value",
            ),
        ],
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());
    push_barrier.wait().await;
    barrier.wait().await;
    assert_eq!(transport.cursor.load(Ordering::SeqCst), 12);
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        10,
        "a Push response at the workspace's global cursor must not acknowledge unseen changes",
    );
    barrier.wait().await;
    worker.await.unwrap().unwrap();

    let values: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, value FROM workspace_variables WHERE id IN (?1, 'remote-variable') ORDER BY id",
    )
    .bind(&local.id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        values,
        vec![
            (local.id, "local-value".into()),
            ("remote-variable".into(), "remote-value".into()),
        ]
    );
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        12,
    );
}

#[tokio::test]
async fn own_pushed_change_is_consumed_once_before_pull_cursor_advances() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.cursor.store(0, Ordering::SeqCst);
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    let local = bus
        .workspace_variable_create(workspace_id.clone(), variable(None, "OWN", "one", false))
        .await
        .unwrap();
    let operation_id: String =
        sqlx::query_scalar("SELECT operation_id FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    transport
        .changes
        .lock()
        .unwrap()
        .push_back(transport.terminal_page(0, "cloud-created"));
    let push_barrier = Arc::new(Barrier::new(2));
    *transport.push_barrier.lock().unwrap() = Some(push_barrier.clone());
    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };

    push_barrier.wait().await;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: 1,
        next_cursor: 1,
        changes: vec![remote_variable_change(
            &workspace_id,
            1,
            &operation_id,
            &local.id,
            "OWN",
            "one",
        )],
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());
    push_barrier.wait().await;
    barrier.wait().await;
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        0,
    );
    barrier.wait().await;
    worker.await.unwrap().unwrap();

    let business_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workspace_variables WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let outbox_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(business_rows, 1);
    assert_eq!(outbox_rows, 0);
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        1,
    );
}

#[tokio::test]
async fn lost_response_replays_same_operation_as_no_op_and_recovers() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.pushes.lock().unwrap().clear();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    transport.cursor.store(0, Ordering::SeqCst);
    let local = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "RETRY", "value", false),
        )
        .await
        .unwrap();
    transport.fail_pushes.store(1, Ordering::SeqCst);
    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::Transport)
    ));
    let first_id = transport.pushes.lock().unwrap()[0].operations[0]
        .operation_id
        .clone();
    let uncertain = service.status(&workspace_id).await.unwrap();
    assert_eq!(uncertain.uncertain_count, 1);
    assert_eq!(uncertain.binding.unwrap().last_pulled_cursor, 0);
    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = NULL")
        .execute(db.pool())
        .await
        .unwrap();
    transport.no_op_pushes.store(1, Ordering::SeqCst);
    transport
        .changes
        .lock()
        .unwrap()
        .push_back(transport.terminal_page(0, "cloud-created"));
    let push_barrier = Arc::new(Barrier::new(2));
    *transport.push_barrier.lock().unwrap() = Some(push_barrier.clone());
    let (restarted, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    let worker = {
        let restarted = restarted.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { restarted.sync_workspace(&workspace_id).await })
    };

    push_barrier.wait().await;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: 1,
        next_cursor: 1,
        changes: vec![remote_variable_change(
            &workspace_id,
            1,
            &first_id,
            &local.id,
            "RETRY",
            "value",
        )],
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());
    push_barrier.wait().await;
    barrier.wait().await;
    assert_eq!(
        restarted
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        0,
    );
    barrier.wait().await;
    worker.await.unwrap().unwrap();
    let pushes = transport.pushes.lock().unwrap();
    assert_eq!(pushes[1].operations[0].operation_id, first_id);
    drop(pushes);
    assert_eq!(
        restarted.status(&workspace_id).await.unwrap().pending_count,
        0
    );
    let status = restarted.status(&workspace_id).await.unwrap();
    assert_eq!(status.uncertain_count, 0);
    assert_eq!(status.binding.unwrap().last_pulled_cursor, 1);
    let business_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workspace_variables WHERE id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let outbox_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_id = ?1")
            .bind(&local.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(business_rows, 1);
    assert_eq!(outbox_rows, 0);
}

#[tokio::test]
async fn account_switch_keeps_bindings_isolated_and_pauses_old_worker_context() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    service.pause_current_account().await.unwrap();
    transport.switch_account("account-b");
    assert!(service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .is_none());
    service.enable(&workspace_id).await.unwrap();
    let bindings: Vec<(String, bool)> = sqlx::query_as(
        "SELECT account_id, sync_enabled FROM cloud_sync_workspace_bindings ORDER BY account_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        bindings,
        vec![("account-a".into(), false), ("account-b".into(), true)]
    );
}

#[tokio::test]
async fn cloud_workspace_list_uses_the_root_snapshot_name() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(1, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Workspace,
            entity_id: "workspace-remote".into(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: 1,
            payload: workspace_payload("Remote workspace"),
        }],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db, transport);

    let workspaces = service.list_cloud_workspaces().await.unwrap();

    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].name.as_deref(), Some("Remote workspace"));
}

#[tokio::test]
async fn download_reports_a_workspace_name_conflict_before_core_apply() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let local = seed
        .list_workspaces()
        .await
        .unwrap()
        .workspaces
        .into_iter()
        .next()
        .unwrap();
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(1, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Workspace,
            entity_id: "workspace-remote".into(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: 1,
            payload: workspace_payload(&local.name.to_ascii_lowercase()),
        }],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);

    assert!(matches!(
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await,
        Err(SyncError::WorkspaceNameConflict)
    ));
    let untouched: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM workspaces), (SELECT COUNT(*) FROM cloud_sync_workspace_bindings), (SELECT COUNT(*) FROM cloud_sync_snapshot_staging)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(untouched, (1, 0, 0));
}

#[tokio::test]
async fn download_is_paged_staged_atomic_and_refuses_an_existing_local_root() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let local_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    *transport.roots.lock().unwrap() = vec![local_id.clone()];
    transport.cursor.store(2, Ordering::SeqCst);
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 2,
        current_cursor: 2,
        items: vec![SnapshotItem {
            entity_type: SyncEntityType::Workspace,
            entity_id: local_id.clone(),
            parent_entity_id: None,
            server_version: 1,
            payload_schema_version: 1,
            payload: workspace_payload("Remote"),
        }],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert!(matches!(
        service.enable(&local_id).await,
        Err(SyncError::CloudWorkspaceNotEmpty)
    ));
    assert!(matches!(
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await,
        Err(SyncError::LocalWorkspaceNotEmpty)
    ));
    let name: String = sqlx::query_scalar("SELECT name FROM workspaces WHERE id = ?1")
        .bind(&local_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_ne!(name, "Remote");
    let untouched: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM cloud_sync_workspace_bindings), (SELECT COUNT(*) FROM cloud_sync_outbox)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(untouched, (0, 0));

    *transport.roots.lock().unwrap() = vec!["workspace-new".into()];
    transport.snapshots.lock().unwrap().extend([
        SnapshotPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-1".into(),
            at_cursor: 2,
            current_cursor: 2,
            items: vec![SnapshotItem {
                entity_type: SyncEntityType::Workspace,
                entity_id: "workspace-new".into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: 1,
                payload: workspace_payload("Downloaded"),
            }],
            next_page_token: Some("page-2".into()),
        },
        SnapshotPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-1".into(),
            at_cursor: 2,
            current_cursor: 2,
            items: vec![SnapshotItem {
                entity_type: SyncEntityType::WorkspaceVariable,
                entity_id: "remote-variable".into(),
                parent_entity_id: Some("workspace-new".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: variable_payload("downloaded"),
            }],
            next_page_token: None,
        },
    ]);
    let downloaded = service
        .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
        .await
        .unwrap();
    assert_eq!(downloaded, "workspace-new");
    let value: String =
        sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = 'remote-variable'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(value, "downloaded");
    let staged: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_snapshot_staging")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(staged, 0);
}

#[tokio::test]
async fn full_snapshot_download_restores_api_tree_through_core_external_apply() {
    let db = database().await;
    let transport = Arc::new(MockTransport::new());
    transport.cursor.store(1, Ordering::SeqCst);
    let workspace_id = "workspace-remote";
    transport.snapshots.lock().unwrap().push_back(SnapshotPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-1".into(),
        at_cursor: 1,
        current_cursor: 1,
        items: vec![
            SnapshotItem {
                entity_type: SyncEntityType::Workspace,
                entity_id: workspace_id.into(),
                parent_entity_id: None,
                server_version: 1,
                payload_schema_version: 1,
                payload: workspace_payload("Remote API"),
            },
            SnapshotItem {
                entity_type: SyncEntityType::ApiCollection,
                entity_id: "collection-1".into(),
                parent_entity_id: Some(workspace_id.into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({
                    "name": "Accounts",
                    "description": null,
                    "createdAt": "2026-08-13T00:00:00Z",
                    "updatedAt": "2026-08-13T00:00:00Z"
                }),
            },
            // Deliberately child-before-parent; Core owns folder topology.
            SnapshotItem {
                entity_type: SyncEntityType::ApiFolder,
                entity_id: "a-child".into(),
                parent_entity_id: Some("z-root".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({
                    "collectionId": "collection-1",
                    "parentFolderId": "z-root",
                    "name": "Child",
                    "sortOrder": 1,
                    "createdAt": "2026-08-13T00:00:00Z",
                    "updatedAt": "2026-08-13T00:00:00Z"
                }),
            },
            SnapshotItem {
                entity_type: SyncEntityType::ApiFolder,
                entity_id: "z-root".into(),
                parent_entity_id: Some("collection-1".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({
                    "collectionId": "collection-1",
                    "parentFolderId": null,
                    "name": "Root",
                    "sortOrder": 0,
                    "createdAt": "2026-08-13T00:00:00Z",
                    "updatedAt": "2026-08-13T00:00:00Z"
                }),
            },
            SnapshotItem {
                entity_type: SyncEntityType::ApiRequest,
                entity_id: "request-1".into(),
                parent_entity_id: Some("a-child".into()),
                server_version: 1,
                payload_schema_version: 1,
                payload: serde_json::json!({
                    "collectionId": "collection-1",
                    "parentFolderId": "a-child",
                    "name": "List accounts",
                    "sortOrder": 0,
                    "authJson": "{}",
                    "method": "GET",
                    "url": "https://example.test/accounts",
                    "headers": [],
                    "query": [],
                    "body": null,
                    "bodyKind": "none",
                    "preRequestScript": null,
                    "postResponseScript": null,
                    "scriptSchemaVersion": 1,
                    "createdAt": "2026-08-13T00:00:00Z",
                    "updatedAt": "2026-08-13T00:00:00Z"
                }),
            },
        ],
        next_page_token: None,
    });
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    assert_eq!(
        service
            .download_workspace("cloud-1", DownloadDecision::DownloadToNewWorkspace)
            .await
            .unwrap(),
        workspace_id
    );
    let restored: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM api_collections WHERE workspace_id = ?1 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM api_collection_folders WHERE workspace_id = ?1 AND deleted_at IS NULL),
             (SELECT COUNT(*) FROM api_requests WHERE workspace_id = ?1 AND deleted_at IS NULL)"#,
    )
    .bind(workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(restored, (1, 2, 1));
    let location: (String, Option<String>) = sqlx::query_as(
        "SELECT collection_id, parent_folder_id FROM api_requests WHERE id = 'request-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(location, ("collection-1".into(), Some("a-child".into())));
}

#[tokio::test]
async fn repeated_workspace_triggers_coalesce_and_global_calls_stay_bounded() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.changes_calls.store(0, Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    assert_eq!(transport.changes_calls.load(Ordering::SeqCst), 1);
    transport.max_active_calls.store(0, Ordering::SeqCst);
    transport.delay_ms.store(20, Ordering::SeqCst);
    let mut tasks = Vec::new();
    for _ in 0..10 {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tasks.push(tokio::spawn(async move {
            service.sync_workspace(&workspace_id).await
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    assert_eq!(transport.max_active_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn trigger_during_error_finalization_stays_in_the_same_flight() {
    let db = concurrent_database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let clock = Arc::new(PausingClock::default());
    let dependencies = SyncDependencies {
        clock: clock.clone(),
        ..SyncDependencies::default()
    };
    let (service, _, _) = SyncRuntime::build_with_dependencies(db, transport.clone(), dependencies);
    service.enable(&workspace_id).await.unwrap();

    let status = service.status(&workspace_id).await.unwrap();
    let binding = status.binding.unwrap();
    transport.changes_calls.store(0, Ordering::SeqCst);
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION + 1,
        cloud_workspace_id: binding.cloud_workspace_id,
        current_cursor: binding.last_pulled_cursor,
        next_cursor: binding.last_pulled_cursor,
        changes: Vec::new(),
    });
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());

    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    barrier.wait().await;
    clock.pause_next();
    barrier.wait().await;
    clock.wait_until_paused().await;

    let merged_trigger = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    let merged_result = tokio::time::timeout(Duration::from_secs(2), merged_trigger).await;
    let old_worker_was_paused = clock.is_paused();
    clock.resume();
    merged_result
        .expect("trigger should merge without waiting for finalization")
        .unwrap()
        .unwrap();
    assert!(
        old_worker_was_paused,
        "the old worker must still be paused while the trigger merges"
    );
    assert_eq!(
        transport.changes_calls.load(Ordering::SeqCst),
        1,
        "the trigger must not start a second worker while the old worker is finalizing"
    );

    assert!(matches!(worker.await.unwrap(), Err(SyncError::InvalidData)));
    assert!(
        transport.changes_calls.load(Ordering::SeqCst) >= 2,
        "the dirty trigger must continue within the existing flight"
    );
    let recovered = service.status(&workspace_id).await.unwrap();
    assert_eq!(recovered.binding.as_ref().unwrap().state, "active");
    assert_eq!(recovered.binding.as_ref().unwrap().last_error, None);
}

#[tokio::test]
async fn stale_generation_error_cannot_overwrite_a_newer_success() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport);
    service.enable(&workspace_id).await.unwrap();
    let old_binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();

    sqlx::query(
        "UPDATE cloud_sync_workspace_bindings SET generation = generation + 1, state = 'active', last_error = NULL WHERE account_id = ?1 AND local_workspace_id = ?2",
    )
    .bind(&old_binding.account_id)
    .bind(&workspace_id)
    .execute(db.pool())
    .await
    .unwrap();
    service
        .repository()
        .record_error(
            &old_binding.account_id,
            &workspace_id,
            old_binding.generation.try_into().unwrap(),
            SyncError::Transport.code(),
            Utc::now(),
        )
        .await
        .unwrap();

    let state: (String, Option<String>) = sqlx::query_as(
        "SELECT state, last_error FROM cloud_sync_workspace_bindings WHERE account_id = ?1 AND local_workspace_id = ?2",
    )
    .bind(&old_binding.account_id)
    .bind(&workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, ("active".into(), None));
}

#[tokio::test]
async fn dirty_account_refresh_failure_releases_flight_for_a_new_worker() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport.clone());
    service.enable(&workspace_id).await.unwrap();
    transport.changes_calls.store(0, Ordering::SeqCst);
    let barrier = Arc::new(Barrier::new(2));
    *transport.changes_barrier.lock().unwrap() = Some(barrier.clone());

    let worker = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    barrier.wait().await;

    let mut dirty_triggers = Vec::new();
    for _ in 0..6 {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        dirty_triggers.push(tokio::spawn(async move {
            service.sync_workspace(&workspace_id).await
        }));
    }
    for trigger in dirty_triggers {
        trigger.await.unwrap().unwrap();
    }
    transport.fail_account_on_call.store(
        transport.account_calls.load(Ordering::SeqCst) + 1,
        Ordering::SeqCst,
    );
    barrier.wait().await;
    assert!(matches!(
        worker.await.unwrap(),
        Err(SyncError::Unauthorized)
    ));
    assert!(!service.status(&workspace_id).await.unwrap().running);

    let before_restart = transport.changes_calls.load(Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    let total_calls = transport.changes_calls.load(Ordering::SeqCst);
    assert!(
        total_calls > before_restart,
        "a new worker must perform network work"
    );
    assert!(total_calls <= 4, "dirty triggers must remain coalesced");
}

#[tokio::test]
async fn empty_cloud_with_only_a_local_root_has_a_recoverable_initial_upload() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db, transport);

    service.enable(&workspace_id).await.unwrap();
    let status = service.status(&workspace_id).await.unwrap();
    assert_eq!(status.binding.unwrap().state, "active");
    assert_eq!(status.dead_count, 0);
}

#[tokio::test]
async fn initial_upload_cross_batch_checkpoint_resumes_after_restart() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    for index in 0..55 {
        seed.workspace_variable_create(
            workspace_id.clone(),
            variable(None, &format!("KEY_{index:02}"), "value", false),
        )
        .await
        .unwrap();
    }
    let transport = Arc::new(MockTransport::new());
    transport.fail_on_push_number.store(2, Ordering::SeqCst);
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    assert!(matches!(
        service.enable(&workspace_id).await,
        Err(SyncError::Transport)
    ));
    let failed = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(failed.state, "error");
    assert_eq!(failed.initial_total, 56);
    assert_eq!(failed.initial_confirmed, 50);
    assert!(failed.initialization_checkpoint.is_some());

    sqlx::query("UPDATE cloud_sync_outbox SET next_attempt_at = NULL")
        .execute(db.pool())
        .await
        .unwrap();
    let (restarted, _, _) = SyncRuntime::build(db, transport);
    restarted.sync_workspace(&workspace_id).await.unwrap();
    let completed = restarted
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(completed.state, "active");
    assert_eq!(completed.initial_confirmed, completed.initial_total);
}

#[tokio::test]
async fn multi_page_changes_continue_until_next_cursor_equals_current_cursor() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let base = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    let page = |cursor: i64, id: &str, value: &str| {
        let mut payload = variable_payload(value);
        payload["key"] = serde_json::json!(id);
        RemoteChange {
            cursor,
            operation_id: format!("remote-{cursor}"),
            entity_type: SyncEntityType::WorkspaceVariable,
            entity_id: id.into(),
            parent_entity_id: Some(workspace_id.clone()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: 1,
            payload: Some(payload),
            deleted_at: None,
        }
    };
    transport.changes.lock().unwrap().extend([
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-created".into(),
            current_cursor: base + 2,
            next_cursor: base + 1,
            changes: vec![page(base + 1, "remote-one", "one")],
        },
        ChangesPage {
            protocol_version: PROTOCOL_VERSION,
            cloud_workspace_id: "cloud-created".into(),
            current_cursor: base + 2,
            next_cursor: base + 2,
            changes: vec![page(base + 2, "remote-two", "two")],
        },
    ]);
    service.sync_workspace(&workspace_id).await.unwrap();
    let values: Vec<String> = sqlx::query_scalar(
        "SELECT value FROM workspace_variables WHERE id IN ('remote-one', 'remote-two') ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(values, vec!["one", "two"]);
    assert_eq!(
        service
            .status(&workspace_id)
            .await
            .unwrap()
            .binding
            .unwrap()
            .last_pulled_cursor,
        base + 2
    );
}

#[tokio::test]
async fn gapped_changes_page_does_not_advance_the_pull_cursor() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let base = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap()
        .last_pulled_cursor;
    transport.changes.lock().unwrap().push_back(ChangesPage {
        protocol_version: PROTOCOL_VERSION,
        cloud_workspace_id: "cloud-created".into(),
        current_cursor: base + 2,
        next_cursor: base + 2,
        changes: vec![remote_variable_change(
            &workspace_id,
            base + 2,
            "remote-gap",
            "remote-gap-variable",
            "GAP",
            "must-not-apply",
        )],
    });

    assert!(matches!(
        service.sync_workspace(&workspace_id).await,
        Err(SyncError::InvalidData)
    ));
    let binding = service
        .status(&workspace_id)
        .await
        .unwrap()
        .binding
        .unwrap();
    assert_eq!(binding.last_pulled_cursor, base);
    let applied: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_variables WHERE id = 'remote-gap-variable'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(applied, 0);
}

#[tokio::test]
async fn sign_out_pause_generation_rejects_an_old_in_flight_result() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, hook, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "IN_FLIGHT", "preserved", false),
    )
    .await
    .unwrap();
    transport.delay_ms.store(80, Ordering::SeqCst);
    let task = {
        let service = service.clone();
        let workspace_id = workspace_id.clone();
        tokio::spawn(async move { service.sync_workspace(&workspace_id).await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    service.pause_current_account().await.unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(SyncError::AccountChanged)
    ));
    let preserved: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox WHERE account_id = 'account-a'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(preserved, 1);
    let state: (bool, String) = sqlx::query_as(
        "SELECT sync_enabled, state FROM cloud_sync_workspace_bindings WHERE account_id = 'account-a'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, (false, "paused".into()));
}

#[tokio::test]
async fn global_pause_preserves_workspace_preferences_and_resumes_them() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let transport = Arc::new(MockTransport::new());
    let (service, _, _) = SyncRuntime::build(db.clone(), transport.clone());
    service.enable(&workspace_id).await.unwrap();

    service.set_global_sync_enabled(false).await.unwrap();
    assert!(!service.global_sync_enabled().await.unwrap());
    let paused = service.status(&workspace_id).await.unwrap();
    assert!(paused.binding.as_ref().unwrap().sync_enabled);

    let calls_before = transport.changes_calls.load(Ordering::SeqCst);
    service.sync_workspace(&workspace_id).await.unwrap();
    assert_eq!(transport.changes_calls.load(Ordering::SeqCst), calls_before);

    service.set_global_sync_enabled(true).await.unwrap();
    assert!(service.global_sync_enabled().await.unwrap());
    let resumed = service.status(&workspace_id).await.unwrap();
    assert!(resumed.binding.unwrap().sync_enabled);
    for _ in 0..20 {
        if transport.changes_calls.load(Ordering::SeqCst) > calls_before {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(transport.changes_calls.load(Ordering::SeqCst) > calls_before);
}
