use common::TrialParams;
use serde::{Deserialize, Serialize};

use crate::trial::TrialResult;

/// All data needed to build a variant-generation prompt for the LLM.
#[derive(Debug, Clone)]
pub struct GenerationContext {
    /// The current champion's parameter set.
    pub champion_params: TrialParams,
    /// The current champion's measured metrics.
    pub champion_metrics: TrialResult,
    /// Recent trial history (newest first, trimmed before passing in).
    pub recent_trials: Vec<TrialSummaryForPrompt>,
    /// A human-readable 7-day trend summary produced by the trend analyzer.
    pub trend_summary: String,
    /// A summary of observed user behavior patterns from the activity log.
    pub behavioral_context: String,
    /// A snapshot of relevant user memory / episodic context.
    pub memory_snapshot: String,
    /// The recommendation written at the end of the previous experiment cycle, if any.
    pub previous_recommendation: Option<String>,
    /// How aggressively to explore the parameter space.
    /// One of `"conservative"`, `"balanced"`, or `"bold"`.
    pub experiment_pace: String,
}

/// A compact representation of one trial used inside the generation prompt.
#[derive(Debug, Clone)]
pub struct TrialSummaryForPrompt {
    /// Short identifier shown to the LLM (e.g. the trial UUID prefix).
    pub id: String,
    /// Serialised snapshot of the parameter set that was tested.
    pub params: String,
    /// Outcome label: `"promoted"`, `"reverted"`, `"completed"`, etc.
    pub outcome: String,
    /// Key metrics measured during the trial, formatted for readability.
    pub result: String,
    /// The reasoning or hypothesis recorded when the trial was generated.
    pub reasoning: String,
}

/// The JSON object the LLM must return in response to the generation prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    /// The suggested parameter variants for the next experiment cycle.
    pub variants: Vec<VariantSuggestion>,
    /// LLM's analysis of the current performance trend.
    pub trend_analysis: String,
    /// A free-text recommendation to be stored and passed back next cycle.
    pub recommendation_for_next_cycle: String,
}

/// One parameter variant suggested by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantSuggestion {
    /// One-sentence hypothesis describing what this variant is testing.
    pub hypothesis: String,
    /// The concrete parameter values to apply during the trial.
    pub params: TrialParams,
    /// Why these values satisfy the promotion constraints.
    pub constraint_reasoning: String,
    /// Confidence label: `"low"`, `"medium"`, or `"high"`.
    pub confidence: String,
    /// Explanation of the confidence assessment.
    pub confidence_reasoning: String,
}

