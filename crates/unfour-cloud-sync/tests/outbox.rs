use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqliteConnection;
use unfour_cloud_sync::{Clock, IdGenerator, SyncOutboxHook, SyncRepository};
use unfour_command_bus::{CommandBus, CommandBusExtensions, TransactionalCommandHook};
use unfour_core::domain::{
    CommandContext, DomainMutation, ExternalApiCollectionApply, ExternalApiCollectionUpsert,
    ExternalApplyPage, ExternalSshTaskApply, ExternalSshTaskStepApply, ExternalSshTaskStepUpsert,
    ExternalSshTaskUpsert, ExternalVariableValue, ExternalWorkspaceVariableApply,
    ExternalWorkspaceVariableUpsert,
};
use unfour_core::models::{
    ApiRequestInput, DatabaseConnectionInput, KeyValue, SshConnectionInput, SshTaskSaveInput,
    SshTaskStepInput, WorkspaceVariableInput,
};
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::SshService;

struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        "2026-07-27T12:00:00Z".parse().expect("fixed time")
    }
}

#[derive(Default)]
struct SequenceIds(AtomicUsize);

impl IdGenerator for SequenceIds {
    fn next_id(&self) -> String {
        format!("operation-{}", self.0.fetch_add(1, Ordering::SeqCst))
    }
}

fn variable(id: Option<String>, key: &str, value: &str, secret: bool) -> WorkspaceVariableInput {
    WorkspaceVariableInput {
        id,
        key: key.into(),
        value: value.into(),
        is_secret: secret,
        is_enabled: true,
        description: Some("metadata".into()),
        sort_order: 0,
    }
}

