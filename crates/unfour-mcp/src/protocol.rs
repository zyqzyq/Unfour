use serde_json::{json, Value};

pub const JSON_RPC_VERSION: &str = "2.0";

pub fn success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": JSON_RPC_VERSION,
        "id": id,
        "result": result,
    })
}

pub fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": JSON_RPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
        },
    })
}
