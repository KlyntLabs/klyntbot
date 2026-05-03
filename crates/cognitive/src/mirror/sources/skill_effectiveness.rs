//! SkillEffectivenessSource — tracks tool/skill effectiveness via EWMA.
//!
//! Periodically reads recent tool execution outcomes from coding-memory and
//! maintains per-tool effectiveness scores. The shared `EffectivenessScores`
//! map feeds into `ToolSearchTool::effectiveness_scores` for reranking.

use ai_core::{mirror::mirror_flush_secs, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use common::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Exponentially-weighted moving average alpha. Higher = more responsive.
const EWMA_ALPHA: f32 = 0.3;

/// Shared effectiveness scores that can be read by `ToolSearchTool`.
pub type EffectivenessScores = Arc<RwLock<HashMap<String, f32>>>;

/// Mirror signal source that tracks per-tool effectiveness.
///
/// On each flush cycle, queries recent tool execution records and updates
/// EWMA scores per tool. Successful executions push the score toward 1.0,
/// failures toward 0.0.
pub struct SkillEffectivenessSource {
    /// Running EWMA scores per tool name.
    scores: EffectivenessScores,
}

impl SkillEffectivenessSource {
    pub fn new() -> Self {
        Self {
            scores: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a clone of the current scores for use by `ToolSearchTool`.
    pub async fn current_scores(&self) -> HashMap<String, f32> {
        self.scores.read().await.clone()
    }

    /// Get the shared scores handle (clone-safe Arc).
    pub fn scores_handle(&self) -> EffectivenessScores {
        Arc::clone(&self.scores)
    }

    /// Record a tool execution outcome. `success` = true if the tool succeeded.
    pub async fn record_outcome(&self, tool_name: &str, success: bool) {
        let new_value = if success { 1.0 } else { 0.0 };
        let mut scores = self.scores.write().await;
        let entry = scores.entry(tool_name.to_string()).or_insert(0.5);
        *entry = *entry + EWMA_ALPHA * (new_value - *entry);
    }
}

impl Default for SkillEffectivenessSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MirrorSignalSource for SkillEffectivenessSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "skill_effectiveness",
            subscribed_kinds: &[], // empty = receives all signals (filters in accumulate)
            flush_interval_secs: Some(mirror_flush_secs(3600)),
        }
    }

    fn name(&self) -> &'static str {
        "skill-effectiveness-source"
    }

    async fn accumulate(&self, _signal: &ai_core::AiSignal) -> Result<()> {
        // TODO(T7): Extract tool_name and success from coding-memory queries
        // during flush. For now this is a no-op accumulator — the source is
        // driven by periodic flush, not by individual signals.
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        // TODO(T7): Query coding_memory for recent tool executions,
        // call record_outcome for each. For now, log current state.
        let count = self.scores.read().await.len();
        if count > 0 {
            tracing::debug!(tools = count, "Skill effectiveness snapshot flushed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ewma_starts_empty() {
        let src = SkillEffectivenessSource::new();
        let scores = src.current_scores().await;
        assert!(scores.is_empty());
    }

    #[tokio::test]
    async fn ewma_accumulates_per_skill() {
        let src = SkillEffectivenessSource::new();

        // Record a success
        src.record_outcome("bash", true).await;
        let scores = src.current_scores().await;
        let bash_score = scores["bash"];
        assert!(bash_score > 0.5, "success should push score above 0.5");

        // Record a failure
        src.record_outcome("bash", false).await;
        let scores = src.current_scores().await;
        let bash_score2 = scores["bash"];
        assert!(bash_score2 < bash_score, "failure should pull score down");
    }

    #[tokio::test]
    async fn different_tools_independent() {
        let src = SkillEffectivenessSource::new();
        src.record_outcome("bash", true).await;
        src.record_outcome("glob", false).await;

        let scores = src.current_scores().await;
        assert!(scores["bash"] > scores["glob"]);
    }

    #[tokio::test]
    async fn scores_handle_shares_state() {
        let src = SkillEffectivenessSource::new();
        let handle = src.scores_handle();

        // Write through the source
        src.record_outcome("edit", true).await;

        // Read through the handle
        let scores = handle.read().await;
        assert!(scores.contains_key("edit"));
    }
}
