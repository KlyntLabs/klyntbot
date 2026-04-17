//! AutotunerBridge implementation that delegates to AutoTunerOrchestrator.
//!
//! Dependency inversion: the `AutotunerBridge` trait is defined in `cognitive`
//! (L4) but implemented here in `agent` (L5) where the orchestrator lives.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use cognitive::services::reforge::{AutotunerBridge, Phase6Result, ValidatedTrial};
use common::TrialParams;
use storage::rows::trial::{ExperimentRow, TrialRow};
use tracing::{info, warn};

use crate::autotuner::AutoTunerOrchestrator;

pub struct AgentAutotunerBridge {
    orchestrator: Arc<AutoTunerOrchestrator>,
    nightly_cycle: Arc<autotuner::NightlyCycle>,
    domain_event_bus: Option<Arc<bus::DomainEventBus>>,
}

impl AgentAutotunerBridge {
    pub fn new(
        orchestrator: Arc<AutoTunerOrchestrator>,
        nightly_cycle: Arc<autotuner::NightlyCycle>,
        domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> Self {
        Self {
            orchestrator,
            nightly_cycle,
            domain_event_bus,
        }
    }
}

#[async_trait]
impl AutotunerBridge for AgentAutotunerBridge {
    async fn run_evaluation(&self) -> common::Result<Phase6Result> {
        let champion = self.orchestrator.champion().await;
        let cycle_result = self
            .nightly_cycle
            .run_evaluation_and_promotion(&champion)
            .await?;

        let mut result = Phase6Result::default();
        result.evaluated_count = cycle_result.evaluated_trials.len();

        // Handle promotion
        if let Some((trial_id, trial_result, params)) = cycle_result.promotion {
            let improvement_pct = if champion.baseline_metrics.correction_rate > 0.0 {
                (champion.baseline_metrics.correction_rate - trial_result.correction_rate)
                    / champion.baseline_metrics.correction_rate
                    * 100.0
            } else {
                0.0
            };
            let affected_params = autotuner::affected_param_names(&champion.params, &params);

            let new_champion = autotuner::Champion {
                trial_id: Some(trial_id),
                params,
                promoted_at: common::time::bridge::chrono_to_jiff(chrono::Utc::now()),
                baseline_metrics: trial_result.clone(),
                reason_for_promotion: format!("Promoted by Reforge Phase 6 (trial {trial_id})"),
                impact_summary: format!(
                    "Evaluated {} trials, {} passed constraints",
                    result.evaluated_count,
                    result
                        .evaluated_count
                        .saturating_sub(cycle_result.failed_constraints.len()),
                ),
                consecutive_regression_days: 0,
            };
            self.orchestrator.update_champion(new_champion).await;

            // Emit domain event for promotion
            if let Some(ref bus) = self.domain_event_bus {
                bus.publish(bus::DomainEvent::AutotunerDecision {
                    trial_id: trial_id.to_string(),
                    verdict: autotuner::TrialStatus::Promoted.as_str().to_string(),
                    improvement_pct,
                    affected_params,
                });
            }

            result.promoted = true;
            result.promotion_summary = Some(format!(
                "Trial {trial_id} promoted — correction_rate: {:.1}%",
                trial_result.correction_rate * 100.0
            ));
        }

        // Handle regression: increment counter, rollback if threshold reached.
        if cycle_result.regression {
            result.regression = true;
            let rollback_threshold = 3u8; // matches AutoTunerConfig default
            self.orchestrator
                .handle_regression(rollback_threshold, self.domain_event_bus.as_ref())
                .await;
        } else {
            // Reset counter on a healthy day.
            self.orchestrator.reset_regression_counter().await;
        }

        result.failed_constraints = cycle_result
            .failed_constraints
            .iter()
            .map(|(id, failures)| format!("{id}: {}", failures.join(", ")))
            .collect();

        Ok(result)
    }

    async fn create_trials(&self, suggestions: Vec<ValidatedTrial>) -> common::Result<u32> {
        let trial_repo = self.orchestrator.trial_repo();
        let champion_params = self
            .orchestrator
            .try_current_champion_params()
            .unwrap_or_default();

        // Create an experiment row to group these trials
        let experiment_id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();

        let hypothesis = suggestions
            .first()
            .map(|s| s.hypothesis.clone())
            .unwrap_or_else(|| "Reforge-generated experiment".to_string());

        let experiment_row = ExperimentRow {
            id: experiment_id.to_string(),
            hypothesis,
            trend_analysis: "Generated by Reforge Phase 3 Review".to_string(),
            recommendation_for_next: String::new(),
            created_at: now.clone(),
        };
        trial_repo
            .create_experiment(&experiment_row)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("create experiment: {e}")))?;

