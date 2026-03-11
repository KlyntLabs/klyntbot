//! MCP tool handler routing.
//!
//! Defines which internal tools are exposed via MCP and provides
//! the bridge between rmcp's handler interface and klyntbot's ToolRegistry.

use super::security;

/// Tools exposed via MCP server to external AI agents.
///
/// This is a curated subset — external agents don't get raw access
/// to all internal tools. Each tool name must match a registered
/// tool in klyntbot's ToolRegistry.
pub const MCP_EXPOSED_TOOLS: &[&str] = &[
    "task",
    "memory",
    "annotate",
    "search",
    "project",
    "area",
    "okr",
    "context_request",
    "learning",
    "web_search",
];

/// Check if a tool is in the exposed list.
pub fn is_exposed(tool_name: &str) -> bool {
    MCP_EXPOSED_TOOLS.contains(&tool_name)
}

/// Validate and sanitize MCP tool call parameters.
///
/// Returns sanitized JSON string on success, or an error if the
/// tool is not exposed.
pub fn validate_tool_call(tool_name: &str, params: &serde_json::Value) -> Result<String, String> {
    if !is_exposed(tool_name) {
        return Err(format!("Tool '{}' is not exposed via MCP", tool_name));
    }

    let params_str = params.to_string();
    Ok(security::sanitize_input(&params_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exposed_tools_list_not_empty() {
        assert!(!MCP_EXPOSED_TOOLS.is_empty());
    }

    #[test]
    fn test_is_exposed_returns_true_for_known_tools() {
        assert!(is_exposed("task"));
        assert!(is_exposed("memory"));
        assert!(is_exposed("annotate"));
        assert!(is_exposed("context_request"));
    }

    #[test]
    fn test_is_exposed_returns_false_for_unknown_tools() {
        assert!(!is_exposed("internal_only_tool"));
        assert!(!is_exposed(""));
        assert!(!is_exposed("shell"));
    }

    #[test]
    fn test_validate_tool_call_rejects_unexposed() {
        let params = serde_json::json!({"action": "list"});
        let result = validate_tool_call("not_exposed", &params);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not exposed"));
    }

    #[test]
    fn test_validate_tool_call_accepts_and_sanitizes() {
        let params = serde_json::json!({"query": "hello\x00world"});
        let result = validate_tool_call("task", &params);
        assert!(result.is_ok());
        let sanitized = result.unwrap();
        assert!(!sanitized.contains('\x00'));
    }
}
