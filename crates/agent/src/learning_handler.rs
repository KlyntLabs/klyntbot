//! LearningHandlerImpl — Implements LearningHandler trait for agent crate (Layer 5).
//!
//! Bridges the LearningHandler trait (defined in tools at Layer 3) with the
//! OutcomeStore, AdaptiveThresholds, and LearningAnalyzer (all in agent/learning),
//! following the dependency inversion pattern used by GoalHandlerImpl.

use async_trait::async_trait;
use common::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::learning_tool::{LearningHandler, LearningStatus, ThresholdEntry, ToolSummary};

use crate::learning::{adaptive::AdaptiveThresholds, analyzer::LearningAnalyzer, OutcomeStore};

/// Implements LearningHandler by delegating to OutcomeStore and LearningAnalyzer.
pub struct LearningHandlerImpl {
    outcome_store: Arc<RwLock<OutcomeStore>>,
    adaptive: Arc<RwLock<AdaptiveThresholds>>,
}

impl LearningHandlerImpl {
    pub fn new(
        outcome_store: Arc<RwLock<OutcomeStore>>,
        adaptive: Arc<RwLock<AdaptiveThresholds>>,
    ) -> Self {
        Self {
            outcome_store,
            adaptive,
        }
    }

    /// Run analysis and build a LearningStatus from the result.
    async fn build_status(&self) -> Result<LearningStatus> {
        let (outcomes, feedback) = {
            let mut store = self.outcome_store.write().await;
            let outcomes = store.get_all_outcomes().await?.to_vec();
            let feedback = store.get_all_feedback().await?.to_vec();
            (outcomes, feedback)
        };

        let analysis = LearningAnalyzer::analyze(&outcomes, &feedback);

        let adaptive = self.adaptive.read().await;
        let current_threshold = adaptive.current_threshold();

        let per_tool: HashMap<String, ToolSummary> = analysis
            .per_tool_stats
            .iter()
            .map(|(name, stats)| {
                (
                    name.clone(),
                    ToolSummary {
                        total_calls: stats.total_calls,
                        success_count: stats.success_count,
                        avg_duration_ms: stats.avg_duration_ms,
                    },
                )
            })
            .collect();

        Ok(LearningStatus {
            current_threshold,
            total_outcomes: analysis.total_outcomes,
            suggested_threshold: analysis.suggested_threshold,
            per_tool,
        })
    }
}

#[async_trait]
impl LearningHandler for LearningHandlerImpl {
    async fn get_status(&self) -> Result<Option<LearningStatus>> {
        let has_outcomes = {
            let mut store = self.outcome_store.write().await;
            let outcomes = store.get_all_outcomes().await?;
            !outcomes.is_empty()
        };

        if !has_outcomes {
            return Ok(None);
        }

        Ok(Some(self.build_status().await?))
    }

    async fn analyze_now(&self) -> Result<LearningStatus> {
        self.build_status().await
    }

    async fn get_threshold_history(&self, limit: usize) -> Result<Vec<ThresholdEntry>> {
        let adaptive = self.adaptive.read().await;
        let history = &adaptive.state().threshold_history;
        let start = history.len().saturating_sub(limit);
        Ok(history[start..]
            .iter()
            .map(|c| ThresholdEntry {
                from: c.from,
                to: c.to,
                reason: c.reason.clone(),
                timestamp: c.timestamp,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::{ExecutionMode, OutcomeRecord};
    use chrono::Utc;
    use tempfile::TempDir;

    async fn make_handler(tmp: &TempDir) -> (LearningHandlerImpl, Arc<RwLock<OutcomeStore>>) {
        let store = Arc::new(RwLock::new(OutcomeStore::new(
            tmp.path().join("outcomes.jsonl"),
        )));
        let adaptive = Arc::new(RwLock::new(
            AdaptiveThresholds::load(tmp.path().join("state.json"), 0.7, 0.4, 0.9, 50).await,
        ));
        let handler = LearningHandlerImpl::new(Arc::clone(&store), adaptive);
        (handler, store)
    }

    #[tokio::test]
    async fn test_get_status_empty() {
        let tmp = TempDir::new().unwrap();
        let (handler, _store) = make_handler(&tmp).await;
        let status = handler.get_status().await.unwrap();
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_get_status_with_outcomes() {
        let tmp = TempDir::new().unwrap();
        let (handler, store) = make_handler(&tmp).await;

        {
            let mut sg = store.write().await;
            sg.record(OutcomeRecord {
                id: "t1".to_string(),
                session_key: "cli:abc".to_string(),
                tool_name: "todo".to_string(),
                success: true,
                error_category: None,
                duration_ms: 25,
                confidence_score: None,
                confidence_dimensions: None,
                execution_mode: ExecutionMode::Chat,
                created_at: Utc::now(),
            })
            .await
            .unwrap();
        }

        let status = handler.get_status().await.unwrap();
        assert!(status.is_some());
        let s = status.unwrap();
        assert_eq!(s.total_outcomes, 1);
        assert!(s.per_tool.contains_key("todo"));
    }
}
