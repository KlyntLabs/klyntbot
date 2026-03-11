//! Domain types for the feature-tasks crate.
//!
//! Canonical definitions of `Task`, `TaskExecution`, `TaskActivity`, `TaskSuggestion`,
//! and all associated enums and supporting types. Includes conversions from/to
//! storage row types.

use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use storage::rows::task::*;

// ── Core Task ───────────────────────────────────────────────────────────────

/// A task with focus tracking, agentic execution, and planning support.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub area_id: String,
    pub project_id: Option<String>,
    pub key_result_id: Option<String>,
    pub objective_id: Option<String>,
    pub parent_id: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub status: String,
    pub focused_at: Option<DateTime<Utc>>,
    pub focus_deadline: Option<DateTime<Utc>>,
    pub focus_expired_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_tracked_secs: i64,
    pub estimated_minutes: Option<i32>,
    pub calendar_event_uid: Option<String>,
    pub last_reminded_at: Option<DateTime<Utc>>,
    pub recurrence_rule: Option<String>,
    pub recurrence_parent_id: Option<String>,
    pub is_template: bool,
    pub next_instance_date: Option<DateTime<Utc>>,
    pub status_label_id: Option<String>,
    pub position: i32,
    pub group_id: Option<String>,
    // Agentic fields
    pub task_type: TaskType,
    pub acceptance_criteria: Option<String>,
    pub agent_config: Option<AgentConfig>,
    pub execution_state: ExecutionState,
    pub spawned_execution_id: Option<String>,
    pub context_snapshot: Option<ContextSnapshot>,
    pub energy_level: Option<EnergyLevel>,
    pub estimated_focus_blocks: Option<i32>,
    pub actual_minutes: Option<i32>,
    pub complexity_score: Option<i32>,
    pub completed: bool,
    // Derived fields (populated by handlers, not stored)
    #[serde(default)]
    pub subtask_count: i64,
    #[serde(default)]
    pub subtask_completed_count: i64,
}

impl Task {
    /// Generate an 8-char short ID from a UUID.
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }

    /// Create a default blank instance (useful as a starting point).
    pub fn default_instance() -> Self {
        let now = Utc::now();
        Self {
            id: Self::generate_id(),
            title: String::new(),
            description: None,
            area_id: String::new(),
            project_id: None,
            key_result_id: None,
            objective_id: None,
            parent_id: None,
            priority: None,
            due_date: None,
            tags: Vec::new(),
            status: "todo".to_string(),
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            status_label_id: None,
            position: 0,
            group_id: None,
            task_type: TaskType::default(),
            acceptance_criteria: None,
            agent_config: None,
            execution_state: ExecutionState::default(),
            spawned_execution_id: None,
            context_snapshot: None,
            energy_level: None,
            estimated_focus_blocks: None,
            actual_minutes: None,
            complexity_score: None,
            completed: false,
            subtask_count: 0,
            subtask_completed_count: 0,
        }
    }
}

/// Searchable implementation for integration with tools-core RRF search.
impl tools_core::Searchable for Task {
    fn search_id(&self) -> &str {
        &self.id
    }
}

// ── Row <-> Domain Conversions ──────────────────────────────────────────────

