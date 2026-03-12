//! Translates internal tool schemas (OpenAI JSON Schema format) to rmcp MCP Tool definitions.

use rmcp::model::Tool as McpTool;

/// Convert an internal tool's schema to an MCP Tool definition.
///
/// Internal tools produce OpenAI-style JSON Schema via `Tool::parameters()`.
/// rmcp's `Tool` is serde-deserializable from JSON, so we build the JSON
/// representation and deserialize it to get all fields set correctly.
pub fn internal_to_mcp_tool(
    name: &str,
    description: &str,
    parameters: serde_json::Value,
) -> McpTool {
    let input_schema = if parameters.is_object() && !parameters.as_object().unwrap().is_empty() {
        parameters
    } else {
        serde_json::json!({"type": "object", "properties": {}})
    };

    serde_json::from_value(serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    }))
    .expect("static tool JSON is always valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_schema_translation() {
        let params = serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "create"] },
                "title": { "type": "string" }
            },
            "required": ["action"]
        });

        let tool = internal_to_mcp_tool("task", "Manage tasks", params);
        assert_eq!(tool.name.as_ref(), "task");
        assert_eq!(tool.description.as_deref(), Some("Manage tasks"));
        // Verify schema roundtrips correctly via JSON
        let schema_json = serde_json::to_value(&tool.input_schema).unwrap();
        assert!(schema_json["properties"]["action"].is_object());
        assert!(schema_json["properties"]["title"].is_object());
    }

    #[test]
    fn test_empty_schema_translation() {
        let params = serde_json::json!({});
        let tool = internal_to_mcp_tool("status", "Get status", params);
        assert_eq!(tool.name.as_ref(), "status");
        // Empty/minimal schema — should deserialize without error
        let schema_json = serde_json::to_value(&tool.input_schema).unwrap();
        assert!(schema_json.is_object());
    }
}