fn api_request(
    workspace_id: &str,
    collection_id: &str,
    parent_folder_id: Option<String>,
) -> ApiRequestInput {
    ApiRequestInput {
        workspace_id: workspace_id.into(),
        name: Some("Secret request".into()),
        parent_folder_id,
        collection_id: Some(collection_id.into()),
        auth_json: Some(r#"{"type":"bearer","token":"raw-auth-secret"}"#.into()),
        method: "POST".into(),
        url: "https://example.test?api_key=raw-url-secret".into(),
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
        timeout_ms: Some(9_999),
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

fn ssh_command_step(id: Option<String>, name: &str, position: i64) -> SshTaskStepInput {
    SshTaskStepInput {
        id,
        name: name.into(),
        step_type: "command".into(),
        position,
        enabled: true,
        config_version: Some(1),
        config_json: serde_json::json!({
            "command": format!("echo {name}"),
            "workingDirectory": "",
            "timeoutSeconds": 30,
            "continueOnError": false
        }),
    }
}

fn ssh_task_input(workspace_id: &str) -> SshTaskSaveInput {
    SshTaskSaveInput {
        id: None,
        workspace_id: workspace_id.into(),
        name: "Deploy".into(),
        description: "Deploy the service".into(),
        default_connection_id: None,
        steps: vec![
            ssh_command_step(None, "Build", 0),
            ssh_command_step(None, "Restart", 1),
        ],
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
        .expect("sqlite");
    let db = LocalDb::from_pool(pool);
    db.migrate().await.expect("core migrations");
    unfour_cloud_sync_storage::migrate(db.pool())
        .await
        .expect("pro migrations");
    db
}

async fn hooked_bus(
    trailing_hooks: Vec<Arc<dyn TransactionalCommandHook>>,
) -> (CommandBus, LocalDb, String) {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.expect("seed bus");
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let ids: Arc<dyn IdGenerator> = Arc::new(SequenceIds::default());
    let clock: Arc<dyn Clock> = Arc::new(FixedClock);
    let repository = SyncRepository::new(db.pool().clone());
    repository
        .create_binding_with_initial_outbox(
            "account-1",
            0,
            &workspace_id,
            "cloud-1",
            0,
            ids.as_ref(),
            clock.as_ref(),
        )
        .await
        .expect("binding");
    repository
        .activate_account("account-1", 0, clock.now())
        .await
        .expect("active account");
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .unwrap();
    let hook = Arc::new(SyncOutboxHook::new(ids, clock, None));
    let mut hooks: Vec<Arc<dyn TransactionalCommandHook>> = vec![hook];
    hooks.extend(trailing_hooks);
    let bus = CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(hooks))
        .await
        .expect("hooked bus");
    (bus, db, workspace_id)
}

async fn outbox_row(db: &LocalDb, entity_id: &str) -> (String, String, Option<String>) {
    sqlx::query_as(
        r#"
        SELECT entity_type, operation, parent_entity_id
        FROM cloud_sync_outbox WHERE entity_id = ?1
        ORDER BY created_at DESC LIMIT 1
        "#,
    )
    .bind(entity_id)
    .fetch_one(db.pool())
    .await
    .expect("outbox row")
}

async fn clear_outbox(db: &LocalDb) {
    sqlx::query("DELETE FROM cloud_sync_outbox")
        .execute(db.pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn local_only_mutation_does_not_wake_the_sync_worker() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let (trigger, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let hook = Arc::new(SyncOutboxHook::new(
        Arc::new(SequenceIds::default()),
        Arc::new(FixedClock),
        Some(trigger),
    ));
    let bus = CommandBus::from_db_with_extensions(db, CommandBusExtensions::new(vec![hook]))
        .await
        .unwrap();

    bus.workspace_variable_create(workspace_id, variable(None, "LOCAL_ONLY", "value", false))
        .await
        .unwrap();

    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn global_pause_captures_outbox_without_waking_the_sync_worker() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let ids: Arc<dyn IdGenerator> = Arc::new(SequenceIds::default());
    let clock: Arc<dyn Clock> = Arc::new(FixedClock);
    let repository = SyncRepository::new(db.pool().clone());
    repository
        .create_binding_with_initial_outbox(
            "account-1",
            0,
            &workspace_id,
            "cloud-1",
            0,
            ids.as_ref(),
            clock.as_ref(),
        )
        .await
        .unwrap();
    repository
        .activate_account("account-1", 0, clock.now())
        .await
        .unwrap();
    clear_outbox(&db).await;
    let (trigger, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let hook = Arc::new(SyncOutboxHook::new(ids, clock, Some(trigger)));
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();

    bus.workspace_variable_create(workspace_id, variable(None, "PAUSED", "value", false))
        .await
        .unwrap();

    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(pending, 1);
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn workspace_pause_captures_outbox_without_waking_the_sync_worker() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let ids: Arc<dyn IdGenerator> = Arc::new(SequenceIds::default());
    let clock: Arc<dyn Clock> = Arc::new(FixedClock);
    let repository = SyncRepository::new(db.pool().clone());
    repository
        .create_binding_with_initial_outbox(
            "account-1",
            0,
            &workspace_id,
            "cloud-1",
            0,
            ids.as_ref(),
            clock.as_ref(),
        )
        .await
        .unwrap();
    repository
        .activate_account("account-1", 0, clock.now())
        .await
        .unwrap();
    repository
        .set_global_sync_enabled("account-1", true, clock.now())
        .await
        .unwrap();
    repository
        .set_enabled("account-1", &workspace_id, false, clock.now())
        .await
        .unwrap();
    clear_outbox(&db).await;
    let (trigger, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let hook = Arc::new(SyncOutboxHook::new(ids, clock, Some(trigger)));
    let bus =
        CommandBus::from_db_with_extensions(db.clone(), CommandBusExtensions::new(vec![hook]))
            .await
            .unwrap();

    bus.workspace_variable_create(
        workspace_id,
        variable(None, "WORKSPACE_PAUSED", "value", false),
    )
    .await
    .unwrap();

    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(pending, 1);
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn four_entity_crud_routes_through_the_transactional_outbox() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;

    bus.rename_workspace(workspace_id.clone(), "Renamed".into())
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &workspace_id).await,
        ("workspace".into(), "upsert".into(), None)
    );
    clear_outbox(&db).await;

    let workspace_variable = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "TOKEN", "super-secret", true),
        )
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &workspace_variable.id).await,
        (
            "workspaceVariable".into(),
            "upsert".into(),
            Some(workspace_id.clone())
        )
    );
    let all_outbox_text: String = sqlx::query_scalar(
        "SELECT group_concat(operation_id || entity_id || coalesce(last_error, '') || coalesce(canonical_payload_json, ''), '') FROM cloud_sync_outbox",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!all_outbox_text.contains("super-secret"));
    clear_outbox(&db).await;
    bus.workspace_variable_update(
        workspace_id.clone(),
        workspace_variable.id.clone(),
        variable(
            Some(workspace_variable.id.clone()),
            "TOKEN",
            "new-secret",
            true,
        ),
    )
    .await
    .unwrap();
    assert_eq!(outbox_row(&db, &workspace_variable.id).await.1, "upsert");
    clear_outbox(&db).await;
    bus.workspace_variable_delete(workspace_id.clone(), workspace_variable.id.clone())
        .await
        .unwrap();
    assert_eq!(outbox_row(&db, &workspace_variable.id).await.1, "delete");
    clear_outbox(&db).await;

    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Development".into())
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &environment.id).await,
        (
            "workspaceEnvironment".into(),
            "upsert".into(),
            Some(workspace_id.clone())
        )
    );
    clear_outbox(&db).await;
    bus.workspace_environment_update_metadata(
        workspace_id.clone(),
        environment.id.clone(),
        "Dev".into(),
        2,
    )
    .await
    .unwrap();
    assert_eq!(outbox_row(&db, &environment.id).await.1, "upsert");
    clear_outbox(&db).await;

    let environment_variable = bus
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            variable(None, "HOST", "localhost", false),
        )
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &environment_variable.id).await,
        (
            "workspaceEnvironmentVariable".into(),
            "upsert".into(),
            Some(environment.id.clone())
        )
    );
    clear_outbox(&db).await;
    bus.workspace_environment_variable_update(
        workspace_id.clone(),
        environment.id.clone(),
        environment_variable.id.clone(),
        variable(
            Some(environment_variable.id.clone()),
            "HOST",
            "127.0.0.1",
            false,
        ),
    )
    .await
    .unwrap();
    assert_eq!(outbox_row(&db, &environment_variable.id).await.1, "upsert");
    clear_outbox(&db).await;
    bus.workspace_environment_variable_delete(
        workspace_id.clone(),
        environment.id.clone(),
        environment_variable.id.clone(),
    )
    .await
    .unwrap();
    assert_eq!(outbox_row(&db, &environment_variable.id).await.1, "delete");
    clear_outbox(&db).await;
    bus.workspace_environment_delete(workspace_id, environment.id.clone())
        .await
        .unwrap();
    assert_eq!(outbox_row(&db, &environment.id).await.1, "delete");
}

