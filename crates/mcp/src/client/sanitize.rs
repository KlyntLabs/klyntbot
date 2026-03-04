//! Tool name sanitization for MCP tools.
//!
//! MCP server and tool names can contain arbitrary characters. This module
//! ensures the namespaced tool names registered in the `ToolRegistry` are
//! safe for LLM function-calling APIs (alphanumeric + underscore + hyphen).

use std::fmt::Write;

/// Maximum length for a tool name in LLM function-calling APIs.
const MAX_TOOL_NAME_LEN: usize = 64;

/// Replace characters outside `[a-zA-Z0-9_-]` with `_`.
pub fn sanitize_name(s: &str) -> String {
    // Fast path: skip allocation if input is already clean
    if is_clean(s) {
        return s.to_string();
    }
    s.chars().map(sanitize_char).collect()
}

/// Build a namespaced tool name: `mcp_{sanitized_server}_{sanitized_tool}`.
///
/// If the result exceeds 64 characters, it is truncated and a short hash
/// suffix is appended to preserve uniqueness.
///
/// Uses a single `String` buffer to avoid intermediate allocations.
pub fn build_tool_name(server: &str, tool: &str) -> String {
    let mut out = String::with_capacity(4 + 1 + server.len() + 1 + tool.len());
    out.push_str("mcp_");
    push_sanitized(&mut out, server);
    out.push('_');
    push_sanitized(&mut out, tool);

    if out.len() <= MAX_TOOL_NAME_LEN {
        return out;
    }

    // Truncate and append 8-char hash suffix for uniqueness
    let hash = simple_hash(&out);
    let truncate_to = MAX_TOOL_NAME_LEN - 9; // "_" + 8 hex chars
    out.truncate(truncate_to);
    write!(out, "_{:08x}", hash).expect("write to String");
    out
}

/// Build the server prefix used for unregistering tools: `mcp_{sanitized_server}_`.
pub fn server_prefix(server: &str) -> String {
    let mut out = String::with_capacity(4 + 1 + server.len() + 1);
    out.push_str("mcp_");
    push_sanitized(&mut out, server);
    out.push('_');
    out
}

/// Append `s` to `buf`, replacing invalid characters inline.
fn push_sanitized(buf: &mut String, s: &str) {
    buf.extend(s.chars().map(sanitize_char));
}

fn sanitize_char(c: char) -> char {
    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
        c
    } else {
        '_'
    }
}

/// Returns true if every byte is already a valid tool-name character.
fn is_clean(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Simple non-cryptographic hash for name truncation.
fn simple_hash(s: &str) -> u32 {
    let mut hash: u32 = 0;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name_alphanumeric_passthrough() {
        assert_eq!(sanitize_name("list_issues"), "list_issues");
        assert_eq!(sanitize_name("get-issue"), "get-issue");
        assert_eq!(sanitize_name("myTool123"), "myTool123");
    }

    #[test]
    fn test_sanitize_name_replaces_invalid_chars() {
        assert_eq!(sanitize_name("my.tool"), "my_tool");
        assert_eq!(sanitize_name("tool@name!"), "tool_name_");
        assert_eq!(sanitize_name("name with spaces"), "name_with_spaces");
        assert_eq!(sanitize_name("tool/path:v2"), "tool_path_v2");
    }

    #[test]
    fn test_build_tool_name_basic() {
        assert_eq!(
            build_tool_name("linear", "list_issues"),
            "mcp_linear_list_issues"
        );
    }

    #[test]
    fn test_build_tool_name_sanitizes() {
        assert_eq!(
            build_tool_name("my.server", "get/data"),
            "mcp_my_server_get_data"
        );
    }

    #[test]
    fn test_build_tool_name_truncates_long_names() {
        let long_server = "a".repeat(40);
        let long_tool = "b".repeat(40);
        let result = build_tool_name(&long_server, &long_tool);
        assert!(
            result.len() <= MAX_TOOL_NAME_LEN,
            "Name too long: {} chars",
            result.len()
        );
        assert!(result.starts_with("mcp_"));
    }

    #[test]
    fn test_build_tool_name_truncation_preserves_uniqueness() {
        // Two different long names should produce different truncated names
        let name1 = build_tool_name("server", &"a".repeat(60));
        let name2 = build_tool_name("server", &"b".repeat(60));
        assert_ne!(name1, name2);
    }

    #[test]
    fn test_server_prefix() {
        assert_eq!(server_prefix("linear"), "mcp_linear_");
        assert_eq!(server_prefix("my.server"), "mcp_my_server_");
    }
}
