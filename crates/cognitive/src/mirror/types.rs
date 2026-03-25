use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorState {
    pub last_routing_snapshot: Option<RoutingSnapshot>,
    pub latest_trend_narrative: Option<TrendNarrative>,
    pub pending_snippets: Vec<NarrativeSnippet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingSnapshot {
    pub id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub window_hours: u8,
    pub total_messages: u32,
    pub distribution: HashMap<String, SkillRouteStats>,
    pub fallback_rate: f64,
    pub avg_routing_confidence: f64,
    pub low_confidence_count: u32,
    pub user_feedback: Option<UserFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRouteStats {
    pub count: u32,
    pub percentage: f64,
    pub avg_confidence: f64,
    pub top_triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendNarrative {
    pub id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub routing_summary: String,
    pub improvement_highlights: Vec<String>,
    pub experiment_summary: String,
    pub meta_rule_updates: Vec<String>,
    pub full_narrative: String,
    pub user_feedback: Option<UserFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeSnippet {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub alert_type: MirrorAlertType,
    pub headline: String,
    pub body: String,
    pub suggested_action: Option<SuggestedAction>,
    pub user_feedback: Option<UserFeedback>,
    pub dismissed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorResponse {
    pub answer: String,
    pub data_sources_used: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserFeedback {
    Helpful,
    NotHelpful,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackTarget {
    Narrative,
    Snippet,
    Routing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MirrorAlertType {
    RoutingDrift,
    TrialUnpromising,
    MetaRuleProposed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestedAction {
    BoostSkill { skill: String },
    ViewDetails,
    #[serde(other)]
    Unknown,
}

/// Context assembled for the NarrativeHandler LLM call
#[derive(Debug, Clone, Serialize)]
pub struct NarrativeContext {
    pub period: (DateTime<Utc>, DateTime<Utc>),
    pub routing_snapshots: Vec<RoutingSnapshot>,
    pub correction_count: u32,
    pub top_skills_by_usage: Vec<(String, f64)>,
    pub past_narrative_feedback: Vec<UserFeedback>,
}

/// Output from the NarrativeHandler LLM call
#[derive(Debug, Clone, Deserialize)]
pub struct GeneratedNarrative {
    pub full_narrative: String,
    pub routing_summary: String,
    pub improvement_highlights: Vec<String>,
}

/// Alert emitted by subscribers when patterns are detected
#[derive(Debug, Clone)]
pub enum MirrorAlert {
    RoutingDrift {
        skill: String,
        delta: f64,
        suggestion: String,
    },
}
