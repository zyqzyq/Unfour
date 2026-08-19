use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;

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
