//! LearningHandlerImpl — Implements LearningHandler trait for agent crate (Layer 5).
//!
//! Reads from StrategyRepo (strategy_records table) to produce learning insights.
//! Follows the dependency inversion pattern used by other handler impls.

use async_trait::async_trait;
use common::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::learning_tool::{LearningHandler, LearningStatus, ThresholdEntry, ToolSummary};

use crate::learning::adaptive::AdaptiveThresholds;

/// Implements LearningHandler by reading from StrategyRepo.
pub struct LearningHandlerImpl {
    strategy_repo: storage::StrategyRepo,
    adaptive: Arc<RwLock<AdaptiveThresholds>>,
}

impl LearningHandlerImpl {
    pub fn new(
        strategy_repo: storage::StrategyRepo,
        adaptive: Arc<RwLock<AdaptiveThresholds>>,
    ) -> Self {
        Self {
            strategy_repo,
            adaptive,
        }
    }
}

#[async_trait]
impl LearningHandler for LearningHandlerImpl {
    async fn get_status(&self) -> Result<Option<LearningStatus>> {
        let count = self.strategy_repo.count_all().await?;
        if count == 0 {
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

impl LearningHandlerImpl {
    async fn build_status(&self) -> Result<LearningStatus> {
        let overall = self.strategy_repo.get_overall_stats().await?;
        let tool_rows = self.strategy_repo.get_tool_stats().await?;

        let per_tool: HashMap<String, ToolSummary> = tool_rows
            .into_iter()
            .map(|t| {
                (
                    t.tool_name,
                    ToolSummary {
                        total_calls: t.total_calls,
                        success_count: t.success_count,
                        avg_duration_ms: t.avg_duration_ms,
                    },
                )
            })
            .collect();

        let adaptive = self.adaptive.read().await;

        Ok(LearningStatus {
            current_threshold: adaptive.current_threshold(),
            total_strategy_records: overall.total_records,
            strategy_accuracy: overall.accuracy,
            avg_response_time_ms: overall.avg_response_time_ms,
            avg_satisfaction: overall.avg_satisfaction,
            suggested_threshold: adaptive
                .state()
                .last_analysis
                .as_ref()
                .map(|a| a.suggested_threshold)
                .unwrap_or_else(|| adaptive.current_threshold()),
            per_tool,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_handler() -> (LearningHandlerImpl, storage::StrategyRepo) {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let strategy_repo = storage::StrategyRepo::new(pool.inner().clone());
        let adaptive = Arc::new(RwLock::new(
            crate::learning::adaptive::AdaptiveThresholds::new_in_memory(0.7, 0.4, 0.9, 50),
        ));
        let handler = LearningHandlerImpl::new(strategy_repo.clone(), adaptive);
        (handler, strategy_repo)
    }

    #[tokio::test]
    async fn test_get_status_empty() {
        let (handler, _) = make_handler().await;
        let status = handler.get_status().await.unwrap();
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_suggested_threshold_differs_from_current() {
        let (handler, repo) = make_handler().await;

        // Insert a record so get_status returns Some
        let row = storage::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: "req-st".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: Some(1.0),
            response_time_ms: 500,
            chat_id: None,
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
            retrieved_memory_count: None,
        };
        repo.create(&row).await.unwrap();

        // Inject a mock analysis with a different suggested_threshold
        {
            use crate::learning::types::{AnalysisResult, EnrichmentStats};
            let analysis = AnalysisResult {
                computed_at: chrono::Utc::now(),
                total_outcomes: 100,
                per_tool_stats: std::collections::HashMap::new(),
                suggested_threshold: 0.55, // Different from initial 0.7
                threshold_confidence: 0.8,
                enrichment_stats: EnrichmentStats::default(),
            };
            let mut adaptive = handler.adaptive.write().await;
            adaptive.apply_analysis(&analysis);
        }

        let status = handler.get_status().await.unwrap().unwrap();
        // suggested_threshold should be 0.55 from the analysis, NOT equal to current
        assert!(
            (status.suggested_threshold - 0.55).abs() < 0.01,
            "suggested_threshold should come from analysis (0.55), got {}",
            status.suggested_threshold
        );
        assert!(
            (status.current_threshold - status.suggested_threshold).abs() > 0.01,
            "current and suggested should differ"
        );
    }

    #[tokio::test]
    async fn test_get_status_with_records() {
        let (handler, repo) = make_handler().await;

        let row = storage::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: "req-1".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: Some(1.0),
            response_time_ms: 500,
            chat_id: None,
            tool_name: Some("todo".to_string()),
            tool_success: Some(true),
            tool_duration_ms: Some(50),
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
            retrieved_memory_count: None,
        };
        repo.create(&row).await.unwrap();

        let status = handler.get_status().await.unwrap();
        assert!(status.is_some());
        let s = status.unwrap();
        assert_eq!(s.total_strategy_records, 1);
        assert!((s.strategy_accuracy - 1.0).abs() < 0.01);
        assert!(s.per_tool.contains_key("todo"));
    }
}