        let mut count = 0u32;
        for suggestion in &suggestions {
            // Merge overrides with champion defaults
            let mut params = champion_params.clone();
            apply_overrides_to_params(&mut params, &suggestion.params);

            let trial_id = uuid::Uuid::new_v4();
            let params_json = serde_json::to_string(&params).unwrap_or_default();
            let trial_row = TrialRow {
                id: trial_id.to_string(),
                experiment_id: experiment_id.to_string(),
                params: params_json,
                generation_reasoning: suggestion.hypothesis.clone(),
                status: autotuner::TrialStatus::Active.as_str().to_string(),
                created_at: now.clone(),
                completed_at: None,
                result: None,
            };

            match trial_repo.create_trial(&trial_row).await {
                Ok(()) => {
                    info!(
                        trial_id = %trial_id,
                        pace = %suggestion.pace,
                        "Reforge Phase 6: created trial"
                    );
                    count += 1;
                }
                Err(e) => {
                    warn!("Reforge Phase 6: failed to create trial: {e}");
                }
            }
        }

        Ok(count)
    }

    fn champion_params_map(&self) -> HashMap<String, f64> {
        let params = self.orchestrator.try_current_champion_params();
        match params {
            Some(p) => params_to_map(&p),
            None => HashMap::new(),
        }
    }

    async fn active_trial_count(&self) -> u32 {
        self.orchestrator
            .trial_repo()
            .count_active()
            .await
            .unwrap_or(0)
    }

    async fn expire_stale_trials(&self) -> u32 {
        match self.orchestrator.trial_repo().expire_stale(7, 20).await {
            Ok(expired) => {
                if expired > 0 {
                    info!("Reforge Phase 6: expired {expired} stale trial(s)");
                }
                expired
            }
            Err(e) => {
                warn!("Reforge Phase 6: failed to expire stale trials: {e}");
                0
            }
        }
    }
}

/// Convert TrialParams struct to a flat HashMap for guardrail comparison.
fn params_to_map(params: &TrialParams) -> HashMap<String, f64> {
    let mut map = HashMap::new();

    macro_rules! insert_f64 {
        ($map:expr, $name:literal, $val:expr) => {
            if let Some(v) = $val {
                $map.insert($name.to_string(), v);
            }
        };
    }
    macro_rules! insert_usize {
        ($map:expr, $name:literal, $val:expr) => {
            if let Some(v) = $val {
                $map.insert($name.to_string(), v as f64);
            }
        };
    }

    // Phase 1: SkillRouter
    insert_f64!(map, "skill_keyword_weight", params.skill_keyword_weight);
    insert_f64!(map, "skill_semantic_weight", params.skill_semantic_weight);
    insert_f64!(
        map,
        "skill_activation_threshold",
        params.skill_activation_threshold
    );
    insert_f64!(
        map,
        "heuristic_confidence_threshold",
        params.heuristic_confidence_threshold
    );
    if let Some(v) = params.llm_classifier_timeout_ms {
        map.insert("llm_classifier_timeout_ms".to_string(), v as f64);
    }

    // Phase 1: Retrieval weights
    insert_f64!(
        map,
        "relevance_weight_semantic",
        params.relevance_weight_semantic
    );
    insert_f64!(
        map,
        "relevance_weight_retrievability",
        params.relevance_weight_retrievability
    );
    insert_f64!(
        map,
        "relevance_weight_situation",
        params.relevance_weight_situation
    );

    // Phase 2: FSRS
    insert_f64!(map, "fsrs_desired_retention", params.fsrs_desired_retention);

    // Phase 2: Accumulation
    insert_usize!(
        map,
        "accumulate_promote_threshold",
        params.accumulate_promote_threshold
    );
    insert_usize!(map, "accumulate_min_days", params.accumulate_min_days);

    // Phase 2: Vector search
    insert_usize!(map, "vector_top_k", params.vector_top_k);
    insert_f64!(map, "min_similarity", params.min_similarity);

    // Phase 2: Remaining weights
    insert_f64!(
        map,
        "relevance_weight_importance",
        params.relevance_weight_importance
    );
    insert_f64!(
        map,
        "relevance_weight_frequency",
        params.relevance_weight_frequency
    );
    insert_f64!(
        map,
        "relevance_weight_temporal",
        params.relevance_weight_temporal
    );

    // Phase 3: Rewriting
    insert_f64!(
        map,
        "rewrite_confidence_threshold",
        params.rewrite_confidence_threshold
    );
    insert_usize!(map, "rewrite_max_signals", params.rewrite_max_signals);
    insert_usize!(
        map,
        "rewrite_min_enrichment_length",
        params.rewrite_min_enrichment_length
    );

    // Phase 4: Hierarchical note retrieval
    insert_f64!(
        map,
        "relevance_weight_hierarchy",
        params.relevance_weight_hierarchy
    );
    insert_f64!(
        map,
        "relevance_weight_path_coherence",
        params.relevance_weight_path_coherence
    );
    insert_usize!(map, "tree_top_k", params.tree_top_k);
    insert_f64!(map, "tree_min_similarity", params.tree_min_similarity);
    insert_f64!(map, "hybrid_bias", params.hybrid_bias);

    // Phase 5: Community graph
    insert_f64!(
        map,
        "relevance_weight_community",
        params.relevance_weight_community
    );
    insert_f64!(
        map,
        "relevance_weight_cross_note",
        params.relevance_weight_cross_note
    );
    insert_usize!(map, "community_top_k", params.community_top_k);
    insert_f64!(
        map,
        "community_min_similarity",
        params.community_min_similarity
    );

    map
}