#[tokio::test]
async fn connection_crud_uses_one_generic_outbox_row_per_aggregate() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    let ssh = bus
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: workspace_id.clone(),
            name: "Device SSH".into(),
            host: "ssh.example.test".into(),
            port: Some(22),
            username: "deploy".into(),
            auth_kind: "private-key".into(),
            key_path: Some(r"C:\Users\alice\.ssh\id_ed25519".into()),
            credential_ref: None,
            secret: None,
        })
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &ssh.id).await,
        ("connection".into(), "upsert".into(), None)
    );
    let first_operation: String = sqlx::query_scalar(
        "SELECT operation_id FROM cloud_sync_outbox WHERE entity_type = 'connection' AND entity_id = ?1",
    )
    .bind(&ssh.id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    bus.save_ssh_connection(SshConnectionInput {
        id: Some(ssh.id.clone()),
        workspace_id: workspace_id.clone(),
        name: "Device SSH Updated".into(),
        host: "ssh-updated.example.test".into(),
        port: Some(2222),
        username: "deploy".into(),
        auth_kind: "private-key".into(),
        key_path: Some(r"D:\keys\device-only.pem".into()),
        credential_ref: None,
        secret: None,
    })
    .await
    .unwrap();
    let compacted: (i64, String, Option<String>) = sqlx::query_as(
        r#"SELECT COUNT(*), operation_id, canonical_payload_json
           FROM cloud_sync_outbox
           WHERE entity_type = 'connection' AND entity_id = ?1"#,
    )
    .bind(&ssh.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(compacted.0, 1);
    assert_ne!(compacted.1, first_operation);
    assert!(
        compacted.2.is_none(),
        "payload is materialized from Core before push"
    );

    bus.delete_ssh_connection(workspace_id.clone(), ssh.id.clone())
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &ssh.id).await,
        ("connection".into(), "delete".into(), None)
    );

    let database = bus
        .save_database_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: workspace_id.clone(),
            name: "Device SQLite".into(),
            driver: "sqlite".into(),
            host: None,
            port: None,
            database: None,
            username: None,
            ssl_mode: None,
            sqlite_path: Some(r"C:\data\device-only.sqlite".into()),
            credential_ref: None,
            read_only: true,
        })
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &database.id).await,
        ("connection".into(), "upsert".into(), None)
    );
    bus.delete_database_connection(workspace_id, database.id.clone())
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &database.id).await,
        ("connection".into(), "delete".into(), None)
    );

    let stored: String = sqlx::query_scalar(
        "SELECT group_concat(coalesce(canonical_payload_json, ''), '') FROM cloud_sync_outbox",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    for forbidden in ["id_ed25519", "device-only.pem", "device-only.sqlite"] {
        assert!(!stored.contains(forbidden), "leaked {forbidden}");
    }
}

