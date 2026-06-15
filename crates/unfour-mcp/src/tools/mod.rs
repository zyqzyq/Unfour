mod mock;

use serde::Serialize;
use serde_json::Value;

use crate::response::structured_tool_result;

type ToolHandler = fn(Value) -> Result<Value, ToolCallError>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub output_schema: Value,
}

struct RegisteredTool {
    definition: ToolDefinition,
    handler: ToolHandler,
}

pub struct ToolRegistry {
    tools: Vec<RegisteredTool>,
}

#[derive(Debug, PartialEq)]
pub enum ToolCallError {
    UnknownTool(String),
    InvalidArguments(String),
}

impl ToolRegistry {
    pub fn mock() -> Self {
        Self {
            tools: mock::registered_tools(),
        }
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| tool.definition.clone())
            .collect()
    }

    pub fn call(&self, name: &str, arguments: Value) -> Result<Value, ToolCallError> {
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.definition.name == name)
            .ok_or_else(|| ToolCallError::UnknownTool(name.to_string()))?;
        let result = (tool.handler)(arguments)?;

        Ok(structured_tool_result(result))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ToolRegistry;

    #[test]
    fn mock_tool_schemas_are_available() {
        let definitions = ToolRegistry::mock().definitions();

        assert_eq!(definitions.len(), 3);
        assert!(definitions
            .iter()
            .all(|definition| definition.name.starts_with("unfour.mock.")));
        assert!(definitions
            .iter()
            .all(|definition| definition.input_schema["type"] == "object"));
    }

    #[test]
    fn mock_echo_returns_structured_json() {
        let result = ToolRegistry::mock()
            .call("unfour.mock.echo", json!({ "value": "anything" }))
            .expect("mock echo should succeed");

        assert_eq!(
            result["structuredContent"],
            json!({
                "ok": true,
                "value": "anything"
            })
        );
        assert_eq!(result["isError"], false);
    }
}