impl From<TaskRow> for Task {
    fn from(row: TaskRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            area_id: row.area_id,
            project_id: row.project_id,
            key_result_id: row.key_result_id,
            objective_id: row.objective_id,
            parent_id: row.parent_id,
            priority: row.priority,
            due_date: row.due_date,
            tags: row.tags,
            status: row.status,
            focused_at: row.focused_at,
            focus_deadline: row.focus_deadline,
            focus_expired_count: row.focus_expired_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
            completed_at: row.completed_at,
            total_tracked_secs: row.total_tracked_secs,
            estimated_minutes: row.estimated_minutes,
            calendar_event_uid: row.calendar_event_uid,
            last_reminded_at: row.last_reminded_at,
            recurrence_rule: row.recurrence_rule,
            recurrence_parent_id: row.recurrence_parent_id,
            is_template: row.is_template,
            next_instance_date: row.next_instance_date,
            status_label_id: row.status_label_id,
            position: row.position,
            group_id: row.group_id,
            task_type: row.task_type.parse::<TaskType>().unwrap_or_default(),
            acceptance_criteria: row.acceptance_criteria,
            agent_config: row
                .agent_config
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            execution_state: row
                .execution_state
                .parse::<ExecutionState>()
                .unwrap_or_default(),
            spawned_execution_id: row.spawned_execution_id,
            context_snapshot: row
                .context_snapshot
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            energy_level: row
                .energy_level
                .as_deref()
                .and_then(|s| s.parse::<EnergyLevel>().ok()),
            estimated_focus_blocks: row.estimated_focus_blocks,
            actual_minutes: row.actual_minutes,
            complexity_score: row.complexity_score,
            completed: row.completed,
            subtask_count: 0,
            subtask_completed_count: 0,
        }
    }
}

impl From<&Task> for TaskRow {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            title: task.title.clone(),
            description: task.description.clone(),
            area_id: task.area_id.clone(),
            project_id: task.project_id.clone(),
            key_result_id: task.key_result_id.clone(),
            objective_id: task.objective_id.clone(),
            parent_id: task.parent_id.clone(),
            priority: task.priority,
            due_date: task.due_date,
            tags: task.tags.clone(),
            status: task.status.clone(),
            focused_at: task.focused_at,
            focus_deadline: task.focus_deadline,
            focus_expired_count: task.focus_expired_count,
            created_at: task.created_at,
            updated_at: task.updated_at,
            completed_at: task.completed_at,
            total_tracked_secs: task.total_tracked_secs,
            estimated_minutes: task.estimated_minutes,
            calendar_event_uid: task.calendar_event_uid.clone(),
            last_reminded_at: task.last_reminded_at,
            recurrence_rule: task.recurrence_rule.clone(),
            recurrence_parent_id: task.recurrence_parent_id.clone(),
            is_template: task.is_template,
            next_instance_date: task.next_instance_date,
            status_label_id: task.status_label_id.clone(),
            position: task.position,
            group_id: task.group_id.clone(),
            task_type: task.task_type.to_string(),
            acceptance_criteria: task.acceptance_criteria.clone(),
            agent_config: task
                .agent_config
                .as_ref()
                .and_then(|c| serde_json::to_string(c).ok()),
            execution_state: task.execution_state.to_string(),
            spawned_execution_id: task.spawned_execution_id.clone(),
            context_snapshot: task
                .context_snapshot
                .as_ref()
                .and_then(|c| serde_json::to_string(c).ok()),
            energy_level: task.energy_level.as_ref().map(|e| e.to_string()),
            estimated_focus_blocks: task.estimated_focus_blocks,
            actual_minutes: task.actual_minutes,
            complexity_score: task.complexity_score,
            completed: task.completed,
        }
    }
}

// ── Core Enums ──────────────────────────────────────────────────────────────

/// The type of task: manual, agentic (fully automated), or hybrid.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    #[default]
    Manual,
    Agentic,
    Hybrid,
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::Agentic => write!(f, "agentic"),
            Self::Hybrid => write!(f, "hybrid"),
        }
    }
}

impl FromStr for TaskType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "manual" => Ok(Self::Manual),
            "agentic" => Ok(Self::Agentic),
            "hybrid" => Ok(Self::Hybrid),
            _ => Err(format!("unknown task type: {}", s)),
        }
    }
}

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

/// Energy level required for a task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EnergyLevel {
    Low,
    #[default]
    Medium,
    High,
    Deep,
}

impl fmt::Display for EnergyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Deep => write!(f, "deep"),
        }
    }
}

impl FromStr for EnergyLevel {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "deep" => Ok(Self::Deep),
            _ => Err(format!("unknown energy level: {}", s)),
        }
    }
}

/// Task status lifecycle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Todo,
    Doing,
    Done,
    Someday,
}

