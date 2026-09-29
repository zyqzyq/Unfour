use std::collections::HashSet;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use jsonschema::validator_for;
use serde_json::{json, Value};

#[test]
fn binary_stdio_contract_matches_advertised_schemas_and_exits_on_eof() {
    let storage_dir = std::env::temp_dir().join(format!(
        "unfour-mcp-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos()
    ));
    let requests = [
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "contract-test", "version": "0.1.0" }
            }
        }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "unfour.system.health", "arguments": {}
        } }),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
            "name": "unfour.workspace.current", "arguments": {}
        } }),
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {
            "name": "unfour.workspace.list", "arguments": {}
        } }),
        json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {
            "name": "unfour.system.health", "arguments": { "unexpected": true }
        } }),
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {
            "name": "unfour.no_such_tool", "arguments": {}
        } }),
        json!({ "jsonrpc": "2.0", "id": 8, "method": "initialize", "params": {} }),
        json!({ "jsonrpc": "2.0", "id": 9, "method": "no_such_method" }),
    ];

    let mut child = Command::new(env!("CARGO_BIN_EXE_unfour-mcp"))
        .env("UNFOUR_MCP_STORAGE_MODE", "ephemeral")
        .env("UNFOUR_DATA_DIR", &storage_dir)
        .env_remove("UNFOUR_STORAGE_PROFILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the real MCP binary");
    {
        let stdin = child.stdin.as_mut().expect("piped stdin");
        for request in requests {
            writeln!(stdin, "{request}").expect("write a JSON-RPC request");
        }
    }
    drop(child.stdin.take());
    // Drain both pipes while waiting so a large tools/list response cannot
    // block the child before it observes EOF.
    let mut stdout_pipe = child.stdout.take().expect("piped stdout");
    let mut stderr_pipe = child.stderr.take().expect("piped stderr");
    let stdout_reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        stdout_pipe
            .read_to_end(&mut output)
            .expect("read MCP stdout");
        output
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        stderr_pipe
            .read_to_end(&mut output)
            .expect("read MCP stderr");
        output
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll MCP process") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill MCP process after EOF timeout");
            child.wait().expect("reap timed out MCP process");
            panic!("MCP did not exit within 30 seconds after stdin EOF");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = String::from_utf8(stdout_reader.join().expect("join MCP stdout reader"))
        .expect("MCP stdout is UTF-8");
    let stderr = String::from_utf8_lossy(&stderr_reader.join().expect("join MCP stderr reader"))
        .into_owned();
    assert!(
        status.success(),
        "MCP must exit zero after EOF; stderr: {stderr}\nstdout: {stdout}"
    );
    assert!(!storage_dir.join("unfour.sqlite").exists());
    let responses: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("stdout contains only JSON-RPC messages"))
        .collect();
    assert_eq!(
        responses.len(),
        9,
        "notifications must not produce responses"
    );
    let response = |id| {
        let matches: Vec<&Value> = responses.iter().filter(|item| item["id"] == id).collect();
        assert_eq!(matches.len(), 1, "expected one response for id {id}");
        matches[0]
    };

    let initialized = response(1);
    assert_eq!(initialized["jsonrpc"], "2.0");
    assert_eq!(initialized["result"]["protocolVersion"], "2025-06-18");
    assert!(initialized["result"]["capabilities"]["tools"].is_object());
    assert_eq!(initialized["result"]["serverInfo"]["name"], "unfour-mcp");

    let tools = response(2)["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array");
    assert!(!tools.is_empty());
    let mut names = HashSet::new();
    for tool in tools {
        let name = tool["name"].as_str().expect("tool name is a string");
        assert!(names.insert(name), "duplicate tool name: {name}");
        for key in ["inputSchema", "outputSchema"] {
            let schema = &tool[key];
            assert_eq!(
                schema["type"], "object",
                "{name} {key} must be an object schema"
            );
            validator_for(schema)
                .unwrap_or_else(|error| panic!("{name} {key} is not a valid JSON Schema: {error}"));
        }
    }

    let health = tools
        .iter()
        .find(|tool| tool["name"] == "unfour.system.health")
        .expect("health tool is advertised");
    let health_input = validator_for(&health["inputSchema"]).expect("health input schema");
    assert!(health_input.is_valid(&json!({})));
    assert!(!health_input.is_valid(&json!({ "unexpected": true })));

    for (id, name) in [
        (3, "unfour.system.health"),
        (4, "unfour.workspace.current"),
        (5, "unfour.workspace.list"),
    ] {
        let result = &response(id)["result"];
        assert_eq!(result["isError"], false, "{name} must succeed: {result}");
        let structured = &result["structuredContent"];
        let text = result["content"][0]["text"]
            .as_str()
            .expect("tool result has text content");
        assert_eq!(serde_json::from_str::<Value>(text).unwrap(), *structured);
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        let validator = validator_for(&tool["outputSchema"]).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(structured)
            .map(|error| error.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "{name} output violates outputSchema: {errors:?}"
        );
    }

    for id in [6, 7, 8] {
        assert_eq!(
            response(id)["error"]["code"],
            -32602,
            "id {id} must reject invalid params"
        );
        assert!(response(id).get("result").is_none());
    }
    assert_eq!(response(9)["error"]["code"], -32601);
}
