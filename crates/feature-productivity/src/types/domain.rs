use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityProject {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub url_patterns: Vec<String>,
    pub color: Option<String>,
    pub is_auto_detected: bool,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUsage {
    pub project_id: String,
    pub display_name: String,
    pub duration_secs: i64,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: Option<i64>,
    pub app_name: String,
    pub window_title: Option<String>,
    pub site_name: Option<String>,
    pub bundle_id: Option<String>,
    pub url: Option<String>,
    pub category_id: Option<String>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub duration_secs: Option<i64>,
    pub is_idle: bool,
    pub metadata: Option<String>,
    pub project_id: Option<String>,
    pub focus_session_id: Option<String>,
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
            _ => {
                Err(common::ToolError::InvalidParams(format!("unknown category type: {s}")).into())
            }
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
            _ => Err(common::ToolError::InvalidParams(format!("unknown session type: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ai_core_macros::AiEntity)]
#[serde(rename_all = "camelCase")]
#[ai(entity_type = "focus_session", embed_on = ["notes"])]
pub struct FocusSession {
    pub id: String,
    pub action_id: Option<String>,
    pub project_id: Option<String>,
    pub session_type: SessionType,
    pub target_mins: Option<i64>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub actual_mins: Option<i64>,
    pub interruptions: i64,
    pub distraction_events: Vec<DistractionEvent>,
    pub quality_score: Option<f64>,
    pub completed: bool,
    pub notes: Option<String>,
    pub source: SessionSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistractionEvent {
    pub timestamp: Timestamp,
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
    pub top_projects: Vec<ProjectUsage>,
    pub productivity_score: Option<f64>,
    pub ai_summary: Option<String>,
    pub deep_work_blocks: i64,
    pub deep_work_secs: i64,
    pub avg_recovery_secs: Option<f64>,
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
    pub category_id: String,
    pub category: String,
    pub category_type: String,
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
            _ => Err(common::ToolError::InvalidParams(format!("unknown nudge type: {s}")).into()),
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
    pub created_at: Timestamp,
}

impl NudgeRecord {
    pub fn new(nudge_type: NudgeType, message: String, created_at: Timestamp) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalType {
    Daily,
    Weekly,
}

impl std::fmt::Display for GoalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daily => write!(f, "daily"),
            Self::Weekly => write!(f, "weekly"),
        }
    }
}

impl std::str::FromStr for GoalType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            _ => Err(common::ToolError::InvalidParams(format!("unknown goal type: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalMetric {
    ProductiveHours,
    FocusSessions,
    ProductivityScore,
    MaxDistractingMins,
    ProjectHours,
}

impl GoalMetric {
    /// Returns the comparison operator for display (">=" for most, "<=" for max-based metrics).
    pub fn operator(&self) -> &'static str {
        match self {
            Self::MaxDistractingMins => "<=",
            _ => ">=",
        }
    }
}

impl std::fmt::Display for GoalMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProductiveHours => write!(f, "productive_hours"),
            Self::FocusSessions => write!(f, "focus_sessions"),
            Self::ProductivityScore => write!(f, "productivity_score"),
            Self::MaxDistractingMins => write!(f, "max_distracting_mins"),
            Self::ProjectHours => write!(f, "project_hours"),
        }
    }
}

impl std::str::FromStr for GoalMetric {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "productive_hours" => Ok(Self::ProductiveHours),
            "focus_sessions" => Ok(Self::FocusSessions),
            "productivity_score" => Ok(Self::ProductivityScore),
            "max_distracting_mins" => Ok(Self::MaxDistractingMins),
            "project_hours" => Ok(Self::ProjectHours),
            _ => Err(common::ToolError::InvalidParams(format!("unknown goal metric: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityGoal {
    pub id: Option<i64>,
    pub goal_type: GoalType,
    pub metric: GoalMetric,
    pub target_value: f64,
    pub enabled: bool,
    pub project_id: Option<String>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: Option<i64>,
    pub description: String,
    pub category_id: Option<String>,
    pub project_id: Option<String>,
    pub started_at: Timestamp,
    pub duration_secs: i64,
    pub source: String,
    pub created_at: Timestamp,
}

/// Duration of a single activity bucket window, in seconds (5 minutes).
/// Used by BucketAggregator for bucket boundaries.
pub const BUCKET_DURATION_SECS: i64 = 300;

/// Real-time tick emitted by ActivityTracker every poll interval.
/// Consumed by all event bus subscribers.
#[derive(Debug, Clone)]
pub struct ActivityTick {
    pub timestamp: Timestamp,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub site_name: Option<String>,
    pub url: Option<String>,
    pub category_id: Option<String>,
    pub category_type: Option<CategoryType>,
    pub is_idle: bool,
    pub idle_secs: f64,
    pub is_context_switch: bool,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    #[default]
    Manual,
    AutoDetected,
    Pomodoro,
}

impl std::fmt::Display for SessionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::AutoDetected => write!(f, "auto_detected"),
            Self::Pomodoro => write!(f, "pomodoro"),
        }
    }
}

impl std::str::FromStr for SessionSource {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(Self::Manual),
            "auto_detected" => Ok(Self::AutoDetected),
            "pomodoro" => Ok(Self::Pomodoro),
            _ => {
                Err(common::ToolError::InvalidParams(format!("unknown session source: {s}")).into())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sentiment {
    Positive,
    Neutral,
    Warning,
    Negative,
}

impl std::fmt::Display for Sentiment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Positive => write!(f, "positive"),
            Self::Neutral => write!(f, "neutral"),
            Self::Warning => write!(f, "warning"),
            Self::Negative => write!(f, "negative"),
        }
    }
}

impl std::str::FromStr for Sentiment {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "positive" => Ok(Self::Positive),
            "neutral" => Ok(Self::Neutral),
            "warning" => Ok(Self::Warning),
            "negative" => Ok(Self::Negative),
            _ => Err(common::ToolError::InvalidParams(format!("unknown sentiment: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightType {
    DeepWorkTrend,
    DistractionSpike,
    PeakHourShift,
    StreakAchieved,
    FatigueWarning,
    RecoveryImprovement,
    CategoryShift,
    NewPersonalBest,
    ConsistencyNote,
}

impl std::fmt::Display for InsightType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeepWorkTrend => write!(f, "deep_work_trend"),
            Self::DistractionSpike => write!(f, "distraction_spike"),
            Self::PeakHourShift => write!(f, "peak_hour_shift"),
            Self::StreakAchieved => write!(f, "streak_achieved"),
            Self::FatigueWarning => write!(f, "fatigue_warning"),
            Self::RecoveryImprovement => write!(f, "recovery_improvement"),
            Self::CategoryShift => write!(f, "category_shift"),
            Self::NewPersonalBest => write!(f, "new_personal_best"),
            Self::ConsistencyNote => write!(f, "consistency_note"),
        }
    }
}

impl std::str::FromStr for InsightType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "deep_work_trend" => Ok(Self::DeepWorkTrend),
            "distraction_spike" => Ok(Self::DistractionSpike),
            "peak_hour_shift" => Ok(Self::PeakHourShift),
            "streak_achieved" => Ok(Self::StreakAchieved),
            "fatigue_warning" => Ok(Self::FatigueWarning),
            "recovery_improvement" => Ok(Self::RecoveryImprovement),
            "category_shift" => Ok(Self::CategoryShift),
            "new_personal_best" => Ok(Self::NewPersonalBest),
            "consistency_note" => Ok(Self::ConsistencyNote),
            _ => Err(common::ToolError::InvalidParams(format!("unknown insight type: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightCard {
    pub id: String,
    pub insight_type: InsightType,
    pub title: String,
    pub body: String,
    pub sentiment: Sentiment,
    pub metric_value: Option<f64>,
    pub baseline_value: Option<f64>,
    pub date: String,
    pub dismissed: bool,
    pub generated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBucket {
    pub bucket_start: String,
    pub date: String,
    pub dominant_app: Option<String>,
    pub dominant_site: Option<String>,
    pub dominant_category: Option<String>,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub idle_secs: i64,
    pub context_switches: i64,
    pub focus_depth: Option<f64>,
    pub tick_count: i64,
    pub dominant_project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistractionPattern {
    pub id: Option<i64>,
    pub date: String,
    pub hour_of_day: i32,
    pub hours_active_today: f64,
    pub mins_since_break: f64,
    pub preceding_app: Option<String>,
    pub preceding_category: Option<String>,
    pub preceding_duration_mins: Option<f64>,
    pub distraction_app: String,
    pub distraction_category: Option<String>,
    pub recovery_secs: Option<i64>,
    pub created_at: Timestamp,
}

// -- Weekly Assessment --------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyAssessment {
    pub id: String,
    pub week_start: String,
    pub week_end: String,
    pub avg_score: Option<f64>,
    pub total_focus_mins: Option<i64>,
    pub total_productive_secs: Option<i64>,
    pub total_distracting_secs: Option<i64>,
    pub top_apps: Option<String>,
    pub summary: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod ai_entity_tests {
    use super::*;
    use ai_core::AiEntity;

    #[test]
    fn focus_session_entity_type() {
        assert_eq!(FocusSession::entity_type(), "focus_session");
    }

    #[test]
    fn focus_session_embed_text_uses_notes() {
        let mut session = FocusSession {
            id: "fs-1".into(),
            action_id: None,
            project_id: None,
            session_type: SessionType::Focus,
            target_mins: Some(25),
            started_at: jiff::Timestamp::now(),
            ended_at: None,
            actual_mins: None,
            interruptions: 0,
            distraction_events: vec![],
            quality_score: None,
            completed: false,
            notes: Some("deep work block".into()),
            source: SessionSource::Manual,
        };
        assert_eq!(session.embed_text(), "deep work block");

        session.notes = None;
        assert_eq!(session.embed_text(), "");
    }
}
