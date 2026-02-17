//! AdaptiveThresholds — adjusts the ConfidenceEvaluator threshold
//! based on analysis results.

use std::path::{Path, PathBuf};

use chrono::Utc;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use common::Result;

use super::types::{AdaptiveThresholdState, AnalysisResult, ThresholdChange};

/// Maximum change per adjustment cycle.
const MAX_THRESHOLD_STEP: f32 = 0.05;

pub struct AdaptiveThresholds {
    state_path: PathBuf,
    state: AdaptiveThresholdState,
    min_threshold: f32,
    max_threshold: f32,
    min_outcomes: usize,
}

impl AdaptiveThresholds {
    /// Load state from disk, or create fresh state with the given initial threshold.
    ///
    /// `min_threshold`, `max_threshold`, and `min_outcomes` are sourced from
    /// `LearningConfig` and replace the previous module-level constants.
    pub async fn load(
        state_path: PathBuf,
        initial_threshold: f32,
        min_threshold: f32,
        max_threshold: f32,
        min_outcomes: usize,
    ) -> Self {
        let state = if state_path.exists() {
            match tokio::fs::read_to_string(&state_path).await {
                Ok(content) => match serde_json::from_str::<AdaptiveThresholdState>(&content) {
                    Ok(state) => state,
                    Err(e) => {
                        warn!("Failed to parse learning state, using defaults: {}", e);
                        Self::fresh_state(initial_threshold)
                    }
                },
                Err(e) => {
                    warn!("Failed to read learning state, using defaults: {}", e);
                    Self::fresh_state(initial_threshold)
                }
            }
        } else {
            Self::fresh_state(initial_threshold)
        };

        Self {
            state_path,
            state,
            min_threshold,
            max_threshold,
            min_outcomes,
        }
    }

    fn fresh_state(threshold: f32) -> AdaptiveThresholdState {
        AdaptiveThresholdState {
            current_threshold: threshold,
            last_analysis: None,
            threshold_history: Vec::new(),
            updated_at: Utc::now(),
        }
    }

    /// Current threshold value.
    pub fn current_threshold(&self) -> f32 {
        self.state.current_threshold
    }

    /// Get the full state (for reporting).
    pub fn state(&self) -> &AdaptiveThresholdState {
        &self.state
    }

    /// Apply analysis results and return new threshold (if changed).
    pub fn apply_analysis(&mut self, analysis: &AnalysisResult) -> Option<f32> {
        if analysis.total_outcomes < self.min_outcomes {
            info!(
                "Skipping threshold adaptation: {} outcomes < {} minimum",
                analysis.total_outcomes, self.min_outcomes
            );
            self.state.last_analysis = Some(analysis.clone());
            self.state.updated_at = Utc::now();
            return None;
        }

        let suggested = analysis
            .suggested_threshold
            .clamp(self.min_threshold, self.max_threshold);
        let delta = (suggested - self.state.current_threshold)
            .clamp(-MAX_THRESHOLD_STEP, MAX_THRESHOLD_STEP);
        let new_threshold =
            (self.state.current_threshold + delta).clamp(self.min_threshold, self.max_threshold);

        if (new_threshold - self.state.current_threshold).abs() < 0.001 {
            self.state.last_analysis = Some(analysis.clone());
            self.state.updated_at = Utc::now();
            return None;
        }

        let old = self.state.current_threshold;
        self.state.threshold_history.push(ThresholdChange {
            from: old,
            to: new_threshold,
            reason: format!(
                "Analysis suggested {:.3} (confidence {:.2}), step-limited from {:.3}",
                suggested, analysis.threshold_confidence, old
            ),
            timestamp: Utc::now(),
        });
        self.state.current_threshold = new_threshold;
        self.state.last_analysis = Some(analysis.clone());
        self.state.updated_at = Utc::now();

        info!(
            "Adaptive threshold updated: {:.3} → {:.3}",
            old, new_threshold
        );
        Some(new_threshold)
    }

