use super::metadata_support::{args, error, invalid, required, result_schema, tool};
use super::{policy::ToolPolicyEvaluation, RegisteredTool, ToolAnnotations, ToolCallError};
use crate::command_bus_adapter::CommandBusAdapter;
use serde_json::{json, Value};

pub(super) fn registered_tool() -> RegisteredTool {
    tool("unfour.redis.check_key",
        "Check one exact UTF-8 Redis key in a saved connection. Optionally check a hash field or compare the complete string/field against equals. Returns metadata and the comparison outcome.",
        json!({
            "workspaceId":{"type":"string"}, "connectionId":{"type":"string"},
            "key":{"type":"string","maxLength":65536}, "field":{"type":"string","maxLength":65536},
            "equals":{"type":"string","maxLength":16384}
        }), &["connectionId","key"],
        result_schema(json!({"exists":{"type":"boolean"},"keyType":{"type":"string"},"pttlMs":{"type":"integer","minimum":-2},"matched":{"type":"boolean"}})),
        ToolAnnotations::remote_read(), check)
}

fn check(
    bus: &dyn CommandBusAdapter,
    policy: &ToolPolicyEvaluation,
    arguments: Value,
) -> Result<Value, ToolCallError> {
    let args = args(
        arguments,
        &["workspaceId", "connectionId", "key", "field", "equals"],
    )?;
    let id = required(&args, "connectionId")?;
    let bytes = |name: &'static str, limit: usize| {
        args.get(name)
            .map(|value| {
                value
                    .as_str()
                    .map(str::as_bytes)
                    .filter(|v| v.len() <= limit)
                    .ok_or_else(|| invalid(name))
            })
            .transpose()
    };
    let key = bytes("key", 65536)?.ok_or_else(|| invalid("key"))?;
    let result = bus
        .check_redis_key(crate::command_bus_adapter::RedisCheck {
            workspace: &policy.workspace.workspace_id,
            connection: &id,
            key,
            field: bytes("field", 65536)?,
            equals: bytes("equals", 16384)?,
        })
        .map_err(error)?;
    Ok(json!(result))
}