#[tokio::test]
async fn api_entities_share_the_transactional_outbox_without_raw_request_payloads() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &collection.id).await,
        (
            "apiCollection".into(),
            "upsert".into(),
            Some(workspace_id.clone())
        )
    );
    clear_outbox(&db).await;

    let folder = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Root".into(),
        )
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &folder.id).await,
        (
            "apiFolder".into(),
            "upsert".into(),
            Some(collection.id.clone())
        )
    );
    clear_outbox(&db).await;

    let request = bus
        .save_api_request(api_request(
            &workspace_id,
            &collection.id,
            Some(folder.id.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(
        outbox_row(&db, &request.id).await,
        (
            "apiRequest".into(),
            "upsert".into(),
            Some(folder.id.clone())
        )
    );
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT canonical_payload_json FROM cloud_sync_outbox WHERE entity_id = ?1",
    )
    .bind(&request.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(
        stored.is_none(),
        "API payload materialization must wait for Core DomainSnapshot"
    );
    let all_outbox_text: String = sqlx::query_scalar(
        "SELECT group_concat(operation_id || entity_id || coalesce(canonical_payload_json, ''), '') FROM cloud_sync_outbox",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    for secret in [
        "raw-auth-secret",
        "raw-header-secret",
        "raw-query-secret",
        "raw-url-secret",
        "raw-body-secret",
        "raw-runtime-secret",
    ] {
        assert!(!all_outbox_text.contains(secret), "outbox leaked {secret}");
    }

    clear_outbox(&db).await;
    bus.api_collection_folder_delete(workspace_id.clone(), folder.id.clone())
        .await
        .unwrap();
    let deletes: Vec<(String, String)> =
        sqlx::query_as("SELECT entity_type, operation FROM cloud_sync_outbox ORDER BY entity_type")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        deletes,
        vec![
            ("apiFolder".into(), "delete".into()),
            ("apiRequest".into(), "delete".into())
        ],
        "Pro must enqueue exactly Core's cascade mutations"
    );

    let remaining_folder = bus
        .api_collection_folder_create(
            workspace_id.clone(),
            collection.id.clone(),
            None,
            "Remaining".into(),
        )
        .await
        .unwrap();
    bus.save_api_request(api_request(
        &workspace_id,
        &collection.id,
        Some(remaining_folder.id),
    ))
    .await
    .unwrap();
    clear_outbox(&db).await;
    bus.api_collection_delete(workspace_id.clone(), collection.id.clone())
        .await
        .unwrap();
    let collection_deletes: Vec<(String, String)> =
        sqlx::query_as("SELECT entity_type, operation FROM cloud_sync_outbox ORDER BY entity_type")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        collection_deletes,
        vec![
            ("apiCollection".into(), "delete".into()),
            ("apiFolder".into(), "delete".into()),
            ("apiRequest".into(), "delete".into())
        ],
        "Pro must enqueue exactly Core's collection cascade mutations"
    );

    clear_outbox(&db).await;
    bus.apply_external_page(ExternalApplyPage {
        api_collections: vec![ExternalApiCollectionApply::Upsert(
            ExternalApiCollectionUpsert {
                id: "remote-collection".into(),
                workspace_id: workspace_id.clone(),
                name: "Remote".into(),
                description: None,
                created_at: "2026-08-13T00:00:00Z".into(),
                updated_at: "2026-08-13T00:00:00Z".into(),
            },
        )],
        ..ExternalApplyPage::default()
    })
    .await
    .unwrap();
    let echoed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(echoed, 0, "external API apply must not echo into outbox");
}

#[tokio::test]
async fn external_apply_has_no_echo_and_preserves_local_secret_material() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    let secret = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "TOKEN", "device-value", true),
        )
        .await
        .unwrap();
    clear_outbox(&db).await;
    bus.apply_external_page(ExternalApplyPage {
        workspace_variables: vec![ExternalWorkspaceVariableApply::Upsert(
            ExternalWorkspaceVariableUpsert {
                id: secret.id.clone(),
                workspace_id,
                key: "TOKEN".into(),
                value: ExternalVariableValue::PreserveLocal,
                is_secret: true,
                is_enabled: false,
                description: Some("remote metadata".into()),
                sort_order: 0,
                created_at: secret.created_at,
                updated_at: "2026-07-27T12:01:00Z".into(),
            },
        )],
        ..ExternalApplyPage::default()
    })
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let value: String = sqlx::query_scalar("SELECT value FROM workspace_variables WHERE id = ?1")
        .bind(secret.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(value, "device-value");
}

