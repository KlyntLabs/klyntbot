//! Mock learning handler for integration tests

use async_trait::async_trait;
use common::Result;
use std::collections::HashMap;
use tools::learning_tool::{LearningHandler, LearningStatus, ThresholdEntry};

pub struct MockLearningHandler {
    pub status: Option<LearningStatus>,
    pub history: Vec<ThresholdEntry>,
}

impl MockLearningHandler {
    pub fn with_status(status: LearningStatus) -> Self {
        Self {
            status: Some(status),
            history: vec![],
        }
    }

    pub fn empty() -> Self {
        Self {
            status: None,
            history: vec![],
        }
    }

    pub fn with_history(history: Vec<ThresholdEntry>) -> Self {
        Self {
            status: None,
            history,
        }
    }

    pub fn default_status() -> LearningStatus {
        LearningStatus {
            current_threshold: 0.7,
            total_strategy_records: 0,
            strategy_accuracy: 0.0,
            avg_response_time_ms: 0,
            avg_satisfaction: None,
            suggested_threshold: 0.7,
            per_tool: HashMap::new(),
        }
    }
}

#[async_trait]
impl LearningHandler for MockLearningHandler {
    async fn get_status(&self) -> Result<Option<LearningStatus>> {
        Ok(self.status.clone())
    }

    async fn analyze_now(&self) -> Result<LearningStatus> {
        Ok(self
            .status
            .clone()
            .unwrap_or_else(Self::default_status))
    }

    async fn get_threshold_history(&self, limit: usize) -> Result<Vec<ThresholdEntry>> {
        let entries = &self.history;
        let start = entries.len().saturating_sub(limit);
        Ok(entries[start..].to_vec())
    }
}
