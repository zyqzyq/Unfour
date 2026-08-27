use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;
use unfour_cloud_sync::{SyncDependencies, SyncRepository};
use unfour_command_bus::CommandBus;
use unfour_local_storage::LocalDb;

fn isolated_storage_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "unfour-mcp-registry-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos()
    ))
}

#[test]
fn ephemeral_binary_passes_registry_introspection_without_sqlite() {
    let storage_dir = isolated_storage_dir();
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"registry-check","version":"0.1.0"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n"
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_unfour-mcp"))
        .env("UNFOUR_MCP_STORAGE_MODE", "ephemeral")
        // If the binary accidentally falls back to default storage, this
        // unique, nonexistent path makes the test fail instead of touching a
        // developer's real ~/.unfour database.
        .env("UNFOUR_DATA_DIR", &storage_dir)
        .env_remove("UNFOUR_STORAGE_PROFILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn unfour-mcp binary");

    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())
        .expect("write registry introspection requests");

    let output = child
        .wait_with_output()
        .expect("wait for unfour-mcp binary");
    let stdout = String::from_utf8(output.stdout).expect("MCP stdout should be UTF-8");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "unfour-mcp exited unsuccessfully: {stderr}\nstdout: {stdout}"
    );

    let responses = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("stdout line should be JSON"))
        .collect::<Vec<_>>();

    // The initialized notification has no response, so only initialize and
    // tools/list should produce JSON-RPC responses.
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(responses[1]["id"], 2);
    let tools = responses[1]["result"]["tools"]
        .as_array()
        .expect("tools/list should return an array");
    assert!(!tools.is_empty());
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "unfour.system.health"));

    assert!(
        !storage_dir.join("unfour.sqlite").exists(),
        "ephemeral introspection must not create a SQLite database"
    );
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn normal_binary_uses_unified_storage_enqueues_outbox_and_exits_on_eof() {
    let storage_dir = isolated_storage_dir();
    let database_path = storage_dir.join("unfour.sqlite");
    let setup_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build setup runtime");
    setup_runtime.block_on(async {
        let db = LocalDb::connect_path(&database_path)
            .await
            .expect("create normal storage");
        db.migrate().await.expect("run core migrations");
        let bus = CommandBus::from_db(db.clone())
            .await
            .expect("seed the local workspace");
        let workspace_id = bus
            .list_workspaces()
            .await
            .expect("read seeded workspace")
            .active_workspace_id;
        unfour_cloud_sync_storage::migrate(db.pool())
            .await
            .expect("run unified migration chain");
        let dependencies = SyncDependencies::default();
        let repository = SyncRepository::new(db.pool().clone());
        repository
            .activate_account("mcp-account", 1, dependencies.clock.now())
            .await
            .expect("activate MCP sync account");
        repository
            .create_binding_with_initial_outbox(
                "mcp-account",
                1,
                &workspace_id,
                "mcp-cloud-workspace",
                0,
                dependencies.ids.as_ref(),
                dependencies.clock.as_ref(),
            )
            .await
            .expect("enable MCP workspace sync");
        db.pool().close().await;
    });
    drop(setup_runtime);

    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"normal-storage-check","version":"0.1.0"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"unfour.workspace.create_variable","arguments":{"key":"UNIFIED_MCP_HOOK","value":"local","isEnabled":true}}}"#,
        "\n"
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_unfour-mcp"))
        .env_remove("UNFOUR_MCP_STORAGE_MODE")
        .env("UNFOUR_DATA_DIR", &storage_dir)
        .env_remove("UNFOUR_STORAGE_PROFILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn normal unfour-mcp binary");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())
        .expect("write normal-storage request");
    let output = child.wait_with_output().expect("wait for normal MCP EOF");
    let stdout = String::from_utf8(output.stdout).expect("MCP stdout should be UTF-8");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "normal unfour-mcp exited unsuccessfully: {stderr}\nstdout: {stdout}"
    );
    let responses = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("stdout line should be JSON"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["result"]["isError"], false);

    let query_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build query runtime");
    let outbox_count: i64 = query_runtime.block_on(async {
        let db = LocalDb::connect_existing_path(&database_path)
            .await
            .expect("open migrated normal storage");
        let count = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cloud_sync_outbox WHERE entity_type = 'workspaceVariable'",
        )
        .fetch_one(db.pool())
        .await
        .expect("query Cloud Sync outbox");
        db.pool().close().await;
        count
    });
    assert_eq!(outbox_count, 1);
    drop(query_runtime);
    let _ = std::fs::remove_dir_all(storage_dir);
}