impl TaskStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "todo" => Some(Self::Todo),
            "doing" => Some(Self::Doing),
            "done" => Some(Self::Done),
            "someday" => Some(Self::Someday),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Done => "done",
            Self::Someday => "someday",
        }
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ── Execution Types ─────────────────────────────────────────────────────────

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
            started_at: row.started_at,
            completed_at: row.completed_at,
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
            created_at: row.created_at,
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
            created_at: row.created_at,
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
            completed_at: row.completed_at,
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
            created_at: row.created_at,
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
            started_at: row.started_at,
            ended_at: row.ended_at,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_type_default_is_manual() {
        assert_eq!(TaskType::default(), TaskType::Manual);
    }

    #[test]
    fn test_execution_state_default_is_idle() {
        assert_eq!(ExecutionState::default(), ExecutionState::Idle);
    }

    #[test]
    fn test_energy_level_default_is_medium() {
        assert_eq!(EnergyLevel::default(), EnergyLevel::Medium);
    }

    #[test]
    fn test_task_serde_round_trip() {
        let mut t = Task::default_instance();
        t.area_id = "area-1".to_string();
        t.title = "Test task".to_string();
        t.task_type = TaskType::Agentic;
        t.energy_level = Some(EnergyLevel::High);
        let json = serde_json::to_string(&t).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(t.id, parsed.id);
        assert_eq!(parsed.task_type, TaskType::Agentic);
        assert_eq!(parsed.energy_level, Some(EnergyLevel::High));
        assert!(!parsed.is_template);
        assert!(!parsed.completed);
    }

    #[test]
    fn test_task_from_row_conversion() {
        let now = Utc::now();
        let row = TaskRow {
            id: "test1234".to_string(),
            title: "Test".to_string(),
            description: None,
            area_id: "area-1".to_string(),
            project_id: None,
            key_result_id: None,
            objective_id: None,
            parent_id: None,
            priority: Some(2),
            due_date: None,
            tags: vec!["tag1".to_string()],
            status: "doing".to_string(),
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            total_tracked_secs: 0,
            estimated_minutes: Some(30),
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            status_label_id: None,
            position: 0,
            group_id: None,
            task_type: "agentic".to_string(),
            acceptance_criteria: Some("It works".to_string()),
            agent_config: Some(r#"{"requireApproval":true,"retryPolicy":null}"#.to_string()),
            execution_state: "idle".to_string(),
            spawned_execution_id: None,
            context_snapshot: None,
            energy_level: Some("high".to_string()),
            estimated_focus_blocks: Some(2),
            actual_minutes: None,
            complexity_score: Some(3),
            completed: false,
        };

        let task = Task::from(row);
        assert_eq!(task.id, "test1234");
        assert_eq!(task.task_type, TaskType::Agentic);
        assert_eq!(task.energy_level, Some(EnergyLevel::High));
        assert_eq!(task.priority, Some(2));
        assert!(task.agent_config.is_some());
        assert!(task.agent_config.unwrap().require_approval);
        assert_eq!(task.estimated_focus_blocks, Some(2));
        assert_eq!(task.complexity_score, Some(3));
    }

    #[test]
    fn test_task_to_row_round_trip() {
        let mut t = Task::default_instance();
        t.area_id = "area-1".to_string();
        t.task_type = TaskType::Hybrid;
        t.energy_level = Some(EnergyLevel::Deep);
        t.agent_config = Some(AgentConfig {
            require_approval: true,
            retry_policy: Some(RetryPolicy::default()),
        });

        let row = TaskRow::from(&t);
        assert_eq!(row.task_type, "hybrid");
        assert_eq!(row.energy_level, Some("deep".to_string()));
        assert!(row.agent_config.is_some());

        let task_back = Task::from(row);
        assert_eq!(task_back.task_type, TaskType::Hybrid);
        assert_eq!(task_back.energy_level, Some(EnergyLevel::Deep));
        assert!(task_back.agent_config.is_some());
    }

    #[test]
    fn test_agent_config_serde() {
        let config = AgentConfig {
            require_approval: true,
            retry_policy: Some(RetryPolicy {
                max_retries: 5,
                base_delay_secs: 120,
                exponential_backoff: true,
            }),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AgentConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.require_approval);
        assert_eq!(parsed.retry_policy.unwrap().max_retries, 5);
    }

    #[test]
    fn test_retry_policy_defaults() {
        let manual = RetryPolicy::default_for_task_type(&TaskType::Manual);
        assert_eq!(manual.max_retries, 0);

        let agentic = RetryPolicy::default_for_task_type(&TaskType::Agentic);
        assert_eq!(agentic.max_retries, 3);
        assert!(agentic.exponential_backoff);

        let hybrid = RetryPolicy::default_for_task_type(&TaskType::Hybrid);
        assert_eq!(hybrid.max_retries, 1);
        assert!(!hybrid.exponential_backoff);
    }

    #[test]
    fn test_task_type_display_and_parse() {
        assert_eq!(TaskType::Manual.to_string(), "manual");
        assert_eq!("agentic".parse::<TaskType>().unwrap(), TaskType::Agentic);
        assert!("unknown".parse::<TaskType>().is_err());
    }

    #[test]
    fn test_execution_state_display_and_parse() {
        assert_eq!(ExecutionState::Idle.to_string(), "idle");
        assert_eq!(
            "running".parse::<ExecutionState>().unwrap(),
            ExecutionState::Running
        );
        assert_eq!(
            "awaiting_approval".parse::<ExecutionState>().unwrap(),
            ExecutionState::AwaitingApproval
        );
    }

    #[test]
    fn test_energy_level_display_and_parse() {
        assert_eq!(EnergyLevel::Deep.to_string(), "deep");
        assert_eq!("low".parse::<EnergyLevel>().unwrap(), EnergyLevel::Low);
        assert!("extreme".parse::<EnergyLevel>().is_err());
    }

    #[test]
    fn test_generate_id_length() {
        let id = Task::generate_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn test_generate_id_uniqueness() {
        use std::collections::HashSet;
        let ids: HashSet<String> = (0..50).map(|_| Task::generate_id()).collect();
        assert_eq!(ids.len(), 50);
    }

    #[test]
    fn test_suggestion_action_serde() {
        let action = SuggestionAction::SetPriority { priority: 1 };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: SuggestionAction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn test_working_hours_default() {
        let wh = WorkingHours::default();
        assert_eq!(wh.start, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        assert_eq!(wh.end, NaiveTime::from_hms_opt(17, 0, 0).unwrap());
        assert_eq!(wh.lunch_start, NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    }

    #[test]
    fn test_task_status_from_str_loose() {
        assert_eq!(TaskStatus::from_str_loose("todo"), Some(TaskStatus::Todo));
        assert_eq!(TaskStatus::from_str_loose("DONE"), Some(TaskStatus::Done));
        assert_eq!(TaskStatus::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_execution_config_default() {
        let cfg = ExecutionConfig::default();
        assert_eq!(cfg.max_iterations, Some(10));
        assert!(!cfg.require_approval);
        assert_eq!(cfg.retry_policy.max_retries, 2);
    }

    #[test]
    fn test_context_snapshot_serde() {
        let snap = ContextSnapshot {
            facts: vec!["fact1".to_string()],
            parent_chain: vec!["p1".to_string()],
            sibling_titles: vec!["sibling".to_string()],
            active_blockers: vec![],
            captured_at: Utc::now(),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let parsed: ContextSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.facts.len(), 1);
    }

    #[test]
    fn test_estimation_record_from_row() {
        let row = TaskEstimationRow {
            id: "est1".to_string(),
            task_id: "task1".to_string(),
            estimated_minutes: 30,
            actual_minutes: 45,
            deviation_pct: 50.0,
            complexity_score: Some(3),
            energy_level: Some("high".to_string()),
            tags: vec!["dev".to_string()],
            completed_at: Utc::now(),
        };
        let record = EstimationRecord::from(row);
        assert_eq!(record.estimated_minutes, 30);
        assert_eq!(record.actual_minutes, 45);
        assert_eq!(record.energy_level, Some(EnergyLevel::High));
    }
}
