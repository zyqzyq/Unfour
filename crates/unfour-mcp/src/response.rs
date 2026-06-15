use serde_json::{json, Value};

pub fn structured_tool_result(value: Value) -> Value {
    let text = serde_json::to_string(&value).expect("serializing a JSON value cannot fail");

    json!({
        "content": [
            {
                "type": "text",
                "text": text,
            }
        ],
        "structuredContent": value,
        "isError": false,
    })
}

pub fn structured_tool_error(code: &str, message: &str) -> Value {
    let value = json!({
        "error": {
            "code": code,
            "message": message,
        }
    });
    let text = serde_json::to_string(&value).expect("serializing a JSON value cannot fail");

    json!({
        "content": [
            {
                "type": "text",
                "text": text,
            }
        ],
        "structuredContent": value,
        "isError": true,
    })
}
