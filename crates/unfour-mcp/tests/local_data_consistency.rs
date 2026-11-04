//! Real stdio MCP child process -> CommandBus -> file SQLite -> desktop bus reads.
//! This does not claim native WebView/UI coverage (covered separately with mocks).
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use unfour_command_bus::CommandBus;
use unfour_local_storage::LocalDb;

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<Value>,
    next_id: u64,
}
impl Mcp {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_unfour-mcp"))
            .env("UNFOUR_DATA_DIR", root)
            .env_remove("UNFOUR_MCP_STORAGE_MODE")
            .env_remove("UNFOUR_STORAGE_PROFILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, replies) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str(&line) {
                    if sender.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        let mut mcp = Self {
            child,
            stdin,
            replies,
            next_id: 0,
        };
        mcp.request("initialize", json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"local-consistency-test","version":"1"}}));
        writeln!(
            mcp.stdin,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .unwrap();
        mcp
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        writeln!(
            self.stdin,
            "{}",
            json!({"jsonrpc":"2.0","id":self.next_id,"method":method,"params":params})
        )
        .unwrap();
        self.stdin.flush().unwrap();
        loop {
            let reply = self
                .replies
                .recv_timeout(Duration::from_secs(15))
                .expect("MCP response timeout");
            if reply["id"] == self.next_id {
                assert!(reply.get("error").is_none(), "{reply}");
                return reply["result"].clone();
            }
        }
    }
    fn call(&mut self, name: &str, mut arguments: Value) -> Value {
        let mut result = self.request("tools/call", json!({"name":name,"arguments":arguments}));
        // Exercise the unchanged policy handshake against disposable fixtures.
        if result["isError"] == true {
            let details: Value =
                serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
            if let Some(confirmation) = details["confirmation_text"].as_str() {
                arguments["confirm"] = json!(true);
                arguments["confirmation_text"] = json!(confirmation);
                result = self.request("tools/call", json!({"name":name,"arguments":arguments}));
            }
        }
        assert_eq!(result["isError"], false, "{name}: {result}");
        result["structuredContent"].clone()
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn clock(bus: &CommandBus, workspace: &str, domain: &str) -> String {
    bus.local_data_versions()
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.workspace_id == workspace && item.domain == domain)
        .map(|item| item.version)
        .unwrap_or_default()
}

