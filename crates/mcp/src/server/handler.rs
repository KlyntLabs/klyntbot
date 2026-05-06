//! MCP server handler utilities — shared helpers for MCP server implementations.

use rmcp::model::ErrorCode;
use rmcp::ErrorData as McpError;

/// Attempt to parse a `GateOutcome::Deny` reason as a structured approval-required error.
///
/// If the reason is a JSON string containing `"code": "approval-required"`,
/// returns a structured `McpError` with the approval details in the `data` field.
/// Returns `None` if the reason is not approval-related JSON.
pub fn try_approval_error(reason: &str) -> Option<McpError> {
    let parsed: serde_json::Value = serde_json::from_str(reason).ok()?;
    if parsed.get("code")?.as_str()? != "approval-required" {
        return None;
    }
    Some(McpError::new(
        ErrorCode::INVALID_REQUEST,
        parsed
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("This action requires user approval.")
            .to_string(),
        Some(parsed),
    ))
}

/// Convert a `GateOutcome::Deny` reason into an `McpError`.
///
/// If the reason matches the approval-required pattern, returns a structured error.
/// Otherwise returns a generic permission-denied error.
pub fn deny_to_mcp_error(reason: &str) -> McpError {
    try_approval_error(reason).unwrap_or_else(|| {
        McpError::new(ErrorCode::INVALID_REQUEST, reason.to_string(), None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_approval_reason() {
        let reason = serde_json::json!({
            "code": "approval-required",
            "tool": "bash",
            "action": "execute",
            "class": "destructive",
            "message": "This action requires user approval. Open Klynt on desktop to approve.",
        })
        .to_string();

        let err = try_approval_error(&reason).expect("should parse");
        assert_eq!(err.code, ErrorCode::INVALID_REQUEST);
        assert!(err.message.contains("desktop"));
        assert!(err.data.is_some());
        let data = err.data.unwrap();
        assert_eq!(data["code"], "approval-required");
        assert_eq!(data["tool"], "bash");
    }

    #[test]
    fn returns_none_for_non_json_reason() {
        assert!(try_approval_error("just a plain string").is_none());
    }

    #[test]
    fn returns_none_for_json_without_approval_code() {
        let reason = serde_json::json!({"code": "other", "message": "nope"}).to_string();
        assert!(try_approval_error(&reason).is_none());
    }

    #[test]
    fn deny_to_mcp_error_falls_back_for_plain_reason() {
        let err = deny_to_mcp_error("permission denied");
        assert_eq!(err.code, ErrorCode::INVALID_REQUEST);
        assert_eq!(err.message, "permission denied");
        assert!(err.data.is_none());
    }
}
