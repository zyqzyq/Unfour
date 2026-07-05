use unfour_command_bus::{ConnectionType, ReadCommand, ReadCommandResult};

use super::{CommandBusAdapterError, LocalCommandBusAdapter};
use unfour_command_bus::CommandBus;
use unfour_core::models::SshConnectionInput;
use unfour_core::AppError;
use unfour_local_storage::LocalDb;

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