#[tokio::test]
async fn independent_mcp_commits_refreshable_clocks_for_child_variables_and_other_definitions() {
    let root = std::env::temp_dir().join(format!(
        "unfour-local-consistency-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let db = LocalDb::connect_path(root.join("unfour.sqlite"))
        .await
        .unwrap();
    db.migrate().await.unwrap();
    let desktop = CommandBus::from_db(db.clone()).await.unwrap();
    let ws = desktop.list_workspaces().await.unwrap().active_workspace_id;
    let environment = desktop
        .workspace_environment_create(ws.clone(), "Local".into())
        .await
        .unwrap();
    let mut mcp = Mcp::start(&root);
    let before = clock(&desktop, &ws, "environments").await;
    for value in ["first", "updated"] {
        mcp.call(
            "unfour.api.set_environment_variable",
            json!({"workspaceId":ws,"environmentId":environment.id,"key":"BASE_URL","value":value}),
        );
        let read = desktop
            .workspace_environments_list(ws.clone())
            .await
            .unwrap();
        assert_eq!(read[0].variables[0].value, value);
        assert_eq!(
            read[0].updated_at, environment.updated_at,
            "child mutation must not rely on parent timestamp"
        );
    }
    assert_ne!(clock(&desktop, &ws, "environments").await, before);
    let before = clock(&desktop, &ws, "environments").await;
    mcp.call(
        "unfour.api.delete_environment_variable",
        json!({"workspaceId":ws,"environmentId":environment.id,"key":"BASE_URL"}),
    );
    assert!(desktop
        .workspace_environments_list(ws.clone())
        .await
        .unwrap()[0]
        .variables
        .is_empty());
    assert_ne!(clock(&desktop, &ws, "environments").await, before);

    let collection = mcp.call(
        "unfour.api.create_collection",
        json!({"workspaceId":ws,"name":"MCP collection"}),
    )["collection"]
        .clone();
    let collection_id = collection["id"].as_str().unwrap();
    let before = clock(&desktop, &ws, "api-collections").await;
    mcp.call(
        "unfour.api.update_collection",
        json!({"workspaceId":ws,"collectionId":collection_id,"name":"Renamed"}),
    );
    assert_ne!(clock(&desktop, &ws, "api-collections").await, before);
    let request = mcp.call("unfour.api.create_request", json!({"workspaceId":ws,"collectionId":collection_id,"name":"Request","method":"GET","url":"https://example.test"}));
    let request_id = request["request"]["id"].as_str().unwrap();
    let before = clock(&desktop, &ws, "api-requests").await;
    mcp.call(
        "unfour.api.update_request",
        json!({"workspaceId":ws,"requestId":request_id,"url":"https://updated.test"}),
    );
    assert_ne!(clock(&desktop, &ws, "api-requests").await, before);
    let before = clock(&desktop, &ws, "api-requests").await;
    mcp.call(
        "unfour.api.delete_request",
        json!({"workspaceId":ws,"requestId":request_id}),
    );
    assert_ne!(clock(&desktop, &ws, "api-requests").await, before);
    mcp.call(
        "unfour.api.delete_collection",
        json!({"workspaceId":ws,"collectionId":collection_id}),
    );

    let before = clock(&desktop, &ws, "ssh-connections").await;
    mcp.call("unfour.ssh.create_connection", json!({"workspaceId":ws,"name":"No connection executed","host":"example.test","username":"test","authKind":"none"}));
    assert_ne!(clock(&desktop, &ws, "ssh-connections").await, before);
    let task = mcp.call("unfour.ssh.save_task", json!({"workspaceId":ws,"name":"Task","steps":[{"name":"Check","stepType":"command","position":0,"enabled":true,"configJson":{"command":"echo test"}}]}));
    let task_id = task["task"]["task"]["id"].as_str().unwrap();
    let before = clock(&desktop, &ws, "ssh-tasks").await;
    mcp.call(
        "unfour.ssh.save_task",
        json!({"workspaceId":ws,"taskId":task_id,"name":"Updated task","steps":[]}),
    );
    assert_ne!(clock(&desktop, &ws, "ssh-tasks").await, before);
    let before = clock(&desktop, &ws, "ssh-tasks").await;
    mcp.call(
        "unfour.ssh.delete_task",
        json!({"workspaceId":ws,"taskId":task_id}),
    );
    assert_ne!(clock(&desktop, &ws, "ssh-tasks").await, before);

    let mut flow = mcp.call("unfour.flow.save", json!({"definition":{"workspaceId":ws,"id":"","name":"Flow","revision":0,"inputs":[],"steps":[{"id":"wait","name":"Wait","kind":"wait","durationMs":1,"timeoutMs":1000}]}}))["flow"].clone();
    let before = clock(&desktop, &ws, "flows").await;
    flow["name"] = json!("Updated flow");
    flow = mcp.call("unfour.flow.save", json!({"definition":flow}))["flow"].clone();
    assert_ne!(clock(&desktop, &ws, "flows").await, before);
    let before = clock(&desktop, &ws, "flows").await;
    mcp.call(
        "unfour.flow.delete",
        json!({"workspaceId":ws,"flowId":flow["id"],"expectedRevision":flow["revision"]}),
    );
    assert_ne!(clock(&desktop, &ws, "flows").await, before);
    let executions: i64 = sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM api_history) + (SELECT COUNT(*) FROM ssh_task_run) + (SELECT COUNT(*) FROM flow_runs) + (SELECT COUNT(*) FROM db_query_history)").fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        executions, 0,
        "definition refresh must never execute business actions"
    );
    drop(mcp);
    drop(desktop);
    db.pool().close().await;
    std::fs::remove_dir_all(root).unwrap();
}
