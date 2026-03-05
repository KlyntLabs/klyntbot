use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: Option<i64>,
    pub app_name: String,
    pub window_title: Option<String>,
    pub bundle_id: Option<String>,
    pub url: Option<String>,
    pub category_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub is_idle: bool,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryType {
    Productive,
    Neutral,
    Distracting,
}

impl std::fmt::Display for CategoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Productive => write!(f, "productive"),
            Self::Neutral => write!(f, "neutral"),
            Self::Distracting => write!(f, "distracting"),
        }
    }
}

impl std::str::FromStr for CategoryType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "productive" => Ok(Self::Productive),
            "neutral" => Ok(Self::Neutral),
            "distracting" => Ok(Self::Distracting),
            _ => Err(common::ToolError::InvalidParams(format!(
                "unknown category type: {s}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCategory {
    pub id: String,
    pub name: String,
    pub category_type: CategoryType,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub rules: Option<CategoryRules>,
    pub is_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRules {
    pub app_names: Vec<String>,
    pub bundle_ids: Vec<String>,
    pub url_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Focus,
    Pomodoro,
    Break,
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Focus => write!(f, "focus"),
            Self::Pomodoro => write!(f, "pomodoro"),
            Self::Break => write!(f, "break"),
        }
    }
}

impl std::str::FromStr for SessionType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "focus" => Ok(Self::Focus),
            "pomodoro" => Ok(Self::Pomodoro),
            "break" => Ok(Self::Break),
            _ => Err(common::ToolError::InvalidParams(format!(
                "unknown session type: {s}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSession {
    pub id: String,
    pub action_id: Option<String>,
    pub project_id: Option<String>,
    pub session_type: SessionType,
    pub target_mins: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub actual_mins: Option<i64>,
    pub interruptions: i64,
    pub distraction_events: Vec<DistractionEvent>,
    pub quality_score: Option<f64>,
    pub completed: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistractionEvent {
    pub timestamp: DateTime<Utc>,
    pub app_name: String,
    pub duration_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySummary {
    pub date: String,
    pub total_active_secs: i64,
    pub total_focus_secs: i64,
    pub total_break_secs: i64,
    pub total_idle_secs: i64,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub focus_sessions_count: i64,
    pub avg_session_quality: Option<f64>,
    pub interruptions_count: i64,
    pub context_switches: i64,
    pub top_apps: Vec<AppUsage>,
    pub top_categories: Vec<CategoryUsage>,
    pub productivity_score: Option<f64>,
    pub ai_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsage {
    pub app_name: String,
    pub duration_secs: i64,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUsage {
    pub category: String,
    pub duration_secs: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NudgeType {
    BreakReminder,
    FocusSuggestion,
    DailySummary,
    BurnoutAlert,
}

impl std::fmt::Display for NudgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BreakReminder => write!(f, "break_reminder"),
            Self::FocusSuggestion => write!(f, "focus_suggestion"),
            Self::DailySummary => write!(f, "daily_summary"),
            Self::BurnoutAlert => write!(f, "burnout_alert"),
        }
    }
}

impl std::str::FromStr for NudgeType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "break_reminder" => Ok(Self::BreakReminder),
            "focus_suggestion" => Ok(Self::FocusSuggestion),
            "daily_summary" => Ok(Self::DailySummary),
            "burnout_alert" => Ok(Self::BurnoutAlert),
            _ => Err(common::ToolError::InvalidParams(format!(
                "unknown nudge type: {s}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NudgeRecord {
    pub id: Option<i64>,
    pub nudge_type: NudgeType,
    pub message: String,
    pub channel: Option<String>,
    pub acknowledged: bool,
    pub created_at: DateTime<Utc>,
}

impl NudgeRecord {
    pub fn new(nudge_type: NudgeType, message: String, created_at: DateTime<Utc>) -> Self {
        Self {
            id: None,
            nudge_type,
            message,
            channel: None,
            acknowledged: false,
            created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityScore {
    pub date: String,
    pub overall: f64,
    pub productive_ratio_score: f64,
    pub focus_quality_score: f64,
    pub distraction_score: f64,
    pub continuity_score: f64,
}
