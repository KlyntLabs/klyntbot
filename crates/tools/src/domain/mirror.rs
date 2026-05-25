//! MirrorTool — read-only access to the Mirror self-reflection layer.
//!
//! Exposes routing patterns, brain versions, narratives, and meta-rules
//! via the tool system (and MCP) so external AI clients can query them.

use std::sync::Arc;

use common::Result;
use tools_core::{tool_actions, ActionParams};

use cognitive::mirror::MirrorFacade;

// ---------------------------------------------------------------------------
// Action param structs
// ---------------------------------------------------------------------------

#[derive(Debug, ActionParams)]
pub struct GetStateParams {}

#[derive(Debug, ActionParams)]
pub struct GetNarrativesParams {
    /// Maximum number of narratives to return (default: 5)
    pub limit: Option<i64>,
}

#[derive(Debug, ActionParams)]
pub struct GetRoutingHistoryParams {
    /// Number of days of routing history to return (default: 7)
    pub days: Option<i64>,
}

#[derive(Debug, ActionParams)]
pub struct GetBrainVersionsParams {}

#[derive(Debug, ActionParams)]
pub struct GetMetaRulesParams {}

// ---------------------------------------------------------------------------
// MirrorTool
// ---------------------------------------------------------------------------

pub struct MirrorTool {
    facade: Arc<MirrorFacade>,
}

impl MirrorTool {
    pub fn new(facade: Arc<MirrorFacade>) -> Self {
        Self { facade }
    }
}

#[tool_actions(
    ctx = "()",
    name = "mirror",
    description = "Query the Mirror self-reflection layer for routing patterns, brain versions, narratives, and experiment status. All actions are read-only.",
    category = "Memory",
    tags = "mirror,reflection,routing,brain,narrative,meta-rule",
    cost = "Free"
)]
impl MirrorTool {
    /// Return the full current MirrorState (latest snapshot, narrative,
    /// pending snippets, meta-rules, brain version, trial previews).
    #[action(name = "get_state")]
    async fn get_state(&self, _params: GetStateParams, _ctx: ()) -> Result<String> {
        let state = self.facade.get_state().await?;
        serde_json::to_string_pretty(&state).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "Failed to serialize MirrorState: {e}"
            )))
        })
    }

    /// Return recent trend narratives (newest first).
    #[action(name = "get_narratives")]
    async fn get_narratives(
        &self,
        params: GetNarrativesParams,
        _ctx: (),
    ) -> Result<String> {
        let limit = params.limit.unwrap_or(5) as u32;
        let narratives = self.facade.get_narratives(limit).await?;
        serde_json::to_string_pretty(&narratives).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "Failed to serialize narratives: {e}"
            )))
        })
    }

    /// Return routing snapshots from the last N days.
    #[action(name = "get_routing_history")]
    async fn get_routing_history(
        &self,
        params: GetRoutingHistoryParams,
        _ctx: (),
    ) -> Result<String> {
        let days = params.days.unwrap_or(7) as u32;
        let history = self.facade.get_routing_history(days).await?;
        serde_json::to_string_pretty(&history).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "Failed to serialize routing history: {e}"
            )))
        })
    }

    /// Return all brain versions (newest first).
    #[action(name = "get_brain_versions")]
    async fn get_brain_versions(
        &self,
        _params: GetBrainVersionsParams,
        _ctx: (),
    ) -> Result<String> {
        let versions = self.facade.get_brain_versions().await?;
        serde_json::to_string_pretty(&versions).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "Failed to serialize brain versions: {e}"
            )))
        })
    }

    /// Return active and pending meta-rules.
    #[action(name = "get_meta_rules")]
    async fn get_meta_rules(
        &self,
        _params: GetMetaRulesParams,
        _ctx: (),
    ) -> Result<String> {
        let (active, pending) = self.facade.get_meta_rules().await?;
        let result = serde_json::json!({
            "active": active,
            "pending": pending,
        });
        serde_json::to_string_pretty(&result).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "Failed to serialize meta-rules: {e}"
            )))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cognitive::mirror::MirrorFacade;

    async fn setup() -> MirrorTool {
        let repo = cognitive::mirror::MirrorRepo::new(test_pool().await);
        MirrorTool::new(Arc::new(MirrorFacade::new(repo)))
    }

    async fn test_pool() -> storage::StoragePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &cognitive::repos::cognitive_migrations(),
        )
        .await
        .unwrap();
        pool
    }

    #[test]
    fn test_mirror_tool_name() {
        // Construct a tool using tokio runtime since we need async setup
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tool: Box<dyn tools_core::Tool> = Box::new(rt.block_on(setup()));
        assert_eq!(tool.name(), "mirror");
    }

    #[tokio::test]
    async fn test_get_state_empty() {
        let tool = setup().await;
        let result = tool.get_state(GetStateParams {}, ()).await.unwrap();
        assert!(result.contains("lastRoutingSnapshot"));
    }

    #[tokio::test]
    async fn test_get_narratives_empty() {
        let tool = setup().await;
        let result = tool
            .get_narratives(GetNarrativesParams { limit: None }, ())
            .await
            .unwrap();
        assert_eq!(result.trim(), "[]");
    }

    #[tokio::test]
    async fn test_get_routing_history_empty() {
        let tool = setup().await;
        let result = tool
            .get_routing_history(GetRoutingHistoryParams { days: None }, ())
            .await
            .unwrap();
        assert_eq!(result.trim(), "[]");
    }

    #[tokio::test]
    async fn test_get_brain_versions_empty() {
        let tool = setup().await;
        let result = tool
            .get_brain_versions(GetBrainVersionsParams {}, ())
            .await
            .unwrap();
        assert_eq!(result.trim(), "[]");
    }

    #[tokio::test]
    async fn test_get_meta_rules_empty() {
        let tool = setup().await;
        let result = tool
            .get_meta_rules(GetMetaRulesParams {}, ())
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["active"].as_array().unwrap().is_empty());
        assert!(parsed["pending"].as_array().unwrap().is_empty());
    }
}