/// Apply HashMap overrides onto a TrialParams struct.
fn apply_overrides_to_params(params: &mut TrialParams, overrides: &HashMap<String, f64>) {
    for (key, &value) in overrides {
        match key.as_str() {
            // Phase 1: SkillRouter
            "skill_keyword_weight" => params.skill_keyword_weight = Some(value),
            "skill_semantic_weight" => params.skill_semantic_weight = Some(value),
            "skill_activation_threshold" => params.skill_activation_threshold = Some(value),
            "heuristic_confidence_threshold" => params.heuristic_confidence_threshold = Some(value),
            "llm_classifier_timeout_ms" => params.llm_classifier_timeout_ms = Some(value as u64),

            // Phase 1: Retrieval weights
            "relevance_weight_semantic" => params.relevance_weight_semantic = Some(value),
            "relevance_weight_retrievability" => {
                params.relevance_weight_retrievability = Some(value)
            }
            "relevance_weight_situation" => params.relevance_weight_situation = Some(value),

            // Phase 2: FSRS
            "fsrs_desired_retention" => params.fsrs_desired_retention = Some(value),

            // Phase 2: Accumulation (f64 → usize)
            "accumulate_promote_threshold" => {
                params.accumulate_promote_threshold = Some(value as usize)
            }
            "accumulate_min_days" => params.accumulate_min_days = Some(value as usize),

            // Phase 2: Vector search
            "vector_top_k" => params.vector_top_k = Some(value as usize),
            "min_similarity" => params.min_similarity = Some(value),

            // Phase 2: Remaining weights
            "relevance_weight_importance" => params.relevance_weight_importance = Some(value),
            "relevance_weight_frequency" => params.relevance_weight_frequency = Some(value),
            "relevance_weight_temporal" => params.relevance_weight_temporal = Some(value),

            // Phase 3: Rewriting
            "rewrite_confidence_threshold" => params.rewrite_confidence_threshold = Some(value),
            "rewrite_max_signals" => params.rewrite_max_signals = Some(value as usize),
            "rewrite_min_enrichment_length" => {
                params.rewrite_min_enrichment_length = Some(value as usize)
            }

            // Phase 4: Hierarchical note retrieval
            "relevance_weight_hierarchy" => params.relevance_weight_hierarchy = Some(value),
            "relevance_weight_path_coherence" => {
                params.relevance_weight_path_coherence = Some(value)
            }
            "tree_top_k" => params.tree_top_k = Some(value as usize),
            "tree_min_similarity" => params.tree_min_similarity = Some(value),
            "hybrid_bias" => params.hybrid_bias = Some(value),

            // Phase 5: Community graph
            "relevance_weight_community" => params.relevance_weight_community = Some(value),
            "relevance_weight_cross_note" => params.relevance_weight_cross_note = Some(value),
            "community_top_k" => params.community_top_k = Some(value as usize),
            "community_min_similarity" => params.community_min_similarity = Some(value),

            _ => {
                warn!("Unknown param override: {key}");
            }
        }
    }
}
