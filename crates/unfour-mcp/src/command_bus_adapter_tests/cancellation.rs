use super::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn stdio_cancels_unlimited_http_at_execution_layer_and_remains_responsive() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let http = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/stall", http.local_addr().unwrap());
    let (received_tx, received_rx) = mpsc::channel();
    let (closed_tx, closed_rx) = mpsc::channel();
    let http_thread = std::thread::spawn(move || {
        let (mut socket, _) = http.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(socket.try_clone().unwrap());
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        received_tx.send(()).unwrap();
        let mut byte = [0];
        closed_tx
            .send(socket.read(&mut byte).unwrap() == 0)
            .unwrap();
    });
    let wire = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client = TcpStream::connect(wire.local_addr().unwrap()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let (server_socket, _) = wire.accept().unwrap();
    let server = Arc::new(crate::McpServer::new(adapter.clone()));
    let transport = std::thread::spawn(move || {
        crate::stdio_transport::run(
            server,
            BufReader::new(server_socket.try_clone().unwrap()),
            &mut &server_socket,
            None,
        )
        .unwrap()
    });
    writeln!(client,"{}",json!({"jsonrpc":"2.0","id":"slow","method":"tools/call","params":{"name":"unfour.api.send_request","arguments":{"method":"GET","url":url,"timeoutMs":0}}})).unwrap();
    received_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    writeln!(client,"{}",json!({"jsonrpc":"2.0","id":"queued","method":"tools/call","params":{"name":"unfour.workspace.create_variable","arguments":{"key":"CANCELLED_CALL","value":"canary"}}})).unwrap();
    writeln!(
        client,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"queued"}})
    )
    .unwrap();
    // A tools/call notification must never perform a mutation.
    writeln!(client,"{}",json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"unfour.workspace.create_variable","arguments":{"key":"NOTIFICATION_CALL","value":"canary"}}})).unwrap();
    writeln!(
        client,
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"ping"})
    )
    .unwrap();
    writeln!(
        client,
        "{}",
        json!({"jsonrpc":"2.0","id":3,"method":"tools/list"})
    )
    .unwrap();
    // Unknown cancellation does not cancel the active call.
    writeln!(
        client,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"unknown"}})
    )
    .unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    for id in [2, 3] {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let result: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(result["id"], id);
        assert!(result.get("result").is_some());
    }
    assert!(closed_rx.try_recv().is_err());
    writeln!(
        client,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"slow"}})
    )
    .unwrap();
    assert!(
        closed_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        "cancellation must close the HTTP execution, not only suppress its response"
    );
    writeln!(client,"{}",json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"unfour.system.health","arguments":{}}})).unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let result: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(result["id"], 4);
    assert_eq!(result["result"]["isError"], false);
    let ReadCommandResult::CurrentWorkspace(workspace) =
        adapter.execute_read(ReadCommand::CurrentWorkspace).unwrap()
    else {
        panic!("workspace result");
    };
    let variables = adapter
        .list_workspace_variables(&workspace.workspace_id)
        .unwrap();
    assert!(variables
        .iter()
        .all(|v| v.key != "CANCELLED_CALL" && v.key != "NOTIFICATION_CALL"));
    client.shutdown(Shutdown::Write).unwrap();
    transport.join().unwrap();
    http_thread.join().unwrap();
    adapter.shutdown();
}
