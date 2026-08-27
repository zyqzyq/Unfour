#![cfg(unix)]

use std::process::{Child, Command, Stdio};
use std::time::Duration;

fn spawn_ephemeral() -> Child {
    Command::new(env!("CARGO_BIN_EXE_unfour-mcp"))
        .env("UNFOUR_MCP_STORAGE_MODE", "ephemeral")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn unfour-mcp")
}

fn assert_signal_exits_cleanly(signal: &str) {
    let child = spawn_ephemeral();
    std::thread::sleep(Duration::from_millis(250));
    let status = Command::new("kill")
        .args([signal, &child.id().to_string()])
        .status()
        .expect("send process signal");
    assert!(status.success(), "kill command should succeed");
    let output = child.wait_with_output().expect("wait for signalled MCP");
    assert!(
        output.status.success(),
        "signal handler should exit zero: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sigint_ctrl_c_exits_cleanly() {
    assert_signal_exits_cleanly("-INT");
}

#[test]
fn sigterm_exits_cleanly() {
    assert_signal_exits_cleanly("-TERM");
}
