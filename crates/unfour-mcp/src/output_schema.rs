use jsonschema::validator_for;
use serde_json::Value;

use crate::response::{assert_call_meta, content_json};
use crate::tools::ToolRegistry;

const MCP_ENVELOPE_FIELDS: &[&str] = &[
    "ok",
    "tool",
    "environment",
    "risk_level",
    "duration_ms",
    "data",
    "warnings",
    "redactions",
];

/// Validate a successful `tools/call` result against the tool's `outputSchema`.
///
/// Future tools can reuse this from any `unfour-mcp` test by calling a tool
/// and then:
///
/// ```ignore
/// crate::output_schema::assert_success_matches_output_schema(&registry, "unfour.system.health", &result);
/// ```
pub fn assert_success_matches_output_schema(
    registry: &ToolRegistry,
    tool_name: &str,
    result: &Value,
) {
    assert_eq!(
        result.get("isError"),
        Some(&Value::Bool(false)),
        "{tool_name} should succeed: {result}"
    );
    let structured = result
        .get("structuredContent")
        .unwrap_or_else(|| panic!("{tool_name} success result is missing structuredContent"));
    assert_eq!(
        &content_json(result),
        structured,
        "{tool_name} content[0].text must deserialize to structuredContent"
    );

    let definition = registry
        .definitions()
        .into_iter()
        .find(|tool| tool.name == tool_name)
        .unwrap_or_else(|| panic!("missing tool definition for {tool_name}"));

    assert_no_unsolicited_envelope_fields(&definition.output_schema, structured, tool_name);
    assert_valid_against_schema(&definition.output_schema, structured, tool_name);

    let environment = result["_meta"]["environment"]
        .as_str()
        .unwrap_or_else(|| panic!("{tool_name} is missing _meta.environment"));
    let risk_level = result["_meta"]["riskLevel"]
        .as_str()
        .unwrap_or_else(|| panic!("{tool_name} is missing _meta.riskLevel"));
    assert_call_meta(result, environment, risk_level);
    assert_eq!(result["_meta"]["tool"], tool_name);
}

pub fn assert_valid_against_schema(schema: &Value, instance: &Value, tool_name: &str) {
    let validator = validator_for(schema).unwrap_or_else(|error| {
        panic!("{tool_name} outputSchema is not a valid JSON Schema: {error}")
    });
    if !validator.is_valid(instance) {
        let messages: Vec<String> = validator
            .iter_errors(instance)
            .map(|error| error.to_string())
            .collect();
        panic!(
            "{tool_name} structuredContent does not match outputSchema: {}\ninstance={instance}\nschema={schema}",
            messages.join("; ")
        );
    }
}

fn assert_no_unsolicited_envelope_fields(schema: &Value, instance: &Value, tool_name: &str) {
    let Some(object) = instance.as_object() else {
        return;
    };
    let declared = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for field in MCP_ENVELOPE_FIELDS {
        if object.contains_key(*field) && !declared.contains_key(*field) {
            panic!(
                "{tool_name} structuredContent includes envelope field `{field}` that is not in the tool outputSchema"
            );
        }
    }
}