/// Build the full LLM prompt for variant generation from `ctx`.
///
/// The prompt describes the current champion, recent trial history, user
/// context, parameter bounds, promotion constraints, and instructs the LLM to
/// return a `GenerationResponse` JSON object with exactly 3 variants.
pub fn build_generation_prompt(ctx: &GenerationContext) -> String {
    let mut prompt = String::with_capacity(4096);

    // ---- 1. Role description ------------------------------------------------
    prompt.push_str(
        "You are the Klyntbot AutoTuner — an AI subsystem that optimises the routing and \
memory parameters of a personal AI assistant for one specific user. Your goal is to \
propose parameter variants that will be evaluated in shadow mode before any change \
reaches the live system. Every suggestion must respect the promotion constraints listed \
below; unsafe suggestions are silently discarded by the evaluator.\n\n",
    );

    // ---- 2. Current champion ------------------------------------------------
    prompt.push_str("## Current Champion Parameters\n\n");
    prompt.push_str("```json\n");
    prompt.push_str(
        &serde_json::to_string_pretty(&ctx.champion_params).unwrap_or_else(|_| "{}".to_string()),
    );
    prompt.push_str("\n```\n\n");

    prompt.push_str("### Champion Metrics\n\n");
    prompt.push_str(&format!(
        "- correction_rate:          {:.4}  (lower is better)\n",
        ctx.champion_metrics.correction_rate
    ));
    prompt.push_str(&format!(
        "- classification_accuracy:  {:.4}  (higher is better)\n",
        ctx.champion_metrics.classification_accuracy
    ));
    prompt.push_str(&format!(
        "- avg_tokens_per_message:   {:.1}\n",
        ctx.champion_metrics.avg_tokens_per_message
    ));
    prompt.push_str(&format!(
        "- avg_response_time_ms:     {:.1}\n",
        ctx.champion_metrics.avg_response_time_ms
    ));
    prompt.push_str(&format!(
        "- routing_stability:        {:.4}  (higher is better)\n",
        ctx.champion_metrics.routing_stability
    ));
    prompt.push_str(&format!(
        "- memory_relevance:         {:.4}  (higher is better)\n",
        ctx.champion_metrics.memory_relevance
    ));
    if let Some(sat) = ctx.champion_metrics.user_satisfaction {
        prompt.push_str(&format!("- user_satisfaction:        {sat:.4}\n"));
    }
    prompt.push('\n');

    // ---- 3. Recent trial history -------------------------------------------
    prompt.push_str("## Recent Trial History\n\n");
    if ctx.recent_trials.is_empty() {
        prompt.push_str("No previous trials recorded.\n\n");
    } else {
        for trial in &ctx.recent_trials {
            prompt.push_str(&format!("### Trial {}\n", trial.id));
            prompt.push_str(&format!("- **Status/Outcome:** {}\n", trial.outcome));
            prompt.push_str(&format!("- **Reasoning:** {}\n", trial.reasoning));
            prompt.push_str(&format!("- **Metrics:** {}\n", trial.result));
            prompt.push_str(&format!("- **Params:** {}\n", trial.params));
            prompt.push('\n');
        }
    }

    // ---- 4. 7-day trend -----------------------------------------------------
    prompt.push_str("## 7-Day Performance Trend\n\n");
    prompt.push_str(&ctx.trend_summary);
    prompt.push_str("\n\n");

    // ---- 5. User behavior patterns ------------------------------------------
    prompt.push_str("## User Behavior Patterns\n\n");
    prompt.push_str(&ctx.behavioral_context);
    prompt.push_str("\n\n");

    // ---- 6. User memory snapshot --------------------------------------------
    prompt.push_str("## User Memory Snapshot\n\n");
    prompt.push_str(&ctx.memory_snapshot);
    prompt.push_str("\n\n");

    // ---- 7. Previous cycle recommendation -----------------------------------
    if let Some(prev) = &ctx.previous_recommendation {
        prompt.push_str("## Previous Cycle Recommendation\n\n");
        prompt.push_str(prev);
        prompt.push_str("\n\n");
    }

    // ---- 8. Parameter bounds table -----------------------------------------
    prompt.push_str("## Parameter Bounds\n\n");
    prompt.push_str(
        "All suggested values must stay within the following bounds:\n\n\
| Parameter                      | Min   | Max   | Step  | Notes |\n\
|-------------------------------|-------|-------|-------|-------|\n\
| skill_keyword_weight           | 0.00  | 1.00  | 0.05  | Must satisfy: keyword + semantic = 1.0 |\n\
| skill_semantic_weight          | 0.00  | 1.00  | 0.05  | Must satisfy: keyword + semantic = 1.0 |\n\
| skill_activation_threshold     | 0.40  | 0.95  | 0.05  | |\n\
| heuristic_confidence_threshold | 0.50  | 0.95  | 0.05  | |\n\
| llm_classifier_timeout_ms      | 500   | 5000  | 250   | Integer |\n\
| relevance_weight_semantic      | 0.10  | 0.60  | 0.05  | |\n\
| relevance_weight_retrievability| 0.05  | 0.40  | 0.05  | |\n\
| relevance_weight_situation     | 0.05  | 0.40  | 0.05  | |\n\
| fsrs_desired_retention         | 0.70  | 0.99  | 0.01  | FSRS target retention for spaced repetition |\n\
| accumulate_promote_threshold   | 2     | 15    | 1     | Min observations before promoting to fact |\n\
| accumulate_min_days            | 1     | 10    | 1     | Min days of observation before promotion |\n\
| vector_top_k                   | 10    | 100   | 5     | Number of candidate vectors to retrieve |\n\
| min_similarity                 | 0.30  | 0.80  | 0.05  | Cosine similarity threshold for retrieval |\n\
| relevance_weight_importance    | 0.05  | 0.40  | 0.05  | Weight for fact importance in ranking |\n\
| relevance_weight_frequency     | 0.02  | 0.30  | 0.02  | Weight for access frequency in ranking |\n\
| relevance_weight_temporal      | 0.01  | 0.20  | 0.01  | Weight for temporal recency in ranking |\n\n",
    );

    // ---- 9. Promotion constraints ------------------------------------------
    prompt.push_str("## Promotion Constraints\n\n");
    prompt.push_str(
        "A variant is only promoted if ALL of the following hold after shadow evaluation:\n\n\
- `correction_rate` must improve by **≥ 5%** relative to champion\n\
- `avg_tokens_per_message` must not increase by **> 8%** relative to champion\n\
- `avg_response_time_ms` must not increase by **> 15%** relative to champion\n\
- `routing_stability` must not decrease by **> 10%** relative to champion\n\
- `memory_relevance` must not decrease by **> 5%** relative to champion\n\n\
Design your variants to satisfy these constraints. Include your constraint reasoning \
in the `constraint_reasoning` field of each variant.\n\n",
    );

    // ---- 10. Diversity instruction (varies by pace) ------------------------
    prompt.push_str("## Your Task\n\n");
    prompt.push_str(
        "Suggest exactly **3 parameter variants** for the next experiment cycle. \
For each variant, provide:\n\
- A one-sentence `hypothesis`\n\
- Concrete `params` (only set the fields you want to change from defaults; \
  use `null` for unchanged fields)\n\
- `constraint_reasoning` explaining why the variant should pass promotion\n\
- `confidence` label: `\"low\"`, `\"medium\"`, or `\"high\"`\n\
- `confidence_reasoning` explaining your confidence assessment\n\n",
    );

    let diversity_instruction = match ctx.experiment_pace.as_str() {
        "conservative" => {
            "**Diversity tiers for this cycle (pace = conservative):** \
All 3 variants should be conservative refinements — small, safe adjustments \
(≤ 10% change per parameter) that are highly likely to pass promotion constraints. \
Prioritise stability over exploration."
        }
        "bold" => {
            "**Diversity tiers for this cycle (pace = bold):** \
Suggest 2 bold variants (large parameter shifts, high exploration risk) and \
1 moderate variant (medium-sized adjustments). Bold variants may fail promotion \
constraints but help map the parameter space quickly."
        }
        _ => {
            // default: "balanced"
            "**Diversity tiers for this cycle (pace = balanced):** \
Suggest 1 conservative variant (small safe adjustment), \
1 moderate variant (medium-sized adjustment, balanced risk), and \
1 bold variant (large shift to explore a new region of parameter space)."
        }
    };
    prompt.push_str(diversity_instruction);
    prompt.push_str("\n\n");

    // ---- Output format instruction ------------------------------------------
    prompt.push_str(
        "## Output Format\n\n\
Respond with a single JSON object conforming to this schema — no markdown \
fences, no commentary outside the JSON:\n\n\
```\n\
{\n\
  \"variants\": [\n\
    {\n\
      \"hypothesis\": \"<string>\",\n\
      \"params\": { <TrialParams fields, null for unchanged> },\n\
      \"constraint_reasoning\": \"<string>\",\n\
      \"confidence\": \"low\" | \"medium\" | \"high\",\n\
      \"confidence_reasoning\": \"<string>\"\n\
    }\n\
  ],\n\
  \"trend_analysis\": \"<string>\",\n\
  \"recommendation_for_next_cycle\": \"<string>\"\n\
}\n\
```\n",
    );

    prompt
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn sample_champion_params() -> TrialParams {
        TrialParams {
            skill_keyword_weight: Some(0.60),
            skill_semantic_weight: Some(0.40),
            skill_activation_threshold: Some(0.70),
            heuristic_confidence_threshold: Some(0.75),
            llm_classifier_timeout_ms: Some(2000),
            relevance_weight_semantic: Some(0.30),
            relevance_weight_retrievability: Some(0.20),
            relevance_weight_situation: Some(0.25),
            ..Default::default()
        }
    }

    fn sample_champion_metrics() -> TrialResult {
        TrialResult {
            trial_id: Uuid::nil(),
            messages_scored: 300,
            correction_rate: 0.15,
            classification_accuracy: 0.88,
            avg_tokens_per_message: 450.0,
            avg_response_time_ms: 750.0,
            routing_stability: 0.92,
            memory_relevance: 0.82,
            user_satisfaction: Some(0.75),
        }
    }

    fn sample_context(pace: &str) -> GenerationContext {
        GenerationContext {
            champion_params: sample_champion_params(),
            champion_metrics: sample_champion_metrics(),
            recent_trials: vec![TrialSummaryForPrompt {
                id: "abc123".to_string(),
                params: r#"{"skill_keyword_weight": 0.55}"#.to_string(),
                outcome: "promoted".to_string(),
                result: "correction_rate: 0.14".to_string(),
                reasoning: "Test lower keyword weight".to_string(),
            }],
            trend_summary: "Stable correction rate over 7 days.".to_string(),
            behavioral_context: "User primarily sends task-management messages.".to_string(),
            memory_snapshot: "Recent episodes: project planning, daily standup.".to_string(),
            previous_recommendation: Some("Focus on routing stability next cycle.".to_string()),
            experiment_pace: pace.to_string(),
        }
    }

    #[test]
    fn parse_generation_response_valid_json() {
        let json = r#"{
            "variants": [
                {
                    "hypothesis": "Increase keyword weight to improve task routing.",
                    "params": {
                        "skill_keyword_weight": 0.65,
                        "skill_semantic_weight": 0.35,
                        "skill_activation_threshold": null,
                        "heuristic_confidence_threshold": null,
                        "llm_classifier_timeout_ms": null,
                        "relevance_weight_semantic": null,
                        "relevance_weight_retrievability": null,
                        "relevance_weight_situation": null
                    },
                    "constraint_reasoning": "Small +5% change; unlikely to regress.",
                    "confidence": "high",
                    "confidence_reasoning": "Similar changes promoted before."
                }
            ],
            "trend_analysis": "Correction rate has been stable.",
            "recommendation_for_next_cycle": "Explore memory relevance weights."
        }"#;

        let response: GenerationResponse =
            serde_json::from_str(json).expect("should parse valid GenerationResponse JSON");

        assert_eq!(response.variants.len(), 1);
        assert_eq!(response.variants[0].confidence, "high");
        assert_eq!(response.variants[0].params.skill_keyword_weight, Some(0.65));
        assert!(response.variants[0]
            .params
            .skill_activation_threshold
            .is_none());
        assert_eq!(response.trend_analysis, "Correction rate has been stable.");
        assert_eq!(
            response.recommendation_for_next_cycle,
            "Explore memory relevance weights."
        );
    }

    #[test]
    fn build_prompt_includes_champion_params() {
        let ctx = sample_context("balanced");
        let prompt = build_generation_prompt(&ctx);

        // Champion param values must appear in the prompt
        assert!(
            prompt.contains("0.6"),
            "prompt should contain keyword weight 0.6"
        );
        assert!(
            prompt.contains("0.4"),
            "prompt should contain semantic weight 0.4"
        );
        assert!(
            prompt.contains("0.15"),
            "prompt should contain champion correction_rate 0.15"
        );
        assert!(
            prompt.contains("0.88"),
            "prompt should contain champion classification_accuracy 0.88"
        );
        // Previous recommendation should appear
        assert!(
            prompt.contains("Focus on routing stability next cycle."),
            "prompt should include previous recommendation"
        );
        // Champion metrics section
        assert!(
            prompt.contains("correction_rate"),
            "prompt should mention correction_rate metric"
        );
        // Parameter bounds
        assert!(
            prompt.contains("skill_keyword_weight"),
            "prompt should include parameter bounds table"
        );
    }

    #[test]
    fn build_prompt_uses_pace_for_diversity() {
        let conservative_prompt = build_generation_prompt(&sample_context("conservative"));
        let balanced_prompt = build_generation_prompt(&sample_context("balanced"));
        let bold_prompt = build_generation_prompt(&sample_context("bold"));

        // Each pace should produce a distinct diversity instruction
        assert!(
            conservative_prompt.contains("pace = conservative"),
            "conservative prompt should mention its pace"
        );
        assert!(
            balanced_prompt.contains("pace = balanced"),
            "balanced prompt should mention its pace"
        );
        assert!(
            bold_prompt.contains("pace = bold"),
            "bold prompt should mention its pace"
        );

        // Conservative: no "bold" diversity language
        assert!(
            !conservative_prompt.contains("1 bold variant"),
            "conservative prompt should not mention bold variants"
        );
        // Balanced: all three tiers present
        assert!(
            balanced_prompt.contains("1 conservative variant"),
            "balanced prompt should mention conservative tier"
        );
        assert!(
            balanced_prompt.contains("1 moderate variant"),
            "balanced prompt should mention moderate tier"
        );
        assert!(
            balanced_prompt.contains("1 bold variant"),
            "balanced prompt should mention bold tier"
        );
        // Bold: 2 bold variants
        assert!(
            bold_prompt.contains("2 bold variants"),
            "bold prompt should mention 2 bold variants"
        );
    }
}
