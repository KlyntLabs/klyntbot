//! MCP approval channel — always declines with a structured reason
//! so the remote caller knows approval is required on another surface.

use approval::{
    ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalDecision, ApprovalRequest,
    BlockingFallbackChannel,
};
use std::collections::HashSet;

/// Approval channel for MCP server connections.
///
/// MCP clients (external AI agents) cannot interactively approve actions,
/// so every request is declined with a structured JSON reason indicating
/// that approval must be obtained on a different surface (e.g. desktop).
pub struct McpApprovalChannel;

#[async_trait::async_trait]
impl ApprovalChannel for McpApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision {
        // Re-use the fallback decline and wrap the reason in JSON.
        let fallback = BlockingFallbackChannel::desktop_prompt();
        let base = fallback.request(req.clone()).await;
        let reason = match base {
            ApprovalDecision::Decline { reason } => serde_json::json!({
                "code": "approval-required",
                "tool": req.tool_name,
                "action": req.action,
                "class": req.class,
                "message": reason,
            })
            .to_string(),
            other => return other,
        };

        ApprovalDecision::Decline { reason }
    }

    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: false,
            supports_classes: HashSet::from([ApprovalClass::Destructive, ApprovalClass::Admin]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approval::{ApprovalContext, ApprovalScope, ChannelKind};

    #[tokio::test]
    async fn always_declines_with_approval_required_code() {
        let chan = McpApprovalChannel;
        let req = ApprovalRequest {
            tool_name: "bash".into(),
            action: Some("execute".into()),
            args: serde_json::json!({"cmd": "ls"}),
            class: ApprovalClass::Destructive,
            scope: ApprovalScope::ToolAction,
            ctx: ApprovalContext {
                mode: common::SessionMode::Coding,
                channel: ChannelKind::Mcp,
                session_id: "s1".into(),
                user_id: None,
            },
        };

        let decision = chan.request(req).await;
        match decision {
            ApprovalDecision::Decline { reason } => {
                let parsed: serde_json::Value = serde_json::from_str(&reason).unwrap();
                assert_eq!(parsed["code"], "approval-required");
                assert_eq!(parsed["tool"], "bash");
                assert_eq!(parsed["action"], "execute");
                assert!(parsed["message"].as_str().unwrap().contains("desktop"));
            }
            _ => panic!("expected Decline"),
        }
    }

    #[test]
    fn capabilities_supports_destructive_and_admin() {
        let caps = McpApprovalChannel.capabilities();
        assert!(!caps.supports_inline);
        assert!(caps.supports_classes.contains(&ApprovalClass::Destructive));
        assert!(caps.supports_classes.contains(&ApprovalClass::Admin));
        assert!(!caps.supports_classes.contains(&ApprovalClass::Safe));
        assert!(!caps.supports_classes.contains(&ApprovalClass::Sensitive));
    }
}