#[tokio::test]
async fn ssh_task_mutations_use_the_generic_outbox_and_delete_children_first() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    let created = bus
        .save_ssh_task(ssh_task_input(&workspace_id))
        .await
        .unwrap();

    let created_rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"SELECT entity_type, operation, parent_entity_id, canonical_payload_json
           FROM cloud_sync_outbox WHERE entity_type IN ('sshTask', 'sshTaskStep')
           ORDER BY entity_type, entity_id"#,
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(created_rows.len(), 3);
    assert_eq!(
        created_rows.iter().filter(|row| row.0 == "sshTask").count(),
        1
    );
    assert!(created_rows.iter().all(|row| row.1 == "upsert"));
    assert!(created_rows.iter().all(|row| row.3.is_none()));
    assert!(created_rows
        .iter()
        .filter(|row| row.0 == "sshTaskStep")
        .all(|row| row.2.as_deref() == Some(created.task.id.as_str())));

    let due = SyncRepository::new(db.pool().clone())
        .due_outbox(
            "account-1",
            "cloud-1",
            "2026-07-27T12:00:01Z".parse().unwrap(),
            10,
        )
        .await
        .unwrap();
    assert_eq!(due[0].entity_type, "sshTask");
    assert!(due[1..]
        .iter()
        .all(|entry| entry.entity_type == "sshTaskStep"));

    clear_outbox(&db).await;
    let removed_step_id = created.steps[0].id.clone();
    let mut remaining = ssh_command_step(Some(created.steps[1].id.clone()), "Reload", 0);
    remaining.enabled = false;
    bus.save_ssh_task(SshTaskSaveInput {
        id: Some(created.task.id.clone()),
        workspace_id: workspace_id.clone(),
        name: "Deploy v2".into(),
        description: "Updated".into(),
        default_connection_id: None,
        steps: vec![remaining],
    })
    .await
    .unwrap();
    assert_eq!(outbox_row(&db, &created.task.id).await.1, "upsert");
    assert_eq!(outbox_row(&db, &removed_step_id).await.1, "delete");
    assert_eq!(outbox_row(&db, &created.steps[1].id).await.1, "upsert");

    clear_outbox(&db).await;
    bus.delete_ssh_task(workspace_id.clone(), created.task.id.clone())
        .await
        .unwrap();
    let deletes = SyncRepository::new(db.pool().clone())
        .due_outbox(
            "account-1",
            "cloud-1",
            "2026-07-27T12:00:01Z".parse().unwrap(),
            10,
        )
        .await
        .unwrap();
    assert_eq!(deletes.len(), 2);
    assert_eq!(deletes[0].entity_type, "sshTaskStep");
    assert_eq!(deletes[1].entity_type, "sshTask");
    assert!(deletes.iter().all(|entry| entry.operation == "delete"));
}

