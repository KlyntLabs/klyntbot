use chrono::{DateTime, NaiveDate, Utc};
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
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUsage {
    pub project_id: String,
    pub display_name: String,
    pub duration_secs: i64,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: Option<i64>,
    pub app_name: String,
    pub window_title: Option<String>,
    pub site_name: Option<String>,
    pub bundle_id: Option<String>,
    pub url: Option<String>,
    pub category_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
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
    pub source: SessionSource,
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
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: Option<i64>,
    pub description: String,
    pub category_id: Option<String>,
    pub project_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_secs: i64,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

/// Duration of a single activity bucket window, in seconds (5 minutes).
/// Shared between BucketAggregator (bucket boundaries) and AutoFocusDetector (window evaluation).
pub const BUCKET_DURATION_SECS: i64 = 300;

/// Real-time tick emitted by ActivityTracker every poll interval.
/// Consumed by all event bus subscribers.
#[derive(Debug, Clone)]
pub struct ActivityTick {
    pub timestamp: DateTime<Utc>,
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
    pub generated_at: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
}

// ── Intelligence Layer Types ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntelligenceSessionType {
    Focus,
    Meeting,
    Break,
}

impl std::fmt::Display for IntelligenceSessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Focus => write!(f, "focus"),
            Self::Meeting => write!(f, "meeting"),
            Self::Break => write!(f, "break"),
        }
    }
}

impl std::str::FromStr for IntelligenceSessionType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "focus" => Ok(Self::Focus),
            "meeting" => Ok(Self::Meeting),
            "break" => Ok(Self::Break),
            _ => Err(common::ToolError::InvalidParams(format!(
                "unknown intelligence session type: {s}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    App,
    Url,
    Title,
    Compound,
}

impl std::fmt::Display for RuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::App => write!(f, "app"),
            Self::Url => write!(f, "url"),
            Self::Title => write!(f, "title"),
            Self::Compound => write!(f, "compound"),
        }
    }
}

impl std::str::FromStr for RuleType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "app" => Ok(Self::App),
            "url" => Ok(Self::Url),
            "title" => Ok(Self::Title),
            "compound" => Ok(Self::Compound),
            _ => Err(common::ToolError::InvalidParams(format!("unknown rule type: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    Exact,
    Prefix,
    Contains,
    Regex,
}

impl std::fmt::Display for MatchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact => write!(f, "exact"),
            Self::Prefix => write!(f, "prefix"),
            Self::Contains => write!(f, "contains"),
            Self::Regex => write!(f, "regex"),
        }
    }
}

impl std::str::FromStr for MatchMode {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exact" => Ok(Self::Exact),
            "prefix" => Ok(Self::Prefix),
            "contains" => Ok(Self::Contains),
            "regex" => Ok(Self::Regex),
            _ => Err(common::ToolError::InvalidParams(format!("unknown match mode: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSource {
    System,
    User,
    Learned,
}

impl std::fmt::Display for RuleSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Learned => write!(f, "learned"),
        }
    }
}

