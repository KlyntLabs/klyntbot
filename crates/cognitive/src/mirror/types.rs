use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorState {
    pub last_routing_snapshot: Option<RoutingSnapshot>,
    pub latest_trend_narrative: Option<TrendNarrative>,
    pub pending_snippets: Vec<NarrativeSnippet>,
    pub active_meta_rules: Vec<MetaRule>,
    pub pending_meta_rules: Vec<MetaRule>,
    pub latest_brain_version: Option<BrainVersion>,
    pub recent_trial_previews: Vec<TrialPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BrainVersion {
    pub version: u32,
    pub trial_id: Option<String>,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub promoted_at: Timestamp,
    #[specta(type = crate::specta_helpers::JsonValue)]
    pub params: serde_json::Value,
    pub reason: String,
    pub parent_version: Option<u32>,
    #[specta(type = crate::specta_helpers::JsonValue)]
    pub metrics_at_promotion: serde_json::Value,
    pub reverted: bool,
}

#[async_trait::async_trait]
pub trait AutotunerBridge: Send + Sync {
    async fn apply_champion(&self, params: serde_json::Value, reason: String)
        -> common::Result<()>;
    async fn current_champion_params(&self) -> common::Result<serde_json::Value>;
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RoutingSnapshot {
    pub id: Uuid,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub captured_at: Timestamp,
    pub window_hours: u8,
    pub total_messages: u32,
    pub distribution: HashMap<String, SkillRouteStats>,
    pub fallback_rate: f64,
    pub avg_routing_confidence: f64,
    pub low_confidence_count: u32,
    pub user_feedback: Option<UserFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkillRouteStats {
    pub count: u32,
    pub percentage: f64,
    pub avg_confidence: f64,
    pub top_triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrendNarrative {
    pub id: Uuid,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub generated_at: Timestamp,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub period_start: Timestamp,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub period_end: Timestamp,
    pub routing_summary: String,
    pub improvement_highlights: Vec<String>,
    pub experiment_summary: String,
    pub meta_rule_updates: Vec<String>,
    pub full_narrative: String,
    pub user_feedback: Option<UserFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeSnippet {
    pub id: Uuid,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub created_at: Timestamp,
    pub alert_type: MirrorAlertType,
    pub headline: String,
    pub body: String,
    pub suggested_action: Option<SuggestedAction>,
    pub user_feedback: Option<UserFeedback>,
    #[specta(type = Option<crate::specta_helpers::Timestamp>)]
    pub dismissed_at: Option<Timestamp>,
    pub coding_alert_kind: Option<String>,
    pub coding_alert_severity: Option<MirrorAlertSeverity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorResponse {
    pub answer: String,
    pub data_sources_used: Vec<String>,
    pub proposed_meta_rule: Option<MetaRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum UserFeedback {
    Helpful,
    NotHelpful,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub enum FeedbackTarget {
    Narrative,
    Snippet,
    Routing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum MirrorAlertType {
    RoutingDrift,
    TrialUnpromising,
    MetaRuleProposed,
    Coding,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub enum SuggestedAction {
    BoostSkill {
        skill: String,
    },
    ViewDetails,
    ApproveMetaRule {
        rule_id: Uuid,
    },
    DismissMetaRule {
        rule_id: Uuid,
    },
    KillTrial {
        trial_id: String,
    },
    ContinueTrial {
        trial_id: String,
    },
    RevertBrainVersion {
        version: u32,
    },
    #[serde(other)]
    Unknown,
}

/// Context assembled for the NarrativeHandler LLM call
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct NarrativeContext {
    #[specta(type = crate::specta_helpers::TimestampTuple)]
    pub period: (Timestamp, Timestamp),
    pub routing_snapshots: Vec<RoutingSnapshot>,
    pub correction_count: u32,
    pub top_skills_by_usage: Vec<(String, f64)>,
    pub past_narrative_feedback: Vec<UserFeedback>,
}

/// Output from the NarrativeHandler LLM call
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct GeneratedNarrative {
    pub full_narrative: String,
    pub routing_summary: String,
    pub improvement_highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum MirrorAlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl MirrorAlertSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            MirrorAlertSeverity::Low => "low",
            MirrorAlertSeverity::Medium => "medium",
            MirrorAlertSeverity::High => "high",
            MirrorAlertSeverity::Critical => "critical",
        }
    }
}

/// Alert emitted by signal sources when patterns are detected
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub enum MirrorAlert {
    RoutingDrift {
        skill: String,
        delta: f64,
        suggestion: String,
    },
    TrialUnpromising {
        trial_id: String,
        reason: String,
    },
    MetaRuleProposed {
        rule_id: Uuid,
        rule_text: String,
        source: MetaRuleSource,
    },
    Coding {
        kind: String,
        severity: MirrorAlertSeverity,
        #[specta(type = crate::specta_helpers::JsonValue)]
        payload: serde_json::Value,
    },
    CostThresholdCrossed {
        session_key: String,
        spend_usd: f64,
        ceiling_usd: f64,
        percent: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetaRule {
    pub id: Uuid,
    pub trigger_condition: String,
    pub action: MetaRuleAction,
    pub source: MetaRuleSource,
    pub effectiveness_score: f64,
    pub status: MetaRuleStatus,
    pub signal_count: u32,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub created_at: Timestamp,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum MetaRuleAction {
    AdjustRouting {
        skill: String,
        direction: String,
    },
    ForceClarification,
    SwitchMode {
        mode: String,
    },
    CreateExperiment {
        hypothesis: String,
    },
    SurfaceInsight {
        message: String,
    },
    Custom {
        #[specta(type = crate::specta_helpers::JsonValue)]
        payload: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum MetaRuleSource {
    UserCreated,
    ReflectionGenerated,
    CorrectionDerived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum MetaRuleStatus {
    Pending,
    Active,
    Disabled,
}

// ---------------------------------------------------------------------------
// Trial Preview types
// ---------------------------------------------------------------------------

/// 4-hour early evaluation of an autotuner trial
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrialPreview {
    pub id: Uuid,
    pub trial_id: String,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub started_at: Timestamp,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub preview_at: Timestamp,
    pub messages_scored: u32,
    pub early_signals: TrialEarlySignals,
    pub recommendation: PreviewRecommendation,
    pub narrative: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrialEarlySignals {
    pub correction_rate_delta: f64,
    pub confidence_trend: TrendDirection,
    pub dominant_skill_shift: Option<String>,
    #[serde(default)]
    pub messages_scored: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum PreviewRecommendation {
    Continue,
    Kill,
    NeedMoreData,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum TrendDirection {
    Rising,
    Falling,
    #[default]
    Stable,
}

/// Trait for querying early trial metrics at the 4-hour mark.
/// Defined in cognitive (L3-L4), implemented in app-core (L7).
#[async_trait::async_trait]
pub trait EarlyTrialEvaluator: Send + Sync {
    async fn evaluate_trial_early(
        &self,
        trial_id: &str,
        since: Timestamp,
    ) -> common::Result<TrialEarlySignals>;
}

// ---------------------------------------------------------------------------
// Task Focus types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskFocusSnapshot {
    pub id: Uuid,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub captured_at: Timestamp,
    pub window_hours: u8,
    pub focus_changes: u32,
    pub tasks_completed: u32,
    pub completion_rate: f64,
    pub longest_unfinished_secs: Option<i64>,
    pub top_tasks: Vec<(String, u32)>,
}

// ---------------------------------------------------------------------------
// Finance Drift types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FinanceDriftSnapshot {
    pub id: Uuid,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub captured_at: Timestamp,
    pub window_hours: u8,
    pub total_transactions: u32,
    pub over_budget_count: u32,
    pub per_category: HashMap<String, CategorySpend>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CategorySpend {
    pub total_amount: f64,
    pub transaction_count: u32,
    pub budget_alerts: u32,
}

// ---------------------------------------------------------------------------
// Coding Todo types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TodoSnapshot {
    pub id: Uuid,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub captured_at: Timestamp,
    pub window_hours: u8,
    pub status_changes: u32,
    pub cancellations: u32,
    pub plans_proposed: u32,
    pub plans_ratified: u32,
    pub blocked_reason_clusters: Vec<(String, u32)>,
}