#[tokio::test]
async fn initial_ssh_snapshot_failure_rolls_back_binding_and_outbox() {
    let db = database().await;
    let seed = CommandBus::from_db(db.clone()).await.unwrap();
    let workspace_id = seed.list_workspaces().await.unwrap().active_workspace_id;
    let created = seed
        .save_ssh_task(ssh_task_input(&workspace_id))
        .await
        .unwrap();
    sqlx::query("UPDATE ssh_task_step SET config_json = '{' WHERE id = ?1")
        .bind(&created.steps[0].id)
        .execute(db.pool())
        .await
        .unwrap();

    let repository = SyncRepository::new(db.pool().clone());
    let ssh = SshService::new(db.clone(), SecretStore::in_memory("initial-sync-rollback"));
    let result = repository
        .create_binding_with_initial_outbox_and_domain_entities(
            "account-1",
            0,
            &workspace_id,
            "cloud-1",
            0,
            None,
            Some(&ssh),
            &SequenceIds::default(),
            &FixedClock,
        )
        .await;

    assert!(matches!(result, Err(unfour_cloud_sync::SyncError::Core)));
    let binding_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_workspace_bindings")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let outbox_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(binding_count, 0);
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn external_ssh_task_apply_has_no_echo_or_device_local_side_effects() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    bus.apply_external_page(ExternalApplyPage {
        ssh_tasks: vec![ExternalSshTaskApply::Upsert(ExternalSshTaskUpsert {
            id: "remote-task".into(),
            workspace_id: workspace_id.clone(),
            name: "Remote task".into(),
            description: "Restored without a binding".into(),
            sort_order: 0,
            created_at: "2026-08-17T00:00:00Z".into(),
            updated_at: "2026-08-17T00:00:00Z".into(),
        })],
        ssh_task_steps: vec![ExternalSshTaskStepApply::Upsert(
            ExternalSshTaskStepUpsert {
                id: "remote-step".into(),
                workspace_id: workspace_id.clone(),
                task_id: "remote-task".into(),
                name: "Restart".into(),
                step_type: "command".into(),
                position: 0,
                enabled: true,
                config_version: 1,
                config_json: serde_json::json!({
                    "command": "systemctl restart app",
                    "workingDirectory": "",
                    "timeoutSeconds": 30,
                    "continueOnError": false
                }),
                created_at: "2026-08-17T00:00:00Z".into(),
                updated_at: "2026-08-17T00:00:00Z".into(),
            },
        )],
        ..ExternalApplyPage::default()
    })
    .await
    .unwrap();

    let outbox: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let bindings: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_local_binding WHERE workspace_id = ?1")
            .bind(&workspace_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1")
        .bind(workspace_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!((outbox, bindings, runs), (0, 0, 0));
}

struct RejectingHook;

impl TransactionalCommandHook for RejectingHook {
    fn on_mutations<'a>(
        &'a self,
        _connection: &'a mut SqliteConnection,
        _context: &'a CommandContext,
        _mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async { Err(AppError::Config("reject after outbox".into())) })
    }
}

#[tokio::test]
async fn later_hook_failure_rolls_back_business_data_and_outbox() {
    let (bus, db, workspace_id) = hooked_bus(vec![Arc::new(RejectingHook)]).await;
    bus.workspace_variable_create(
        workspace_id.clone(),
        variable(None, "ROLLBACK", "value", false),
    )
    .await
    .expect_err("reject command");
    let business: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_variables WHERE workspace_id = ?1 AND key = 'ROLLBACK'",
    )
    .bind(workspace_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let outbox: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cloud_sync_outbox")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!((business, outbox), (0, 0));
}

#[tokio::test]
async fn in_flight_then_local_edit_keeps_one_latest_head_and_old_failure_cannot_clobber_it() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    let created = bus
        .workspace_variable_create(
            workspace_id.clone(),
            variable(None, "COLLISION", "first", false),
        )
        .await
        .unwrap();
    let repository = SyncRepository::new(db.pool().clone());
    let now: DateTime<Utc> = "2026-07-27T12:00:00Z".parse().unwrap();
    let entries = repository
        .due_outbox("account-1", "cloud-1", now, 10)
        .await
        .unwrap();
    let old_operation = entries[0].operation_id.clone();
    repository
        .mark_in_flight(&entries, "worker-1", now)
        .await
        .unwrap();

    bus.workspace_variable_update(
        workspace_id.clone(),
        created.id.clone(),
        variable(Some(created.id.clone()), "COLLISION", "second", false),
    )
    .await
    .unwrap();

    let head: (String, String, String) = sqlx::query_as(
        "SELECT operation_id, status, canonical_payload_json FROM cloud_sync_outbox WHERE account_id = 'account-1' AND entity_id = ?1",
    )
    .bind(&created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_ne!(head.0, old_operation);
    assert_eq!(head.1, "pending");
    assert!(head.2.contains("second"));
    assert!(!head.2.contains("first"));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_sync_outbox WHERE account_id = 'account-1' AND entity_id = ?1",
    )
    .bind(&created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);

    repository
        .mark_not_sent(&entries, "explicit_failure", true, now)
        .await
        .unwrap();
    let after_failure: (String, String) = sqlx::query_as(
        "SELECT operation_id, status FROM cloud_sync_outbox WHERE account_id = 'account-1' AND entity_id = ?1",
    )
    .bind(created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(after_failure, (head.0, "pending".into()));
}

