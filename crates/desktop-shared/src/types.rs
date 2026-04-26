use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    P1,
    P2,
    P3,
    P4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Todo,
    Doing,
    Done,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AreaFilter {
    All,
    Work,
    Personal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    Table,
    Board,
    Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SidebarItem {
    Chat,
    Tasks,
    Okr,
    Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityKind {
    Task,
    Project,
    Objective,
    Area,
    KeyResult,
    FocusSession,
    Productivity,
    Note,
    Notebook,
    Finance,
    Source,
    Conversation,
    MirrorSnippet,
    BrainVersion,
    PendingMemory,
    CodingFact,
    CodingEpisode,
}

impl EntityKind {
    /// Parse from a string (case-insensitive). Returns None for unknown kinds.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "task" | "action" => Some(Self::Task),
            "project" => Some(Self::Project),
            "objective" => Some(Self::Objective),
            "area" => Some(Self::Area),
            "key_result" | "keyresult" => Some(Self::KeyResult),
            "focus_session" | "focussession" => Some(Self::FocusSession),
            "productivity" => Some(Self::Productivity),
            "note" => Some(Self::Note),
            "notebook" => Some(Self::Notebook),
            "finance"
            | "finance_account"
            | "finance_transaction"
            | "finance_budget"
            | "finance_goal"
            | "finance_liability"
            | "finance_portfolio"
            | "finance_investment" => Some(Self::Finance),
            "source" => Some(Self::Source),
            "conversation" => Some(Self::Conversation),
            "mirrorsnippet" | "mirror_snippet" => Some(Self::MirrorSnippet),
            "brainversion" | "brain_version" => Some(Self::BrainVersion),
            "pending_memory" | "pendingmemory" => Some(Self::PendingMemory),
            "coding_fact" | "codingfact" => Some(Self::CodingFact),
            "coding_episode" | "codingepisode" => Some(Self::CodingEpisode),
            _ => None,
        }
    }
}

// ── Cron / Automations ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobResponse {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub origin: String,
    pub schedule: serde_json::Value,
    pub payload: CronPayloadResponse,
    pub state: CronJobStateResponse,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub delete_after_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronPayloadResponse {
    pub kind: String,
    pub message: String,
    pub deliver: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobStateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobCreateParams {
    pub name: String,
    pub schedule: serde_json::Value,
    pub message: String,
    #[serde(default)]
    pub deliver: bool,
    pub channel: Option<String>,
    pub to: Option<String>,
    #[serde(default)]
    pub delete_after_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub schedule: Option<serde_json::Value>,
    pub message: Option<String>,
    pub deliver: Option<bool>,
    pub channel: Option<Option<String>>,
    pub to: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronStatusResponse {
    pub enabled: bool,
    pub jobs: usize,
    pub next_wake_at_ms: Option<i64>,
}


#[cfg(test)]
mod phase4_kind_tests {
    use super::*;

    #[test]
    fn coding_fact_serializes_camel_case() {
        let v = serde_json::to_value(EntityKind::CodingFact).unwrap();
        assert_eq!(v, serde_json::json!("codingFact"));
    }

    #[test]
    fn coding_episode_serializes_camel_case() {
        let v = serde_json::to_value(EntityKind::CodingEpisode).unwrap();
        assert_eq!(v, serde_json::json!("codingEpisode"));
    }

    #[test]
    fn parse_coding_kinds() {
        assert!(matches!(EntityKind::parse("coding_fact"), Some(EntityKind::CodingFact)));
        assert!(matches!(EntityKind::parse("codingfact"), Some(EntityKind::CodingFact)));
        assert!(matches!(EntityKind::parse("coding_episode"), Some(EntityKind::CodingEpisode)));
        assert!(matches!(EntityKind::parse("codingepisode"), Some(EntityKind::CodingEpisode)));
    }
}
