//! Core Task entity, base enums, and row conversions.

use ai_core_macros::AiEntity;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use storage::rows::task::*;

use super::planning::{Attachment, TimeEntry};

// ── Core Task ───────────────────────────────────────────────────────────────

/// A task with focus tracking, agentic execution, and planning support.
#[derive(Debug, Clone, Serialize, Deserialize, AiEntity)]
#[serde(rename_all = "camelCase")]
#[ai(entity_type = "task", embed_on = ["title", "description"])]
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
    pub due_date: Option<Timestamp>,
    pub tags: Vec<String>,
    pub status: String,
    pub focused_at: Option<Timestamp>,
    pub focus_deadline: Option<Timestamp>,
    pub focus_expired_count: i32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub completed_at: Option<Timestamp>,
    pub total_tracked_secs: i64,
    pub estimated_minutes: Option<i32>,
    pub calendar_event_uid: Option<String>,
    pub last_reminded_at: Option<Timestamp>,
    pub recurrence_rule: Option<String>,
    pub recurrence_parent_id: Option<String>,
    pub is_template: bool,
    pub next_instance_date: Option<Timestamp>,
    pub status_label_id: Option<String>,
    pub position: i32,
    pub group_id: Option<String>,
    pub task_type: TaskType,
    pub energy_level: Option<EnergyLevel>,
    pub estimated_focus_blocks: Option<i32>,
    pub actual_minutes: Option<i32>,
    pub complexity_score: Option<i32>,
    pub completed: bool,
    pub scheduled_start: Option<Timestamp>,
    pub scheduled_end: Option<Timestamp>,
    // Derived fields (populated by handlers, not stored)
    #[serde(default)]
    pub subtask_count: i64,
    #[serde(default)]
    pub subtask_completed_count: i64,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub time_entries: Vec<TimeEntry>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
}

impl Task {
    /// Generate an 8-char short ID from a UUID.
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }

    /// Create a default blank instance (useful as a starting point).
    pub fn default_instance() -> Self {
        let now = Timestamp::now();
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
            energy_level: None,
            estimated_focus_blocks: None,
            actual_minutes: None,
            complexity_score: None,
            completed: false,
            scheduled_start: None,
            scheduled_end: None,
            subtask_count: 0,
            subtask_completed_count: 0,
            attachments: Vec::new(),
            time_entries: Vec::new(),
            blocked_by: Vec::new(),
            blocks: Vec::new(),
        }
    }

    /// Create a default instance for tests.
    pub fn default_for_test() -> Self {
        Self::default_instance()
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
            due_date: row.due_date.map(|ts| *ts),
            tags: row.tags,
            status: row.status,
            focused_at: row.focused_at.map(|ts| *ts),
            focus_deadline: row.focus_deadline.map(|ts| *ts),
            focus_expired_count: row.focus_expired_count,
            created_at: *row.created_at,
            updated_at: *row.updated_at,
            completed_at: row.completed_at.map(|ts| *ts),
            total_tracked_secs: row.total_tracked_secs,
            estimated_minutes: row.estimated_minutes,
            calendar_event_uid: row.calendar_event_uid,
            last_reminded_at: row.last_reminded_at.map(|ts| *ts),
            recurrence_rule: row.recurrence_rule,
            recurrence_parent_id: row.recurrence_parent_id,
            is_template: row.is_template,
            next_instance_date: row.next_instance_date.map(|ts| *ts),
            status_label_id: row.status_label_id,
            position: row.position,
            group_id: row.group_id,
            task_type: row.task_type.parse::<TaskType>().unwrap_or_default(),
            energy_level: row
                .energy_level
                .as_deref()
                .and_then(|s| s.parse::<EnergyLevel>().ok()),
            estimated_focus_blocks: row.estimated_focus_blocks,
            actual_minutes: row.actual_minutes,
            complexity_score: row.complexity_score,
            completed: row.completed,
            scheduled_start: row.scheduled_start.map(|ts| *ts),
            scheduled_end: row.scheduled_end.map(|ts| *ts),
            subtask_count: 0,
            subtask_completed_count: 0,
            attachments: Vec::new(),
            time_entries: Vec::new(),
            blocked_by: Vec::new(),
            blocks: Vec::new(),
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
            due_date: task.due_date.map(|ts| ts.into()),
            tags: task.tags.clone(),
            status: task.status.clone(),
            focused_at: task.focused_at.map(|ts| ts.into()),
            focus_deadline: task.focus_deadline.map(|ts| ts.into()),
            focus_expired_count: task.focus_expired_count,
            created_at: task.created_at.into(),
            updated_at: task.updated_at.into(),
            completed_at: task.completed_at.map(|ts| ts.into()),
            total_tracked_secs: task.total_tracked_secs,
            estimated_minutes: task.estimated_minutes,
            calendar_event_uid: task.calendar_event_uid.clone(),
            last_reminded_at: task.last_reminded_at.map(|ts| ts.into()),
            recurrence_rule: task.recurrence_rule.clone(),
            recurrence_parent_id: task.recurrence_parent_id.clone(),
            is_template: task.is_template,
            next_instance_date: task.next_instance_date.map(|ts| ts.into()),
            status_label_id: task.status_label_id.clone(),
            position: task.position,
            group_id: task.group_id.clone(),
            task_type: task.task_type.to_string(),
            energy_level: task.energy_level.as_ref().map(|e| e.to_string()),
            estimated_focus_blocks: task.estimated_focus_blocks,
            actual_minutes: task.actual_minutes,
            complexity_score: task.complexity_score,
            completed: task.completed,
            scheduled_start: task.scheduled_start.map(|ts| ts.into()),
            scheduled_end: task.scheduled_end.map(|ts| ts.into()),
        }
    }
}

// ── Core Enums ──────────────────────────────────────────────────────────────

/// The type of task. Always `Manual`; kept for forward-compatibility with stored strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    #[default]
    Manual,
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "manual")
    }
}

impl FromStr for TaskType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "manual" | "standard" => Ok(Self::Manual),
            _ => Err(format!("unknown task type: {}", s)),
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
