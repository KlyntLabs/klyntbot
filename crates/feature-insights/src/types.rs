//! Core types for the Insight Review V2 system.

use serde::{Deserialize, Serialize};

/// The 5-tab insight content stored as a JSON blob.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InsightContent {
    pub synthesis: Option<String>,
    pub gap_analysis: Option<String>,
    pub self_assessment: Option<String>,
    pub concept_map: Option<String>,
    pub perspectives: Option<String>,
}

/// Scope type for insight generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ScopeType {
    #[default]
    Backlinks,
    Semantic,
    Project,
    Manual,
}

/// Configuration for what context to include in insight generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeConfig {
    #[serde(default)]
    pub scope_type: ScopeType,
    #[serde(default = "default_radius")]
    pub radius: f64,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub include_cognitive: bool,
    #[serde(default)]
    pub deep_dive: bool,
    #[serde(default = "default_merge_threshold")]
    pub merge_threshold: f64,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        Self {
            scope_type: ScopeType::default(),
            radius: default_radius(),
            node_ids: Vec::new(),
            include_cognitive: true,
            deep_dive: false,
            merge_threshold: default_merge_threshold(),
        }
    }
}

fn default_radius() -> f64 {
    0.72
}
fn default_true() -> bool {
    true
}
fn default_merge_threshold() -> f64 {
    0.60
}

/// A single insight review version (row from `insight_reviews` table).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InsightReviewRow {
    pub id: String,
    pub note_id: String,
    pub version: i64,
    pub generated_at: String,
    pub content: String,
    pub input_hash: String,
    pub scope_config: String,
    pub persona_ids: String,
    pub parent_insight_id: Option<String>,
    pub token_cost_usd: Option<f64>,
    pub superseded_at: Option<String>,
}

/// A progress snapshot for a specific insight version.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProgressSnapshotRow {
    pub id: String,
    pub insight_review_id: String,
    pub version: i64,
    pub flashcard_success: f64,
    pub semantic_drift: f64,
    pub gap_closure: f64,
    pub quiz_score: f64,
    pub overall_progress: f64,
    pub computed_at: String,
}

/// Progress weights for the composite score (configurable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressWeights {
    pub flashcard: f64,
    pub drift: f64,
    pub gap: f64,
    pub quiz: f64,
}

impl Default for ProgressWeights {
    fn default() -> Self {
        Self {
            flashcard: 0.40,
            drift: 0.25,
            gap: 0.20,
            quiz: 0.15,
        }
    }
}
