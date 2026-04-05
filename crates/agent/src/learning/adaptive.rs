//! AdaptiveThresholds — adjusts the ConfidenceEvaluator threshold
//! based on analysis results.

use chrono::Utc;
use tracing::{info, warn};

use common::Result;

use super::types::{AdaptiveThresholdState, AnalysisResult, ThresholdChange};

/// Maximum change per adjustment cycle.
const MAX_THRESHOLD_STEP: f32 = 0.05;

/// Well-known key for the adaptive threshold state in the learning_state table.
const ADAPTIVE_STATE_KEY: &str = "adaptive_thresholds";

pub struct AdaptiveThresholds {
    state: AdaptiveThresholdState,
    min_threshold: f32,
    max_threshold: f32,
    min_outcomes: usize,
    repo: Option<storage::LearningStateRepo>,
}

/// Core threshold-adaptation logic shared by production and tests.
fn apply_analysis_impl(
    state: &mut AdaptiveThresholdState,
    analysis: &AnalysisResult,
    min_threshold: f32,
    max_threshold: f32,
    min_outcomes: usize,
) -> Option<f32> {
    if analysis.total_outcomes < min_outcomes {
        info!(
            "Skipping threshold adaptation: {} outcomes < {} minimum",
            analysis.total_outcomes, min_outcomes
        );
        state.last_analysis = Some(analysis.clone());
        state.updated_at = Utc::now();
        return None;
    }

    let suggested = analysis
        .suggested_threshold
        .clamp(min_threshold, max_threshold);
    // Scale step size by data confidence: more data → larger allowed steps → faster convergence.
    let effective_step = MAX_THRESHOLD_STEP * analysis.threshold_confidence.clamp(0.2, 1.0);
    let delta = (suggested - state.current_threshold).clamp(-effective_step, effective_step);
    let new_threshold = (state.current_threshold + delta).clamp(min_threshold, max_threshold);

    if (new_threshold - state.current_threshold).abs() < 0.001 {
        state.last_analysis = Some(analysis.clone());
        state.updated_at = Utc::now();
        return None;
    }

    let old = state.current_threshold;
    state.push_change(ThresholdChange {
        from: old,
        to: new_threshold,
        reason: format!(
            "Analysis suggested {:.3} (confidence {:.2}), step-limited from {:.3}",
            suggested, analysis.threshold_confidence, old
        ),
        timestamp: Utc::now(),
    });
    state.current_threshold = new_threshold;
    state.last_analysis = Some(analysis.clone());
    state.updated_at = Utc::now();

    info!(
        "Adaptive threshold updated: {:.3} → {:.3}",
        old, new_threshold
    );
    Some(new_threshold)
}

impl AdaptiveThresholds {
    /// Load state from SQL repository, or create fresh state with the given initial threshold.
    pub async fn new(
        repo: storage::LearningStateRepo,
        initial_threshold: f32,
        min_threshold: f32,
        max_threshold: f32,
        min_outcomes: usize,
    ) -> Self {
        let state = match repo.get_value(ADAPTIVE_STATE_KEY).await {
            Ok(Some(value)) => match serde_json::from_value::<AdaptiveThresholdState>(value) {
                Ok(state) => state,
                Err(e) => {
                    warn!(
                        "Failed to parse learning state from SQL, using defaults: {}",
                        e
                    );
                    Self::fresh_state(initial_threshold)
                }
            },
            Ok(None) => Self::fresh_state(initial_threshold),
            Err(e) => {
                warn!(
                    "Failed to read learning state from SQL, using defaults: {}",
                    e
                );
                Self::fresh_state(initial_threshold)
            }
        };

        Self {
            state,
            min_threshold,
            max_threshold,
            min_outcomes,
            repo: Some(repo),
        }
    }

