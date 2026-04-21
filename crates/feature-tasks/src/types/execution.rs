//! Execution runtime types and activity audit log.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use storage::rows::task::TaskActivityRow;

use super::entity::TaskType;

// ── Execution Types ─────────────────────────────────────────────────────────

/// The execution state of an agentic task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionState {
    #[default]
    Idle,
    AwaitingApproval,
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::AwaitingApproval => write!(f, "awaitingapproval"),
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl FromStr for ExecutionState {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "idle" => Ok(Self::Idle),
            "awaitingapproval" | "awaiting_approval" => Ok(Self::AwaitingApproval),
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown execution state: {}", s)),
        }
    }
}

/// Status of a task execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
    BudgetExceeded,
    Cancelled,
    TimedOut,
}

impl fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::BudgetExceeded => write!(f, "budgetexceeded"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::TimedOut => write!(f, "timedout"),
        }
    }
}

impl FromStr for ExecutionStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "budgetexceeded" | "budget_exceeded" => Ok(Self::BudgetExceeded),
            "cancelled" => Ok(Self::Cancelled),
            "timedout" | "timed_out" => Ok(Self::TimedOut),
            _ => Err(format!("unknown execution status: {}", s)),
        }
    }
}

/// Retry policy for failed task executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    /// Maximum number of retries.
    pub max_retries: u32,
    /// Base delay between retries in seconds.
    pub base_delay_secs: u64,
    /// Whether to use exponential backoff.
    pub exponential_backoff: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay_secs: 30,
            exponential_backoff: true,
        }
    }
}

impl RetryPolicy {
    /// Return a default retry policy tuned for the given task type.
    pub fn default_for_task_type(task_type: &TaskType) -> Self {
        match task_type {
            TaskType::Manual => Self {
                max_retries: 0,
                base_delay_secs: 0,
                exponential_backoff: false,
            },
            TaskType::Agentic => Self {
                max_retries: 3,
                base_delay_secs: 60,
                exponential_backoff: true,
            },
            TaskType::Hybrid => Self {
                max_retries: 1,
                base_delay_secs: 30,
                exponential_backoff: false,
            },
        }
    }
}

/// An artifact produced by an agentic execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionArtifact {
    pub name: String,
    pub artifact_type: String,
    pub content: String,
    pub created_at: Timestamp,
}

/// Snapshot of contextual information captured when a task is created or updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshot {
    /// Key facts relevant to this task.
    #[serde(default)]
    pub facts: Vec<String>,
    /// Chain of parent task IDs.
    #[serde(default)]
    pub parent_chain: Vec<String>,
    /// Titles of sibling tasks under the same parent.
    #[serde(default)]
    pub sibling_titles: Vec<String>,
    /// IDs of tasks blocking this one.
    #[serde(default)]
    pub active_blockers: Vec<String>,
    /// When this snapshot was captured.
    pub captured_at: Timestamp,
}

/// Agent-specific configuration stored on a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    /// Whether user approval is required before execution.
    #[serde(default)]
    pub require_approval: bool,
    /// Retry policy override.
    pub retry_policy: Option<RetryPolicy>,
}

// ── Activity Types ──────────────────────────────────────────────────────────

/// An audit-log entry for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskActivity {
    pub id: String,
    pub task_id: String,
    pub activity_type: ActivityType,
    pub field_changed: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub actor_type: ActorType,
    pub actor_id: Option<String>,
    pub summary: Option<String>,
    pub created_at: Timestamp,
}

impl From<TaskActivityRow> for TaskActivity {
    fn from(row: TaskActivityRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            activity_type: row
                .activity_type
                .parse::<ActivityType>()
                .unwrap_or(ActivityType::Updated),
            field_changed: row.field_changed,
            old_value: row.old_value,
            new_value: row.new_value,
            actor_type: row
                .actor_type
                .parse::<ActorType>()
                .unwrap_or(ActorType::System),
            actor_id: row.actor_id,
            summary: row.summary,
            created_at: *row.created_at,
        }
    }
}

/// Type of activity logged for a task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActivityType {
    Created,
    #[default]
    Updated,
    Completed,
    Deleted,
    Decomposed,
    FocusStarted,
    FocusEnded,
    DependencyAdded,
    DependencyRemoved,
    StatusChanged,
    Migrated,
}

impl fmt::Display for ActivityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Updated => write!(f, "updated"),
            Self::Completed => write!(f, "completed"),
            Self::Deleted => write!(f, "deleted"),
            Self::Decomposed => write!(f, "decomposed"),
            Self::FocusStarted => write!(f, "focusstarted"),
            Self::FocusEnded => write!(f, "focusended"),
            Self::DependencyAdded => write!(f, "dependencyadded"),
            Self::DependencyRemoved => write!(f, "dependencyremoved"),
            Self::StatusChanged => write!(f, "statuschanged"),
            Self::Migrated => write!(f, "migrated"),
        }
    }
}

impl FromStr for ActivityType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "created" => Ok(Self::Created),
            "updated" => Ok(Self::Updated),
            "completed" => Ok(Self::Completed),
            "deleted" => Ok(Self::Deleted),
            "decomposed" => Ok(Self::Decomposed),
            "focusstarted" | "focus_started" => Ok(Self::FocusStarted),
            "focusended" | "focus_ended" => Ok(Self::FocusEnded),
            "dependencyadded" | "dependency_added" => Ok(Self::DependencyAdded),
            "dependencyremoved" | "dependency_removed" => Ok(Self::DependencyRemoved),
            "statuschanged" | "status_changed" => Ok(Self::StatusChanged),
            "migrated" => Ok(Self::Migrated),
            _ => Err(format!("unknown activity type: {}", s)),
        }
    }
}

/// Who performed the activity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActorType {
    User,
    Agent,
    #[default]
    System,
}

impl fmt::Display for ActorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Agent => write!(f, "agent"),
            Self::System => write!(f, "system"),
        }
    }
}

impl FromStr for ActorType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "agent" => Ok(Self::Agent),
            "system" => Ok(Self::System),
            _ => Err(format!("unknown actor type: {}", s)),
        }
    }
}