impl std::str::FromStr for RuleSource {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            "learned" => Ok(Self::Learned),
            _ => Err(common::ToolError::InvalidParams(format!("unknown rule source: {s}")).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationSource {
    Rule,
    AiFallback,
    Default,
}

impl std::fmt::Display for ClassificationSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rule => write!(f, "rule"),
            Self::AiFallback => write!(f, "ai"),
            Self::Default => write!(f, "default"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingRule {
    pub id: String,
    pub rule_type: RuleType,
    pub match_field: String,
    pub match_pattern: String,
    pub match_mode: MatchMode,
    pub category: String,
    pub session_type: IntelligenceSessionType,
    pub priority: i32,
    pub source: RuleSource,
    pub confidence: f64,
    pub hit_count: i64,
    pub last_hit_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ProductivitySession {
    pub id: String,
    pub session_type: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_secs: Option<i64>,
    pub dominant_category: Option<String>,
    pub category_purity: Option<f64>,
    pub quality_score: Option<f64>,
    pub source: String,
    pub app_breakdown: Option<String>,
    pub context_switches: i64,
    pub distraction_count: i64,
    pub predicted_energy: Option<f64>,
    pub okr_alignment: Option<f64>,
    pub notes: Option<String>,
    pub tags: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct QualityScore {
    pub id: String,
    pub score_date: String,
    pub session_id: Option<String>,
    pub overall_score: f64,
    pub focus_depth: f64,
    pub okr_alignment: f64,
    pub distraction_inv: f64,
    pub task_completion: f64,
    pub continuity: f64,
    pub weights_json: Option<String>,
    pub explanation: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastType {
    Energy,
    FocusWindow,
    MeetingLoad,
    BurnoutRisk,
}

impl std::fmt::Display for ForecastType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Energy => write!(f, "energy"),
            Self::FocusWindow => write!(f, "focus_window"),
            Self::MeetingLoad => write!(f, "meeting_load"),
            Self::BurnoutRisk => write!(f, "burnout_risk"),
        }
    }
}

impl std::str::FromStr for ForecastType {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "energy" => Ok(Self::Energy),
            "focus_window" => Ok(Self::FocusWindow),
            "meeting_load" => Ok(Self::MeetingLoad),
            "burnout_risk" => Ok(Self::BurnoutRisk),
            _ => {
                Err(common::ToolError::InvalidParams(format!("unknown forecast type: {s}")).into())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    pub id: String,
    pub forecast_date: String,
    pub forecast_type: String,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub predicted_value: f64,
    pub confidence: f64,
    pub stability: f64,
    pub auto_protected: bool,
    pub user_overrode: bool,
    pub actual_value: Option<f64>,
    pub prediction_error: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyMoment {
    pub time: String,
    pub description: String,
    pub moment_type: MomentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MomentType {
    DeepWork,
    Breakthrough,
    Distraction,
    Recovery,
    MeetingHeavy,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Narrative {
    pub id: String,
    pub narrative_date: String,
    pub narrative_text: String,
    pub key_moments: Option<String>,
    pub sentiment: Option<String>,
    pub total_focus_mins: i64,
    pub total_meeting_mins: i64,
    pub total_break_mins: i64,
    pub quality_score: Option<f64>,
    pub top_categories: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct VoiceJournalEntry {
    pub id: String,
    pub recorded_at: String,
    pub duration_secs: i64,
    pub transcript: Option<String>,
    pub extracted_facts: Option<String>,
    pub sentiment: Option<String>,
    pub session_id: Option<String>,
    pub processed: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CategorizationCacheEntry {
    pub cache_key: String,
    pub category: String,
    pub session_type: String,
    pub confidence: f64,
    pub source: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyRule {
    pub id: String,
    pub rule_type: String,
    pub pattern: String,
    pub match_mode: MatchMode,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RuleEvolutionEntry {
    pub id: String,
    pub rule_id: String,
    pub action: String,
    pub old_confidence: Option<f64>,
    pub new_confidence: Option<f64>,
    pub old_category: Option<String>,
    pub new_category: Option<String>,
    pub trigger_source: Option<String>,
    pub evidence_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationResult {
    pub category: String,
    pub session_type: IntelligenceSessionType,
    pub confidence: f64,
    pub rule_id: Option<String>,
    pub source: ClassificationSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyFilterResult {
    pub excluded: bool,
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEvent {
    Created {
        session_id: String,
        session_type: IntelligenceSessionType,
        dominant_category: String,
    },
    Ended {
        session_id: String,
        session_type: IntelligenceSessionType,
        duration_secs: i64,
        quality_score: Option<f64>,
    },
    Updated {
        session_id: String,
        session_type: IntelligenceSessionType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterventionType {
    FocusModeActivation,
    BreakReminder,
    EnergyWindowAlert,
    ContextSwitchWarning,
    QualityDrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedAction {
    TakeBreak,
    StartFocus,
    ReviewTasks,
    ReduceDistractions,
    ProtectBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityIntervention {
    pub intervention_type: InterventionType,
    pub message: String,
    pub suggested_action: SuggestedAction,
    pub urgency: Urgency,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreWeights {
    pub focus_depth: f64,
    pub okr_alignment: f64,
    pub distraction_inv: f64,
    pub task_completion: f64,
    pub continuity: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            focus_depth: 0.30,
            okr_alignment: 0.25,
            distraction_inv: 0.20,
            task_completion: 0.15,
            continuity: 0.10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedBlock {
    pub window_start: String,
    pub window_end: String,
    pub predicted_energy: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccuracyReport {
    pub forecast_date: NaiveDate,
    pub total_forecasts: usize,
    pub evaluated: usize,
    pub mean_error: f64,
    pub mean_stability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeMetrics {
    pub total_focus_mins: i64,
    pub total_meeting_mins: i64,
    pub total_break_mins: i64,
    pub quality_score: Option<f64>,
    pub top_categories: Vec<String>,
}