#[tokio::test]
async fn expired_in_flight_lease_is_recovered_with_the_same_idempotency_key() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    let created = bus
        .workspace_variable_create(workspace_id, variable(None, "LEASE", "value", false))
        .await
        .unwrap();
    let repository = SyncRepository::new(db.pool().clone());
    let started: DateTime<Utc> = "2026-07-27T12:00:00Z".parse().unwrap();
    let entries = repository
        .due_outbox("account-1", "cloud-1", started, 10)
        .await
        .unwrap();
    let operation_id = entries[0].operation_id.clone();
    repository
        .mark_in_flight(&entries, "crashed-worker", started)
        .await
        .unwrap();
    let recovered_at: DateTime<Utc> = "2026-07-27T12:01:00Z".parse().unwrap();
    repository
        .recover_expired_leases("account-1", recovered_at)
        .await
        .unwrap();
    let recovered: (String, String, Option<String>) = sqlx::query_as(
        "SELECT operation_id, status, lease_owner FROM cloud_sync_outbox WHERE entity_id = ?1",
    )
    .bind(created.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(recovered, (operation_id, "pending".into(), None));
    let attempt: String =
        sqlx::query_scalar("SELECT status FROM cloud_sync_attempts WHERE operation_id = ?1")
            .bind(entries[0].operation_id.clone())
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(attempt, "uncertain");
}

#[tokio::test]
async fn cascading_deletes_are_due_in_reverse_dependency_order() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Delete Me".into())
        .await
        .unwrap();
    bus.workspace_environment_variable_create(
        workspace_id.clone(),
        environment.id.clone(),
        variable(None, "CHILD", "value", false),
    )
    .await
    .unwrap();
    clear_outbox(&db).await;
    bus.workspace_environment_delete(workspace_id, environment.id)
        .await
        .unwrap();
    let due = SyncRepository::new(db.pool().clone())
        .due_outbox(
            "account-1",
            "cloud-1",
            "2026-07-27T12:00:01Z".parse().unwrap(),
            10,
        )
        .await
        .unwrap();
    assert_eq!(due.len(), 2);
    assert_eq!(due[0].entity_type, "workspaceEnvironmentVariable");
    assert_eq!(due[1].entity_type, "workspaceEnvironment");
}

#[tokio::test]
async fn workspace_delete_enqueues_descendants_before_workspace() {
    let (bus, db, workspace_id) = hooked_bus(Vec::new()).await;
    bus.create_workspace("Keep".into()).await.unwrap();
    let collection = bus
        .api_collection_create(workspace_id.clone(), "Accounts".into())
        .await
        .unwrap();
    let request = bus
        .save_api_request(api_request(&workspace_id, &collection.id, None))
        .await
        .unwrap();
    let environment = bus
        .workspace_environment_create(workspace_id.clone(), "Delete Me".into())
        .await
        .unwrap();
    let env_var = bus
        .workspace_environment_variable_create(
            workspace_id.clone(),
            environment.id.clone(),
            variable(None, "CHILD", "value", false),
        )
        .await
        .unwrap();
    clear_outbox(&db).await;
    bus.delete_workspace(workspace_id.clone()).await.unwrap();
    let due = SyncRepository::new(db.pool().clone())
        .due_outbox(
            "account-1",
            "cloud-1",
            "2026-07-27T12:00:01Z".parse().unwrap(),
            20,
        )
        .await
        .unwrap();
    let types: Vec<&str> = due.iter().map(|entry| entry.entity_type.as_str()).collect();
    assert_eq!(due[0].entity_type, "apiRequest");
    assert_eq!(due[0].entity_id, request.id);
    assert_eq!(due.last().unwrap().entity_type, "workspace");
    assert_eq!(due.last().unwrap().entity_id, workspace_id);
    let env_var_pos = types
        .iter()
        .position(|entity_type| *entity_type == "workspaceEnvironmentVariable")
        .expect("environment variable delete");
    let environment_pos = types
        .iter()
        .position(|entity_type| *entity_type == "workspaceEnvironment")
        .expect("environment delete");
    let collection_pos = types
        .iter()
        .position(|entity_type| *entity_type == "apiCollection")
        .expect("collection delete");
    assert_eq!(due[env_var_pos].entity_id, env_var.id);
    assert!(env_var_pos < environment_pos);
    assert!(env_var_pos < due.len() - 1);
    assert!(collection_pos < due.len() - 1);
    assert!(due.iter().all(|entry| entry.operation == "delete"));
}
