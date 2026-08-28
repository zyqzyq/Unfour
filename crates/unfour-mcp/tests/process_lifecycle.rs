#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const INITIALIZE_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"process-lifecycle","version":"0.1.0"}}}"#;

fn spawn_ephemeral() -> Child {
    Command::new(env!("CARGO_BIN_EXE_unfour-mcp"))
        .env("UNFOUR_MCP_STORAGE_MODE", "ephemeral")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn unfour-mcp")
}

fn assert_signal_exits_cleanly(signal: &str) {
    let mut child = spawn_ephemeral();
    let mut stdin = child.stdin.take().expect("MCP stdin should be piped");
    let stdout = child.stdout.take().expect("MCP stdout should be piped");
    let (ready_sender, ready_receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut response = String::new();
        let result = BufReader::new(stdout)
            .read_line(&mut response)
            .map(|_| response);
        let _ = ready_sender.send(result);
    });

    writeln!(stdin, "{INITIALIZE_REQUEST}").expect("write initialize request");
    stdin.flush().expect("flush initialize request");
    let response = match ready_receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("read MCP initialize response: {error}");
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("MCP did not become ready within 5 seconds: {error}");
        }
    };
    if !response.contains("\"result\"") {
        let _ = child.kill();
        let _ = child.wait();
        panic!("MCP initialize should return a result: {response}");
    }

    let status = Command::new("kill")
        .args([signal, &child.id().to_string()])
        .status()
        .expect("send process signal");
    assert!(status.success(), "kill command should succeed");
    drop(stdin);
    let output = child.wait_with_output().expect("wait for signalled MCP");
    assert!(
        output.status.success(),
        "signal handler should exit zero (status {:?}): {}",
        output.status,
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
