// IMPORTANT: context_engine is L3, cognitive is L5. We define a local
// UserSituationSnapshot instead of importing cognitive::UserSituation.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct RewriteResult {
    pub enriched_query: String,
    pub confidence: f32,
    pub source: RewriteSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteSource {
    Heuristic,
    Llm,
}

#[derive(Debug, Clone, Default)]
pub struct UserSituationSnapshot {
    pub energy_level: f64,
    pub focus_state: f64,
    pub deadline_pressure: f64,
    pub distraction_risk: f64,
}

#[derive(Debug, Clone, Default)]
pub struct RetrievalContext {
    pub active_skill: Option<String>,
    pub active_task: Option<ActiveTaskContext>,
    pub recent_user_messages: Vec<String>,
    pub situation: Option<UserSituationSnapshot>,
    pub active_view: Option<ActiveView>,
    pub recent_correction: Option<CorrectionContext>,
}

#[derive(Debug, Clone)]
pub struct ActiveTaskContext {
    pub title: String,
    pub project_name: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveView {
    pub dashboard: String,
    pub focused_entity: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CorrectionContext {
    pub rejected_topic: String,
    pub corrected_to: String,
}
