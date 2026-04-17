//! Planning, forecast, shared, and attachment types.

use chrono::{DateTime, NaiveTime, Utc};
use common::time::bridge::jiff_to_chrono;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use storage::rows::task::*;

use super::entity::{EnergyLevel, Task};

// ── Planning Types ──────────────────────────────────────────────────────────

/// Context for day planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningContext {
    /// Available tasks (pre-filtered).
    pub tasks: Vec<Task>,
    /// User's working hours.
    pub working_hours: WorkingHours,
    /// Calendar blocks for the day.
    #[serde(default)]
    pub calendar_blocks: Vec<CalendarBlock>,
    /// User's energy profile.
    pub energy_profile: Option<EnergyProfile>,
    /// Maximum tasks to schedule.
    pub max_tasks: Option<u32>,
    /// Currently focused task IDs (locked slots).
    #[serde(default)]
    pub locked_task_ids: Vec<String>,
    /// Target date for the plan.
    pub target_date: Option<String>,
}

/// A task with a computed priority score.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredTask {
    pub task: Task,
    pub score: f64,
    pub is_unblocked: bool,
    pub dependent_count: u32,
}

/// A generated day plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayPlan {
    pub slots: Vec<PlanSlot>,
    #[serde(default)]
    pub locked_slots: Vec<PlanSlot>,
    pub total_work_mins: i32,
    pub available_mins: i32,
    pub utilization: f64,
    pub reasoning: String,
    #[serde(default)]
    pub deferred: Vec<DeferredTask>,
    pub generated_at: DateTime<Utc>,
}

/// A slot in a day plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSlot {
    pub task_id: String,
    pub title: String,
    pub estimated_minutes: i32,
    pub energy_level: Option<EnergyLevel>,
    pub start_time: Option<String>,
    pub status: PlanSlotStatus,
}

/// Status of a plan slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlanSlotStatus {
    #[default]
    Pending,
    Active,
    Completed,
    Skipped,
}

/// A task deferred from the current plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeferredTask {
    pub task_id: String,
    pub title: String,
    pub reason: String,
}

/// Working hours configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingHours {
    /// Start of working day.
    pub start: NaiveTime,
    /// End of working day.
    pub end: NaiveTime,
    /// Start of lunch break.
    pub lunch_start: NaiveTime,
}

impl Default for WorkingHours {
    fn default() -> Self {
        Self {
            start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            lunch_start: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
        }
    }
}

// ── Forecast Types ──────────────────────────────────────────────────────────

/// Context for generating forecasts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastContext {
    /// Minimum number of historical samples required.
    pub min_sample_size: u32,
    /// How far back (in days) to look for historical data.
    pub lookback_days: u32,
    /// Whether to include subtask analysis.
    #[serde(default)]
    pub include_subtasks: bool,
}

impl Default for ForecastContext {
    fn default() -> Self {
        Self {
            min_sample_size: 5,
            lookback_days: 90,
            include_subtasks: true,
        }
    }
}

/// Forecast for a single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskForecast {
    pub task_id: String,
    pub estimated_minutes: i32,
    pub confidence_low: i32,
    pub confidence_high: i32,
    pub methodology: ForecastMethodology,
    #[serde(default)]
    pub risks: Vec<ForecastRisk>,
    pub data_quality: DataQuality,
}

/// Forecast for a project (aggregation of task forecasts).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectForecast {
    pub project_id: String,
    pub total_estimated_minutes: i32,
    pub confidence_low: i32,
    pub confidence_high: i32,
    pub remaining_tasks: u32,
    pub completed_tasks: u32,
    #[serde(default)]
    pub risks: Vec<ForecastRisk>,
    pub data_quality: DataQuality,
}

/// A risk associated with a forecast.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastRisk {
    pub kind: RiskKind,
    pub description: String,
    pub impact_minutes: Option<i32>,
}

/// Kind of forecast risk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RiskKind {
    HistoricalUnderestimation,
    DependencyChain,
    UnknownComplexity,
    ResourceContention,
    ExternalDependency,
}

/// Methodology used for forecasting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastMethodology {
    pub name: String,
    pub sample_size: u32,
    pub lookback_days: u32,
    pub adjustments: Vec<String>,
}

/// Quality of data underlying a forecast.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DataQuality {
    High,
    #[default]
    Medium,
    Low,
    Insufficient,
}