    /// Persist state to disk (atomic: write to .tmp then rename).
    pub async fn save(&self) -> Result<()> {
        if let Some(parent) = self.state_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to create learning state directory: {}",
                    e
                ))
            })?;
        }

        let tmp_path = self.state_path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(&self.state).map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to serialize learning state: {}", e))
        })?;

        let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to create learning state temp file: {}",
                e
            ))
        })?;
        file.write_all(content.as_bytes()).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to write learning state temp file: {}",
                e
            ))
        })?;
        file.flush().await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to flush learning state temp file: {}",
                e
            ))
        })?;
        file.sync_data().await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to sync learning state temp file: {}",
                e
            ))
        })?;

        tokio::fs::rename(&tmp_path, &self.state_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to rename learning state file: {}",
                    e
                ))
            })?;

        Ok(())
    }

    /// Get the state file path.
    pub fn state_path(&self) -> &Path {
        &self.state_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::types::EnrichmentStats;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_analysis(total: usize, suggested: f32, confidence: f32) -> AnalysisResult {
        AnalysisResult {
            computed_at: Utc::now(),
            total_outcomes: total,
            per_tool_stats: HashMap::new(),
            suggested_threshold: suggested,
            threshold_confidence: confidence,
            enrichment_stats: EnrichmentStats::default(),
        }
    }

    fn default_adaptive(
        path: PathBuf,
        initial: f32,
    ) -> impl std::future::Future<Output = AdaptiveThresholds> {
        AdaptiveThresholds::load(path, initial, 0.4, 0.9, 50)
    }

    #[tokio::test]
    async fn test_cold_start_protection() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let mut adaptive = default_adaptive(path, 0.7).await;

        let analysis = make_analysis(10, 0.5, 0.3); // too few outcomes
        let result = adaptive.apply_analysis(&analysis);
        assert!(result.is_none());
        assert!((adaptive.current_threshold() - 0.7).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_threshold_adjustment_clamped_step() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let mut adaptive = default_adaptive(path, 0.7).await;

        // Suggest a big jump to 0.4 — should be step-limited
        let analysis = make_analysis(100, 0.4, 0.9);
        let result = adaptive.apply_analysis(&analysis);
        assert!(result.is_some());
        let new = result.unwrap();
        // Should be 0.7 - 0.05 = 0.65 (step limited)
        assert!((new - 0.65).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_threshold_clamped_to_bounds() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let mut adaptive = default_adaptive(path, 0.42).await;

        // Suggest below minimum (0.4)
        let analysis = make_analysis(100, 0.1, 0.9);
        let result = adaptive.apply_analysis(&analysis);
        if let Some(new) = result {
            assert!(new >= 0.4);
        }
    }

    #[tokio::test]
    async fn test_no_change_when_threshold_matches() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let mut adaptive = default_adaptive(path, 0.7).await;

        let analysis = make_analysis(100, 0.7, 0.9);
        let result = adaptive.apply_analysis(&analysis);
        assert!(result.is_none()); // no change needed
    }

    #[tokio::test]
    async fn test_save_and_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");

        {
            let mut adaptive = default_adaptive(path.clone(), 0.7).await;
            let analysis = make_analysis(100, 0.6, 0.8);
            adaptive.apply_analysis(&analysis);
            adaptive.save().await.unwrap();
        }

        let reloaded = default_adaptive(path, 0.7).await;
        // Should load the saved threshold, not the default
        assert!((reloaded.current_threshold() - 0.65).abs() < 0.01);
        assert!(!reloaded.state().threshold_history.is_empty());
    }

    #[tokio::test]
    async fn test_history_tracking() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let mut adaptive = default_adaptive(path, 0.7).await;

        let analysis = make_analysis(100, 0.5, 0.8);
        adaptive.apply_analysis(&analysis);

        assert_eq!(adaptive.state().threshold_history.len(), 1);
        let change = &adaptive.state().threshold_history[0];
        assert!((change.from - 0.7).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_configurable_min_outcomes_blocks_adaptation() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        // Use a higher min_outcomes than the 100 we'll provide
        let mut adaptive = AdaptiveThresholds::load(path, 0.7, 0.4, 0.9, 200).await;

        let analysis = make_analysis(100, 0.5, 0.9); // 100 < 200 min
        let result = adaptive.apply_analysis(&analysis);
        assert!(
            result.is_none(),
            "Should not adapt with fewer than min_outcomes"
        );
    }

    #[tokio::test]
    async fn test_configurable_bounds_clamp_threshold() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        // Custom narrow range [0.65, 0.75]
        let mut adaptive = AdaptiveThresholds::load(path, 0.7, 0.65, 0.75, 50).await;

        // Suggest far below min_threshold
        let analysis = make_analysis(100, 0.2, 0.9);
        let result = adaptive.apply_analysis(&analysis);
        if let Some(new) = result {
            assert!(new >= 0.65, "Must not go below custom min_threshold");
            assert!(new <= 0.75, "Must not exceed custom max_threshold");
        }
    }
}
