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
        path: Option<&str>,
    ) -> Option<approval::SuggestedGrant> {
        let pattern = self
            .facade
            .suggest_approval_pattern(tool_name, path)
            .await?;
        Some(approval::SuggestedGrant {
            pattern: pattern.label.clone(),
            scope: match &pattern.scope {
                cognitive::mirror::sources::approval_history::PatternScope::ExactPath(p) => {
                    approval::GrantScope::ExactToolPath {
                        tool: tool_name.to_string(),
                        path: std::path::PathBuf::from(p),
                    }
                }
                cognitive::mirror::sources::approval_history::PatternScope::ParentFolder(p) => {
                    approval::GrantScope::ToolFolder {
                        tool: tool_name.to_string(),
                        folder: std::path::PathBuf::from(p),
                    }
                }
                cognitive::mirror::sources::approval_history::PatternScope::Glob(g) => {
                    approval::GrantScope::ToolGlob {
                        tool: tool_name.to_string(),
                        glob: g.clone(),
                    }
                }
            },
            reason: format!(
                "Mirror: {} prior approvals for this pattern",
                pattern.approval_count
            ),
        })
    }
}
