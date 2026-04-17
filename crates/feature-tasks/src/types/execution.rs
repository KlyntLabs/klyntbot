//! Execution runtime types and activity audit log.

use chrono::{DateTime, Utc};
use common::time::bridge::jiff_to_chrono;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use storage::rows::task::*;

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

/// A single execution record for an agentic task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecution {
    pub id: String,
    pub task_id: String,
    pub status: ExecutionStatus,
    pub agent_profile: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub tokens_used: Option<i64>,
    pub cost_usd: Option<f64>,
    pub input_context: Option<String>,
    pub output_summary: Option<String>,
    pub error_message: Option<String>,
    pub artifacts: Vec<ExecutionArtifact>,
    pub metrics: Option<serde_json::Value>,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
}

impl From<TaskExecutionRow> for TaskExecution {
    fn from(row: TaskExecutionRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            status: row.status.parse::<ExecutionStatus>().unwrap_or_default(),
            agent_profile: row.agent_profile,
            started_at: row.started_at.map(|ts| jiff_to_chrono(*ts)),
            completed_at: row.completed_at.map(|ts| jiff_to_chrono(*ts)),
            duration_secs: row.duration_secs,
            tokens_used: row.tokens_used,
            cost_usd: row.cost_usd,
            input_context: row.input_context,
            output_summary: row.output_summary,
            error_message: row.error_message,
            artifacts: row
                .artifacts
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default(),
            metrics: row
                .metrics
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            retry_count: row.retry_count,
            created_at: jiff_to_chrono(*row.created_at),
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

/// Result of initiating task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum ExecuteResult {
    Started { execution_id: String },
    AwaitingApproval { suggestion_id: String },
}

/// Configuration for an agentic task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionConfig {
    /// Which agent profile to use (e.g. "task", "general").
    pub agent_profile: Option<String>,
    /// Maximum cost in USD for this execution.
    pub max_cost_usd: Option<f64>,
    /// Maximum number of ReAct iterations.
    pub max_iterations: Option<u32>,
    /// Timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// Allowlisted tools (empty = all allowed).
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Allowlisted MCP servers (empty = none, ["*"] = all).
    #[serde(default)]
    pub allowed_mcp: Vec<String>,
    /// Retry policy for failed executions.
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    /// Whether to require user approval before execution starts.
    #[serde(default)]
    pub require_approval: bool,
    /// How often (in seconds) to emit progress updates.
    pub progress_interval_secs: Option<u64>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            agent_profile: Some("task".to_string()),
            max_cost_usd: Some(0.50),
            max_iterations: Some(10),
            timeout_secs: Some(300),
            allowed_tools: Vec::new(),
            allowed_mcp: Vec::new(),
            retry_policy: RetryPolicy::default(),
            require_approval: false,
            progress_interval_secs: Some(30),
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
    pub created_at: DateTime<Utc>,
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
    pub captured_at: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
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
            created_at: jiff_to_chrono(*row.created_at),
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
