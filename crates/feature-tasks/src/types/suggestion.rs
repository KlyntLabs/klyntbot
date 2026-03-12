//! Suggestion and decomposition types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use storage::rows::task::*;

use super::entity::{EnergyLevel, TaskType};
use super::planning::{CalendarBlock, EnergyProfile};

// ── Suggestion Types ────────────────────────────────────────────────────────

/// An AI-generated suggestion for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSuggestion {
    pub id: String,
    pub task_id: Option<String>,
    pub suggestion_type: SuggestionType,
    pub title: String,
    pub description: Option<String>,
    pub confidence: f64,
    pub action_payload: Option<SuggestionAction>,
    pub status: SuggestionStatus,
    pub trigger: Option<SuggestionTrigger>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl From<TaskSuggestionRow> for TaskSuggestion {
    fn from(row: TaskSuggestionRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            suggestion_type: row
                .suggestion_type
                .parse::<SuggestionType>()
                .unwrap_or(SuggestionType::WorkflowInsight),
            title: row.title,
            description: row.description,
            confidence: row.confidence,
            action_payload: row
                .action_payload
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            status: row.status.parse::<SuggestionStatus>().unwrap_or_default(),
            trigger: row
                .trigger
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            created_at: row.created_at,
            resolved_at: row.resolved_at,
        }
    }
}

/// Type of suggestion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionType {
    Reprioritize,
    Reschedule,
    Decompose,
    Delegate,
    Abandon,
    Merge,
    Unblock,
    AdjustEstimation,
    AdjustEnergy,
    WorkflowInsight,
    Execute,
}

impl fmt::Display for SuggestionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reprioritize => write!(f, "reprioritize"),
            Self::Reschedule => write!(f, "reschedule"),
            Self::Decompose => write!(f, "decompose"),
            Self::Delegate => write!(f, "delegate"),
            Self::Abandon => write!(f, "abandon"),
            Self::Merge => write!(f, "merge"),
            Self::Unblock => write!(f, "unblock"),
            Self::AdjustEstimation => write!(f, "adjustestimation"),
            Self::AdjustEnergy => write!(f, "adjustenergy"),
            Self::WorkflowInsight => write!(f, "workflowinsight"),
            Self::Execute => write!(f, "execute"),
        }
    }
}

impl FromStr for SuggestionType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "reprioritize" => Ok(Self::Reprioritize),
            "reschedule" => Ok(Self::Reschedule),
            "decompose" => Ok(Self::Decompose),
            "delegate" => Ok(Self::Delegate),
            "abandon" => Ok(Self::Abandon),
            "merge" => Ok(Self::Merge),
            "unblock" => Ok(Self::Unblock),
            "adjustestimation" | "adjust_estimation" => Ok(Self::AdjustEstimation),
            "adjustenergy" | "adjust_energy" => Ok(Self::AdjustEnergy),
            "workflowinsight" | "workflow_insight" => Ok(Self::WorkflowInsight),
            "execute" => Ok(Self::Execute),
            _ => Err(format!("unknown suggestion type: {}", s)),
        }
    }
}

/// Status of a suggestion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionStatus {
    #[default]
    Pending,
    Accepted,
    Applied,
    Dismissed,
    Expired,
    AutoApplied,
}

impl fmt::Display for SuggestionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Accepted => write!(f, "accepted"),
            Self::Applied => write!(f, "applied"),
            Self::Dismissed => write!(f, "dismissed"),
            Self::Expired => write!(f, "expired"),
            Self::AutoApplied => write!(f, "autoapplied"),
        }
    }
}

impl FromStr for SuggestionStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "applied" => Ok(Self::Applied),
            "dismissed" => Ok(Self::Dismissed),
            "expired" => Ok(Self::Expired),
            "autoapplied" | "auto_applied" => Ok(Self::AutoApplied),
            _ => Err(format!("unknown suggestion status: {}", s)),
        }
    }
}

/// A candidate suggestion that can be created.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionCandidate {
    pub task_id: Option<String>,
    pub suggestion_type: SuggestionType,
    pub title: String,
    pub description: Option<String>,
    pub confidence: f64,
    pub action: Option<SuggestionAction>,
    pub trigger: Option<SuggestionTrigger>,
}

/// Action to apply for a suggestion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum SuggestionAction {
    SetPriority { priority: i16 },
    SetDueDate { due_date: String },
    TriggerDecomposition,
    ConvertToAgentic,
    Archive,
    MergeInto { target_task_id: String },
    RemoveBlocker { blocker_id: String },
    UpdateEstimationBaseline { minutes: i32 },
    SetEnergyLevel { level: EnergyLevel },
    Informational,
}

/// What triggered a suggestion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionTrigger {
    TaskOverdue,
    TaskStale,
    ExecutionFailed,
    EstimationDeviation,
    WipLimitExceeded,
    BlockedChainStale,
    FocusAbandonedEarly,
    PeriodicScan,
    UserRequested,
}

/// Scope filter for generating suggestions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionScope {
    pub project_id: Option<String>,
    pub area_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

// ── Decomposition Types ─────────────────────────────────────────────────────

/// Context provided to the decomposition handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompositionContext {
    /// Maximum depth of subtask nesting.
    pub max_depth: u32,
    /// Maximum subtasks per level.
    pub max_subtasks_per_level: u32,
    /// Titles of existing subtasks (to avoid duplicates).
    #[serde(default)]
    pub existing_subtasks: Vec<String>,
    /// Project context string.
    pub project_context: Option<String>,
    /// Cognitive facts relevant to the task (placeholder for SemanticFact).
    #[serde(default)]
    pub cognitive_facts: Vec<String>,
    /// User's energy profile for scheduling subtasks.
    pub user_energy_profile: Option<EnergyProfile>,
    /// Calendar blocks for scheduling context.
    #[serde(default)]
    pub calendar_context: Vec<CalendarBlock>,
}

impl Default for DecompositionContext {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_subtasks_per_level: 7,
            existing_subtasks: Vec::new(),
            project_context: None,
            cognitive_facts: Vec::new(),
            user_energy_profile: None,
            calendar_context: Vec::new(),
        }
    }
}

/// Result of a task decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompositionResult {
    pub tree: DecompositionTree,
    pub confidence: f64,
    pub reasoning: String,
    #[serde(default)]
    pub validation_warnings: Vec<ValidationWarning>,
}

/// The decomposition plan tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompositionTree {
    pub subtasks: Vec<PlannedSubtask>,
    pub total_estimated_mins: Option<i32>,
}

/// A planned subtask in a decomposition tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSubtask {
    /// Temporary ID for internal references within the tree.
    pub temp_id: String,
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub energy_level: Option<EnergyLevel>,
    pub priority: Option<i16>,
    pub task_type: Option<TaskType>,
    /// Temporary IDs of subtasks this one depends on.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Nested children.
    #[serde(default)]
    pub children: Vec<PlannedSubtask>,
}

/// A warning generated during decomposition validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationWarning {
    pub kind: ValidationWarningKind,
    pub message: String,
    pub subtask_temp_id: Option<String>,
}

/// Kind of validation warning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationWarningKind {
    TooManySubtasks,
    TooDeep,
    MissingEstimation,
    CircularDependency,
    DuplicateTitle,
    EstimationMismatch,
}

/// Status of a decomposition plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DecompositionStatus {
    #[default]
    Pending,
    Applied,
    Rejected,
}

impl fmt::Display for DecompositionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Applied => write!(f, "applied"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

impl FromStr for DecompositionStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "applied" => Ok(Self::Applied),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("unknown decomposition status: {}", s)),
        }
    }
}