    /// Create an in-memory instance (for tests without PostgreSQL).
    /// `save()` is a no-op.
    pub fn new_in_memory(
        initial_threshold: f32,
        min_threshold: f32,
        max_threshold: f32,
        min_outcomes: usize,
    ) -> Self {
        Self {
            state: Self::fresh_state(initial_threshold),
            min_threshold,
            max_threshold,
            min_outcomes,
            repo: None,
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
        apply_analysis_impl(
            &mut self.state,
            analysis,
            self.min_threshold,
            self.max_threshold,
            self.min_outcomes,
        )
    }

    /// Persist state to SQL (no-op for in-memory backend).
    pub async fn save(&self) -> Result<()> {
        let repo = match &self.repo {
            Some(r) => r,
            None => return Ok(()), // in-memory: no-op
        };
        let value = serde_json::to_value(&self.state)?;
        repo.set(ADAPTIVE_STATE_KEY, &value).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::types::EnrichmentStats;
    use std::collections::HashMap;

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

    // Unit tests for apply_analysis (pure logic, no DB needed)

    #[test]
    fn test_cold_start_protection() {
        let mut state = AdaptiveThresholdState {
            current_threshold: 0.7,
            last_analysis: None,
            threshold_history: Vec::new(),
            updated_at: Utc::now(),
        };

        let analysis = make_analysis(10, 0.5, 0.3); // too few outcomes
        let result = apply_analysis_impl(&mut state, &analysis, 0.4, 0.9, 50);
        assert!(result.is_none());
        assert!((state.current_threshold - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_threshold_adjustment_clamped_step() {
        let mut state = AdaptiveThresholdState {
            current_threshold: 0.7,
            last_analysis: None,
            threshold_history: Vec::new(),
            updated_at: Utc::now(),
        };

        let analysis = make_analysis(100, 0.4, 0.9);
        let result = apply_analysis_impl(&mut state, &analysis, 0.4, 0.9, 50);
        assert!(result.is_some());
        let new = result.unwrap();
        assert!((new - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_threshold_clamped_to_bounds() {
        let mut state = AdaptiveThresholdState {
            current_threshold: 0.42,
            last_analysis: None,
            threshold_history: Vec::new(),
            updated_at: Utc::now(),
        };

        let analysis = make_analysis(100, 0.1, 0.9);
        let result = apply_analysis_impl(&mut state, &analysis, 0.4, 0.9, 50);
        if let Some(new) = result {
            assert!(new >= 0.4);
        }
    }

    #[test]
    fn test_no_change_when_threshold_matches() {
        let mut state = AdaptiveThresholdState {
            current_threshold: 0.7,
            last_analysis: None,
            threshold_history: Vec::new(),
            updated_at: Utc::now(),
        };

        let analysis = make_analysis(100, 0.7, 0.9);
        let result = apply_analysis_impl(&mut state, &analysis, 0.4, 0.9, 50);
        assert!(result.is_none());
    }

    #[test]
    fn test_history_tracking() {
        let mut state = AdaptiveThresholdState {
            current_threshold: 0.7,
            last_analysis: None,
            threshold_history: Vec::new(),
            updated_at: Utc::now(),
        };

        let analysis = make_analysis(100, 0.5, 0.8);
        apply_analysis_impl(&mut state, &analysis, 0.4, 0.9, 50);

        assert_eq!(state.threshold_history.len(), 1);
        let change = &state.threshold_history[0];
        assert!((change.from - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_configurable_min_outcomes_blocks_adaptation() {
        let mut state = AdaptiveThresholdState {
            current_threshold: 0.7,
            last_analysis: None,
            threshold_history: Vec::new(),
            updated_at: Utc::now(),
        };

        let analysis = make_analysis(100, 0.5, 0.9);
        let result = apply_analysis_impl(&mut state, &analysis, 0.4, 0.9, 200);
        assert!(
            result.is_none(),
            "Should not adapt with fewer than min_outcomes"
        );
    }

    #[test]
    fn test_configurable_bounds_clamp_threshold() {
        let mut state = AdaptiveThresholdState {
            current_threshold: 0.7,
            last_analysis: None,
            threshold_history: Vec::new(),
            updated_at: Utc::now(),
        };

        let analysis = make_analysis(100, 0.2, 0.9);
        let result = apply_analysis_impl(&mut state, &analysis, 0.65, 0.75, 50);
        if let Some(new) = result {
            assert!(new >= 0.65, "Must not go below custom min_threshold");
            assert!(new <= 0.75, "Must not exceed custom max_threshold");
        }
    }
}