/// Report on estimation accuracy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccuracyReport {
    pub scope: AccuracyScope,
    pub sample_size: u32,
    pub mean_deviation_pct: f64,
    pub median_deviation_pct: f64,
    pub p90_deviation_pct: f64,
    pub trend: AccuracyTrend,
    pub by_energy_level: HashMap<String, f64>,
    pub by_complexity: HashMap<String, f64>,
}

/// Scope for accuracy reporting.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccuracyScope {
    pub project_id: Option<String>,
    pub area_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub lookback_days: Option<u32>,
}

/// Trend in estimation accuracy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccuracyTrend {
    Improving,
    #[default]
    Stable,
    Degrading,
    Insufficient,
}

// ── Shared Types ────────────────────────────────────────────────────────────

/// User's energy profile for scheduling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergyProfile {
    /// Hours of the day when energy is highest (e.g. [9, 10, 11]).
    #[serde(default)]
    pub peak_hours: Vec<u32>,
    /// Hours of the day when energy is lowest.
    #[serde(default)]
    pub low_energy_hours: Vec<u32>,
    /// Average focus session duration in minutes.
    pub avg_focus_duration_mins: Option<u32>,
    /// Preferred task size in minutes.
    pub preferred_task_size_mins: Option<u32>,
}

impl Default for EnergyProfile {
    fn default() -> Self {
        Self {
            peak_hours: vec![9, 10, 11],
            low_energy_hours: vec![14, 15],
            avg_focus_duration_mins: Some(45),
            preferred_task_size_mins: Some(30),
        }
    }
}

/// A calendar block representing busy/free time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarBlock {
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    #[serde(default = "default_true")]
    pub is_busy: bool,
}

fn default_true() -> bool {
    true
}

/// Per-scope overrides for task management settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScopeOverrides {
    pub wip_limit: Option<u32>,
    pub stale_task_days: Option<u32>,
}

// ── Estimation Record ───────────────────────────────────────────────────────

/// A historical estimation accuracy record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimationRecord {
    pub id: String,
    pub task_id: String,
    pub estimated_minutes: i32,
    pub actual_minutes: i32,
    pub deviation_pct: f64,
    pub complexity_score: Option<i32>,
    pub energy_level: Option<EnergyLevel>,
    pub tags: Vec<String>,
    pub completed_at: DateTime<Utc>,
}

impl From<TaskEstimationRow> for EstimationRecord {
    fn from(row: TaskEstimationRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            estimated_minutes: row.estimated_minutes,
            actual_minutes: row.actual_minutes,
            deviation_pct: row.deviation_pct,
            complexity_score: row.complexity_score,
            energy_level: row
                .energy_level
                .as_deref()
                .and_then(|s| s.parse::<EnergyLevel>().ok()),
            tags: row.tags,
            completed_at: jiff_to_chrono(*row.completed_at),
        }
    }
}

// ── Attachment & Time Entry ─────────────────────────────────────────────────

/// Attachment to a task (file, URL, or note).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub attachment_type: AttachmentType,
    pub title: Option<String>,
    pub value: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Type of attachment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    File,
    Url,
    Note,
}

impl AttachmentType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "file" => Some(Self::File),
            "url" => Some(Self::Url),
            "note" => Some(Self::Note),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Url => "url",
            Self::Note => "note",
        }
    }
}

impl From<TaskAttachmentRow> for Attachment {
    fn from(row: TaskAttachmentRow) -> Self {
        Self {
            id: row.id.to_string(),
            attachment_type: match row.attachment_type.as_str() {
                "file" => AttachmentType::File,
                "url" => AttachmentType::Url,
                _ => AttachmentType::Note,
            },
            title: row.title,
            value: row.value,
            tags: row.tags,
            created_at: jiff_to_chrono(*row.created_at),
        }
    }
}

/// Time tracking entry for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub note: Option<String>,
    pub energy_level: Option<EnergyLevel>,
    #[serde(default)]
    pub source: TimeEntrySource,
}

/// Source of a time entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimeEntrySource {
    #[default]
    Focus,
    Manual,
}

impl From<TaskTimeEntryRow> for TimeEntry {
    fn from(row: TaskTimeEntryRow) -> Self {
        Self {
            id: row.id.to_string(),
            started_at: jiff_to_chrono(*row.started_at),
            ended_at: row.ended_at.map(|ts| jiff_to_chrono(*ts)),
            duration_secs: row.duration_secs,
            note: row.note,
            energy_level: row
                .energy_level
                .as_deref()
                .and_then(|s| s.parse::<EnergyLevel>().ok()),
            source: match row.source.as_str() {
                "manual" => TimeEntrySource::Manual,
                _ => TimeEntrySource::Focus,
            },
        }
    }
}
