use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use sqlx::SqliteConnection;
use unfour_command_bus::{
    CommandBusExtensions, ConnectionType, ReadCommand, ReadCommandResult, TransactionalCommandHook,
};
use unfour_core::domain::{CommandContext, DomainMutation};

use super::{CommandBusAdapter, CommandBusAdapterError, LocalCommandBusAdapter};
use unfour_command_bus::CommandBus;
use unfour_core::models::{KeyValue, SshConnectionInput, WorkspaceVariableInput};
use unfour_core::AppError;
use unfour_local_storage::LocalDb;

struct EnvironmentSqlHook {
    fail_on: Option<&'static str>,
}

impl TransactionalCommandHook for EnvironmentSqlHook {
    fn on_mutations<'a>(
        &'a self,
        connection: &'a mut SqliteConnection,
        context: &'a CommandContext,
        mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            for mutation in mutations {
                sqlx::query("INSERT INTO hook_effects (command_name, entity_id) VALUES (?1, ?2)")
                    .bind(&context.command_name)
                    .bind(&mutation.entity.entity_id)
                    .execute(&mut *connection)
                    .await?;
            }
            if self.fail_on == Some(context.command_name.as_str()) {
                return Err(AppError::Config("test hook rejected mutation".to_string()));
            }
            Ok(())
        })
    }
}

fn test_storage_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "unfour-mcp-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn initialize_storage_dir(storage_dir: &Path) -> String {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build setup runtime");
    runtime.block_on(async {
        let db = LocalDb::connect_path(
            storage_dir.join(unfour_command_bus::DEFAULT_DATABASE_FILE),
        )
        .await
        .expect("create test database");
        db.migrate().await.expect("run migrations");
        let bus = CommandBus::from_db(db.clone()).await.expect("seed workspace");
        sqlx::query(
            "CREATE TABLE hook_effects (id INTEGER PRIMARY KEY, command_name TEXT NOT NULL, entity_id TEXT NOT NULL)",
        )
        .execute(db.pool())
        .await
        .expect("create hook effects table");
        bus.list_workspaces().await.unwrap().active_workspace_id
    })
}

fn read_counts(storage_dir: &Path) -> (i64, i64) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build query runtime");
    runtime.block_on(async {
        let db = LocalDb::connect_path(storage_dir.join(unfour_command_bus::DEFAULT_DATABASE_FILE))
            .await
            .expect("open test database");
        let environments = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workspace_environments WHERE deleted_at IS NULL",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let hook_effects = sqlx::query_scalar("SELECT COUNT(*) FROM hook_effects")
            .fetch_one(db.pool())
            .await
            .unwrap();
        (environments, hook_effects)
    })
}

#[test]
fn ephemeral_adapter_executes_real_command_bus_reads() {
    let adapter = LocalCommandBusAdapter::ephemeral().expect("create adapter");

    let workspace = adapter
        .execute_read(ReadCommand::CurrentWorkspace)
        .expect("read current workspace");
    let ReadCommandResult::CurrentWorkspace(workspace) = workspace else {
        panic!("expected current workspace result");
    };
    assert!(!workspace.workspace_id.is_empty());
    assert_eq!(workspace.source, "command-bus");

    let connections = adapter
        .execute_read(ReadCommand::ListConnections {
            connection_type: ConnectionType::All,
        })
        .expect("list connections");
    let ReadCommandResult::Connections(connections) = connections else {
        panic!("expected connection list result");
    };
    assert_eq!(connections.count, 0);
    assert_eq!(connections.source, "command-bus");

    // Database connections should also be listable through the adapter.
    let db_connections = adapter
        .list_db_connections(&workspace.workspace_id)
        .expect("list db connections");
    assert_eq!(db_connections.len(), 0);

    // System health should be readable through the real adapter.
    let health = adapter.system_health().expect("system health");
    assert!(health.command_bus_ready);
    assert!(health.storage_ready);
}

