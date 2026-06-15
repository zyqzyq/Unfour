use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::protocol;
use crate::tools::{ToolCallError, ToolRegistry};

pub const SUPPORTED_PROTOCOL_VERSION: &str = "2025-06-18";

pub struct McpServer {
    tools: ToolRegistry,
}

impl Default for McpServer {
    fn default() -> Self {
        Self {
            tools: ToolRegistry::mock(),
        }
    }
}

impl McpServer {
    pub fn handle_line(&self, line: &str) -> Option<String> {
        let response = match serde_json::from_str::<Value>(line) {
            Ok(message) => self.handle_message(&message),
            Err(error) => Some(protocol::error(
                Value::Null,
                -32700,
                format!("Parse error: {error}"),
            )),
        };

        response.map(|value| {
            serde_json::to_string(&value).expect("serializing a JSON-RPC response cannot fail")
        })
    }

    pub fn handle_message(&self, message: &Value) -> Option<Value> {
        let Some(object) = message.as_object() else {
            return Some(protocol::error(Value::Null, -32600, "Invalid Request"));
        };

        let id = object.get("id").cloned();
        let response_id = id.clone().unwrap_or(Value::Null);

        if object.get("jsonrpc").and_then(Value::as_str) != Some(protocol::JSON_RPC_VERSION) {
            return id.map(|_| protocol::error(response_id, -32600, "Invalid Request"));
        }

        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return id.map(|_| protocol::error(response_id, -32600, "Invalid Request"));
        };

        let result = match method {
            "initialize" => self.initialize(object.get("params")),
            "notifications/initialized" => return None,
            "tools/list" => Ok(json!({ "tools": self.tools.definitions() })),
            "tools/call" => self.call_tool(object.get("params")),
            _ => Err((-32601, format!("Method not found: {method}"))),
        };

        id.map(|id| match result {
            Ok(result) => protocol::success(id, result),
            Err((code, message)) => protocol::error(id, code, message),
        })
    }

    fn initialize(&self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        let requested_version = params
            .and_then(|value| value.get("protocolVersion"))
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "Missing protocolVersion".to_string()))?;

        let protocol_version = if requested_version == SUPPORTED_PROTOCOL_VERSION {
            requested_version
        } else {
            SUPPORTED_PROTOCOL_VERSION
        };

        Ok(json!({
            "protocolVersion": protocol_version,
            "capabilities": {
                "tools": {
                    "listChanged": false,
                }
            },
            "serverInfo": {
                "name": "unfour-mcp",
                "title": "Unfour MCP",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": "Mock-only MCP skeleton. No Unfour business capabilities are connected.",
        }))
    }

    fn call_tool(&self, params: Option<&Value>) -> Result<Value, (i64, String)> {
        let params = params
            .and_then(Value::as_object)
            .ok_or_else(|| (-32602, "tools/call params must be an object".to_string()))?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "tools/call requires a tool name".to_string()))?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        self.tools
            .call(name, arguments)
            .map_err(|error| match error {
                ToolCallError::UnknownTool(name) => (-32602, format!("Unknown tool: {name}")),
                ToolCallError::InvalidArguments(message) => (-32602, message),
            })
    }
}

pub fn run_stdio<R, W>(reader: R, mut writer: W) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let server = McpServer::default();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Some(response) = server.handle_line(&line) {
            writeln!(writer, "{response}")?;
            writer.flush()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::{json, Value};

    use super::{run_stdio, McpServer, SUPPORTED_PROTOCOL_VERSION};

    #[test]
    fn initialize_declares_tools_capability() {
        let response = McpServer::default()
            .handle_message(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": SUPPORTED_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "test-client",
                        "version": "0.1.0"
                    }
                }
            }))
            .expect("initialize should return a response");

        assert_eq!(
            response["result"]["protocolVersion"],
            SUPPORTED_PROTOCOL_VERSION
        );
        assert_eq!(
            response["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
    }

    #[test]
    fn stdio_round_trip_lists_and_calls_tools() {
        let input = [
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": SUPPORTED_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "test-client",
                        "version": "0.1.0"
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "unfour.mock.ping",
                    "arguments": {
                        "message": "hello"
                    }
                }
            }),
        ]
        .into_iter()
        .map(|message| serde_json::to_string(&message).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        let mut output = Vec::new();

        run_stdio(Cursor::new(input), &mut output).expect("stdio server should complete");

        let responses = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(responses.len(), 3);
        assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 3);
        assert_eq!(
            responses[2]["result"]["structuredContent"],
            json!({
                "ok": true,
                "message": "pong",
                "echo": "hello"
            })
        );
    }
}
