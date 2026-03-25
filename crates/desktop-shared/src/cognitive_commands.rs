use serde::{Deserialize, Serialize};

// ── Memory DTOs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticFactResponse {
    pub id: String,
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub stability: f64,
    pub retrievability: f64,
    pub last_accessed: Option<String>,
    pub access_count: i64,
    pub status: String, // "active" | "superseded" | "archived"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodicMemoryResponse {
    pub id: String,
    pub domain: String,
    pub content: String,
    pub summary: Option<String>,
    pub importance: f64,
    pub occurred_at: String,
    pub recorded_at: String,
    pub stability: f64,
    pub access_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProceduralRuleResponse {
    pub id: String,
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
    pub source: String,
    pub signal_count: i64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserModelSummaryResponse {
    pub identity_count: usize,
    pub energy_count: usize,
    pub work_count: usize,
    pub finance_count: usize,
    pub learning_count: usize,
    pub preferences_count: usize,
    pub identity_preview: Vec<String>,
    pub energy_preview: Vec<String>,
    pub work_preview: Vec<String>,
    pub finance_preview: Vec<String>,
    pub learning_preview: Vec<String>,
    pub preferences_preview: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatsResponse {
    pub active_facts: usize,
    pub archived_facts: u64,
    pub episodic_count: usize,
    pub rules_count: usize,
    pub last_compaction: Option<String>,
}

// ── Coaching DTOs ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSituationResponse {
    pub energy_level: f64,
    pub focus_state: f64,
    pub deadline_pressure: f64,
    pub distraction_risk: f64,
    pub coaching_receptivity: f64,
    pub task_avoidance_detected: bool,
    pub hours_active_today: f64,
    pub mins_since_break: f64,
    pub hour_of_day: u32,
    pub recent_context_switches: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalResponse {
    pub event_type: String,
    pub timestamp: String,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerConditionResponse {
    pub name: String,
    pub cooldown_remaining_secs: i64,
    pub last_fired: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalWindowResponse {
    pub window_size: usize,
    pub signals: Vec<SignalResponse>,
    pub triggers: Vec<TriggerConditionResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedPatternResponse {
    pub name: String,
    pub confidence: f64,
    pub signal_count: i32,
    pub description: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveredInterventionResponse {
    pub id: String,
    pub intervention_type: String,
    pub message: String,
    pub trigger_name: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyFeedbackResponse {
    pub strategy_type: String,
    pub domain: String,
    pub times_used: i32,
    pub acceptance_rate: f64,
    pub effectiveness: f64,
    pub behavioral_positive: i32,
    pub behavioral_negative: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterStatusResponse {
    pub hourly_count: usize,
    pub hourly_limit: usize,
    pub daily_count: usize,
    pub daily_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterventionLogResponse {
    pub id: String,
    pub intervention_type: String,
    pub message: String,
    pub trigger_name: String,
    pub feedback: Option<String>,
    pub delivered_at: String,
    pub feedback_at: Option<String>,
}

// ── Events DTOs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEventPayload {
    pub event_type: String,
    pub salience: String, // "extract" | "accumulate" | "discard"
    pub domain: String,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

// ── System DTOs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentStatusResponse {
    pub name: String,
    pub status: String,       // "wired" | "built" | "stub"
    pub handler_type: String, // "heuristic" | "llm" | "n/a"
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatusResponse {
    pub domain_bus_subscribers: usize,
    pub domain_bus_published: u64,
    pub background_service_running: bool,
    pub background_events_processed: u64,
    pub active_facts: usize,
    pub episodic_count: usize,
    pub rules_count: usize,
    pub components: Vec<ComponentStatusResponse>,
}

// ── Mutation Params ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactCreateParams {
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactUpdateParams {
    pub object: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCreateParams {
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
}

// ── Knowledge Trust DTOs ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryHealthResponse {
    pub overall: f64,
    pub domains: Vec<DomainHealthEntry>,
    pub total_facts_90d: i64,
    pub fast_failures_90d: i64,
    pub trend_pct: Option<f64>,
    pub computed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainHealthEntry {
    pub domain: String,
    pub score: f64,
    pub total_facts: i64,
    pub active_facts: i64,
    pub fast_failures: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionResultResponse {
    pub archived_count: u64,
    pub deleted_episodic: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectionResultResponse {
    pub fact_updates: usize,
    pub rule_updates: usize,
    pub summary: String,
}