#[test]
fn unified_ephemeral_adapter_installs_cloud_sync_schema_and_hook() {
    let adapter = LocalCommandBusAdapter::from_storage_mode(crate::StorageMode::Ephemeral)
        .expect("create unified ephemeral adapter");
    let workspace = adapter
        .execute_read(ReadCommand::CurrentWorkspace)
        .expect("read current workspace");
    let ReadCommandResult::CurrentWorkspace(workspace) = workspace else {
        panic!("expected current workspace result");
    };

    // A local mutation can commit only if the Cloud Sync migration exists and
    // SyncOutboxHook can enqueue its outbox row in the same transaction.
    let created = adapter
        .create_workspace_variable(
            &workspace.workspace_id,
            WorkspaceVariableInput {
                id: None,
                key: "UNIFIED_MCP_HOOK".to_string(),
                value: "local".to_string(),
                is_secret: false,
                is_enabled: true,
                description: None,
                sort_order: 0,
            },
        )
        .expect("create variable through unified hook");
    assert_eq!(created.key, "UNIFIED_MCP_HOOK");
    adapter.shutdown();
}

#[test]
fn adapter_shutdown_is_idempotent() {
    let adapter = LocalCommandBusAdapter::ephemeral().expect("create adapter");

    adapter.shutdown();
    adapter.shutdown();
}

#[test]
fn ephemeral_adapter_executes_environment_crud() {
    let adapter = LocalCommandBusAdapter::ephemeral().expect("create adapter");
    let workspace = adapter
        .execute_read(ReadCommand::CurrentWorkspace)
        .expect("read current workspace");
    let ReadCommandResult::CurrentWorkspace(workspace) = workspace else {
        panic!("expected current workspace result");
    };

    let created = adapter
        .create_api_environment(&workspace.workspace_id, "QA")
        .expect("create environment");
    assert_eq!(created.name, "QA");
    assert!(created.variables.is_empty());
    let activity = adapter
        .execute_read(ReadCommand::ListActivity {
            workspace_id: Some(workspace.workspace_id.clone()),
            limit: Some(10),
        })
        .expect("read environment activity through adapter");
    let ReadCommandResult::Activity(activity) = activity else {
        panic!("expected activity result");
    };
    assert!(activity
        .activity
        .iter()
        .any(|item| item.action == "workspace.environment.create"));

    let updated = adapter
        .update_api_environment(
            &workspace.workspace_id,
            &created.id,
            "Staging",
            vec![KeyValue {
                key: "baseUrl".to_string(),
                value: "https://staging.example.test".to_string(),
                enabled: true,
            }],
        )
        .expect("update environment");
    assert_eq!(updated.name, "Staging");
    assert_eq!(updated.variables.len(), 1);

    let remaining = adapter
        .delete_api_environment(&workspace.workspace_id, &created.id)
        .expect("delete environment");
    assert!(remaining.is_empty());
}

