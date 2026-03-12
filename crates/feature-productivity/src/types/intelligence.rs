use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

// -- Intelligence Layer Types -------------------------------------------------

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
    pub action_id: Option<String>,
    pub project_id: Option<String>,
    pub target_mins: Option<i64>,
    pub actual_mins: Option<i64>,
    pub interruptions: Option<i64>,
    pub distraction_events: Option<String>,
    pub completed: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

impl ProductivitySession {
    /// Extract the most-used app from the JSON `app_breakdown` field.
    pub fn top_app(&self) -> String {
        self.app_breakdown
            .as_deref()
            .and_then(|ab| serde_json::from_str::<serde_json::Value>(ab).ok())
            .and_then(|v| {
                v.as_object().and_then(|m| {
                    m.iter()
                        .max_by_key(|(_, v)| v.as_i64().unwrap_or(0))
                        .map(|(k, _)| k.clone())
                })
            })
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Generate a template-based title from category + top app.
    pub fn fallback_title(&self) -> String {
        let app = self.top_app();
        let category = self.dominant_category.as_deref().unwrap_or("Activity");
        match category {
            "coding" | "Coding" => format!("Coding in {app}"),
            "design" | "Design" => format!("Design in {app}"),
            "communication" | "Communication" => format!("{app} comms"),
            "research" | "Research" => format!("Research in {app}"),
            "meeting" | "Meeting" => format!("Meeting in {app}"),
            "writing" | "Writing" => format!("Writing in {app}"),
            cat => format!("{cat} · {app}"),
        }
    }

    /// Generate a template-based description from duration, quality, and context switches.
    pub fn fallback_description(&self, quality: Option<f64>) -> Option<String> {
        let dur_mins = self.duration_secs.unwrap_or(0) / 60;
        if dur_mins == 0 {
            return None;
        }
        let quality_desc = match quality.or(self.quality_score) {
            Some(q) if q >= 80.0 => "excellent focus",
            Some(q) if q >= 60.0 => "good focus",
            Some(q) if q >= 40.0 => "moderate focus",
            Some(_) => "scattered",
            None => "unscored",
        };
        Some(format!(
            "{}min · {} · {} switches",
            dur_mins, quality_desc, self.context_switches
        ))
    }
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
    pub deep_work_ratio: f64,
    pub avg_session_length: f64,
    pub meeting_focus_ratio: f64,
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub calendar_id: String,
    pub title: String,
    pub description: Option<String>,
    pub started_at: String,
    pub ended_at: String,
    pub location: Option<String>,
    pub attendees_count: i64,
    pub is_recurring: bool,
    pub recurrence_id: Option<String>,
    pub source: String,
    pub external_uid: String,
    pub session_id: Option<String>,
    pub color: Option<String>,
    pub synced_at: String,
    pub created_at: String,
    pub updated_at: String,
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
    pub deep_work_ratio: f64,
    pub avg_session_length: f64,
    pub meeting_focus_ratio: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            focus_depth: 0.20,
            okr_alignment: 0.10,
            distraction_inv: 0.15,
            task_completion: 0.10,
            continuity: 0.10,
            deep_work_ratio: 0.15,
            avg_session_length: 0.10,
            meeting_focus_ratio: 0.10,
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
