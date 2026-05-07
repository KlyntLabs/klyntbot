//! Bridge between the cognitive Mirror facade and the approval crate's
//! `ApprovalSuggester` trait. Lives in `app-core` to avoid circular deps.

use std::sync::Arc;

/// Wraps the cognitive Mirror facade and implements `approval::ApprovalSuggester`.
pub struct MirrorApprovalSuggester {
    facade: Arc<cognitive::mirror::MirrorFacade>,
}

impl MirrorApprovalSuggester {
    pub fn new(facade: Arc<cognitive::mirror::MirrorFacade>) -> Self {
        Self { facade }
    }
}

#[async_trait::async_trait]
impl approval::ApprovalSuggester for MirrorApprovalSuggester {
    async fn suggest(
        &self,
        tool_name: &str,
        args_hash: &str,
    ) -> Option<approval::SuggestedGrant> {
        let pattern = self.facade.suggest_approval_pattern(tool_name, args_hash).await?;
        Some(approval::SuggestedGrant {
            pattern: pattern.label.clone(),
            scope: approval::GrantScope::ToolGlob {
                tool: tool_name.to_string(),
                glob: format!("{}*", pattern.args_hash),
            },
            reason: format!(
                "Mirror: {} prior approvals for this tool configuration",
                pattern.approval_count
            ),
        })
    }
}