#[test]
fn adapter_extensions_observe_environment_create_update_and_delete() {
    let storage_dir = test_storage_dir("hook-crud");
    let workspace_id = initialize_storage_dir(&storage_dir);
    let adapter = LocalCommandBusAdapter::from_storage_dir_with_extensions(
        &storage_dir,
        CommandBusExtensions::new(vec![Arc::new(EnvironmentSqlHook { fail_on: None })]),
    )
    .expect("create hooked adapter");

    let created = adapter
        .create_api_environment(&workspace_id, "Hooked")
        .expect("create environment");
    adapter
        .update_api_environment(&workspace_id, &created.id, "Updated", Vec::new())
        .expect("update environment");
    adapter
        .delete_api_environment(&workspace_id, &created.id)
        .expect("delete environment");
    adapter.shutdown();
    drop(adapter);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let commands: Vec<(String,)> = runtime.block_on(async {
        let db = LocalDb::connect_path(storage_dir.join(unfour_command_bus::DEFAULT_DATABASE_FILE))
            .await
            .unwrap();
        sqlx::query_as("SELECT DISTINCT command_name FROM hook_effects ORDER BY command_name")
            .fetch_all(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(
        commands,
        vec![
            ("workspace.environment.create".to_string(),),
            ("workspace.environment.delete".to_string(),),
            ("workspace.environment.update".to_string(),),
        ]
    );
    drop(runtime);
    std::fs::remove_dir_all(storage_dir).unwrap();
}

#[test]
fn adapter_hook_failure_rolls_back_environment_and_hook_sql() {
    let storage_dir = test_storage_dir("hook-rollback");
    let workspace_id = initialize_storage_dir(&storage_dir);
    let adapter = LocalCommandBusAdapter::from_storage_dir_with_extensions(
        &storage_dir,
        CommandBusExtensions::new(vec![Arc::new(EnvironmentSqlHook {
            fail_on: Some("workspace.environment.create"),
        })]),
    )
    .expect("create rejecting adapter");

    adapter
        .create_api_environment(&workspace_id, "Must Roll Back")
        .expect_err("hook should reject create");
    adapter.shutdown();
    drop(adapter);
    assert_eq!(read_counts(&storage_dir), (0, 0));
    std::fs::remove_dir_all(storage_dir).unwrap();
}

#[test]
fn community_storage_adapter_keeps_empty_extensions_behavior() {
    let storage_dir = test_storage_dir("community-default");
    let workspace_id = initialize_storage_dir(&storage_dir);
    let adapter =
        LocalCommandBusAdapter::from_storage_dir(&storage_dir).expect("create community adapter");
    adapter
        .create_api_environment(&workspace_id, "Community")
        .expect("create without hooks");
    adapter.shutdown();
    drop(adapter);
    assert_eq!(read_counts(&storage_dir), (1, 0));
    std::fs::remove_dir_all(storage_dir).unwrap();
}

#[test]
fn storage_dir_adapter_reads_persisted_connection_metadata() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build test runtime");
    let storage_dir = std::env::temp_dir().join(format!(
        "unfour-mcp-storage-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    runtime.block_on(async {
        let db_path = storage_dir.join(unfour_command_bus::DEFAULT_DATABASE_FILE);
        let db = LocalDb::connect_path(&db_path).await.expect("create db");
        db.migrate().await.expect("run migrations");
        let bus = CommandBus::from_db(db).await.expect("create bus");
        let state = bus.list_workspaces().await.expect("list workspaces");
        bus.save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: state.active_workspace_id,
            name: "Manual SSH".to_string(),
            host: "ssh.example.test".to_string(),
            port: Some(22),
            username: "developer".to_string(),
            auth_kind: "password".to_string(),
            key_path: None,
            credential_ref: Some("ssh-secret".to_string()),
            secret: None,
        })
        .await
        .expect("save ssh connection");
    });

    let adapter =
        LocalCommandBusAdapter::from_storage_dir_read_only(&storage_dir).expect("open storage");
    let result = adapter
        .execute_read(ReadCommand::ListConnections {
            connection_type: ConnectionType::Ssh,
        })
        .expect("list ssh connections");
    let ReadCommandResult::Connections(result) = result else {
        panic!("expected connections");
    };

    assert_eq!(result.count, 1);
    assert_eq!(result.connections[0].name, "Manual SSH");
    assert_eq!(
        result.connections[0].safe_summary.host.as_deref(),
        Some("ssh.example.test")
    );
    let json = serde_json::to_string(&result).expect("serialize result");
    assert!(!json.contains("developer"));
    assert!(!json.contains("ssh-secret"));

    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn default_storage_database_path_matches_command_bus_default() {
    assert_eq!(
        LocalCommandBusAdapter::default_database_path().expect("adapter database path"),
        unfour_command_bus::default_database_path().expect("command bus database path")
    );
}

#[test]
fn ssh_validation_error_mentions_control_characters() {
    assert_eq!(
        CommandBusAdapterError::from_ssh_app_error(
            "The command-bus SSH command failed.",
            &AppError::Validation("ssh command cannot contain control characters".to_string()),
        )
        .message,
        "SSH command validation failed: control characters/newlines are not allowed."
    );
}
