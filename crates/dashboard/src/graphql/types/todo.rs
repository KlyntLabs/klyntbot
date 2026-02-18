//! GraphQL types wrapping `tools::todo_types` for the dashboard API.

use async_graphql::{Enum, Object, SimpleObject, ID};
use chrono::{DateTime, Utc};

use tools::todo_types::{
    Attachment, AttachmentType, TimeEntry, TimeEntrySource, Todo, TodoStatus,
};

// ── Status Enum ─────────────────────────────────────────────────────────────

/// Task status exposed via GraphQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum GqlTodoStatus {
    Todo,
    Doing,
    Done,
    Archived,
}

impl From<TodoStatus> for GqlTodoStatus {
    fn from(s: TodoStatus) -> Self {
        match s {
            TodoStatus::Todo => Self::Todo,
            TodoStatus::Doing => Self::Doing,
            TodoStatus::Done => Self::Done,
            TodoStatus::Archived => Self::Archived,
        }
    }
}

impl From<GqlTodoStatus> for TodoStatus {
    fn from(s: GqlTodoStatus) -> Self {
        match s {
            GqlTodoStatus::Todo => Self::Todo,
            GqlTodoStatus::Doing => Self::Doing,
            GqlTodoStatus::Done => Self::Done,
            GqlTodoStatus::Archived => Self::Archived,
        }
    }
}

// ── Attachment Types ─────────────────────────────────────────────────────────

/// Attachment type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum GqlAttachmentType {
    File,
    Url,
    Note,
}

impl From<AttachmentType> for GqlAttachmentType {
    fn from(a: AttachmentType) -> Self {
        match a {
            AttachmentType::File => Self::File,
            AttachmentType::Url => Self::Url,
            AttachmentType::Note => Self::Note,
        }
    }
}

/// A file, URL, or text note attached to a todo.
#[derive(Debug, Clone, SimpleObject)]
pub struct GqlAttachment {
    pub id: ID,
    pub attachment_type: GqlAttachmentType,
    pub title: Option<String>,
    pub value: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl From<Attachment> for GqlAttachment {
    fn from(a: Attachment) -> Self {
        Self {
            id: ID(a.id),
            attachment_type: a.attachment_type.into(),
            title: a.title,
            value: a.value,
            tags: a.tags,
            created_at: a.created_at,
        }
    }
}

// ── Time Entry ───────────────────────────────────────────────────────────────

/// Whether a time entry was created by focus or logged manually.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum GqlTimeEntrySource {
    Focus,
    Manual,
}

impl From<TimeEntrySource> for GqlTimeEntrySource {
    fn from(s: TimeEntrySource) -> Self {
        match s {
            TimeEntrySource::Focus => Self::Focus,
            TimeEntrySource::Manual => Self::Manual,
        }
    }
}

/// A recorded work session on a todo.
#[derive(Debug, Clone, SimpleObject)]
pub struct GqlTimeEntry {
    pub id: ID,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i32>,
    pub note: Option<String>,
    pub source: GqlTimeEntrySource,
}

impl From<TimeEntry> for GqlTimeEntry {
    fn from(e: TimeEntry) -> Self {
        Self {
            id: ID(e.id),
            started_at: e.started_at,
            ended_at: e.ended_at,
            duration_secs: e.duration_secs.map(|d| d as i32),
            note: e.note,
            source: e.source.into(),
        }
    }
}

// ── Todo ─────────────────────────────────────────────────────────────────────

/// A task/todo item with all fields exposed via GraphQL.
#[derive(Debug, Clone)]
pub struct GqlTodo(pub Todo);

#[Object]
impl GqlTodo {
    async fn id(&self) -> ID {
        ID(self.0.id.clone())
    }
    async fn title(&self) -> &str {
        &self.0.title
    }
    async fn description(&self) -> Option<&str> {
        self.0.description.as_deref()
    }
    async fn priority(&self) -> Option<i32> {
        self.0.priority.map(|p| p as i32)
    }
    async fn due_date(&self) -> Option<DateTime<Utc>> {
        self.0.due_date
    }
    async fn tags(&self) -> &[String] {
        &self.0.tags
    }
    async fn status(&self) -> GqlTodoStatus {
        self.0.status.into()
    }
    async fn focused_at(&self) -> Option<DateTime<Utc>> {
        self.0.focused_at
    }
    async fn focus_deadline(&self) -> Option<DateTime<Utc>> {
        self.0.focus_deadline
    }
    async fn focus_expired_count(&self) -> i32 {
        self.0.focus_expired_count as i32
    }
    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }
    async fn updated_at(&self) -> DateTime<Utc> {
        self.0.updated_at
    }
    async fn completed_at(&self) -> Option<DateTime<Utc>> {
        self.0.completed_at
    }
    async fn parent_id(&self) -> Option<ID> {
        self.0.parent_id.as_ref().map(|id| ID(id.clone()))
    }
    async fn project_id(&self) -> Option<ID> {
        self.0.project_id.as_ref().map(|id| ID(id.clone()))
    }
    async fn attachments(&self) -> Vec<GqlAttachment> {
        self.0.attachments.iter().cloned().map(Into::into).collect()
    }
    async fn time_entries(&self) -> Vec<GqlTimeEntry> {
        self.0.time_entries.iter().cloned().map(Into::into).collect()
    }
    async fn total_tracked_secs(&self) -> i32 {
        self.0.total_tracked_secs as i32
    }
    async fn estimated_minutes(&self) -> Option<i32> {
        self.0.estimated_minutes.map(|m| m as i32)
    }
    async fn calendar_event_uid(&self) -> Option<&str> {
        self.0.calendar_event_uid.as_deref()
    }
    async fn recurrence_rule(&self) -> Option<&str> {
        self.0.recurrence_rule.as_deref()
    }
    async fn is_template(&self) -> bool {
        self.0.is_template
    }
    async fn blocked_by(&self) -> Vec<ID> {
        self.0.blocked_by.iter().map(|id| ID(id.clone())).collect()
    }
    async fn blocks(&self) -> Vec<ID> {
        self.0.blocks.iter().map(|id| ID(id.clone())).collect()
    }
}

impl From<Todo> for GqlTodo {
    fn from(t: Todo) -> Self {
        Self(t)
    }
}

// ── Paginated connection ──────────────────────────────────────────────────────

/// Paginated list of todos with total count.
#[derive(Debug, Clone, SimpleObject)]
pub struct GqlTodoConnection {
    pub nodes: Vec<GqlTodo>,
    pub total_count: i32,
}

// ── Tree node ─────────────────────────────────────────────────────────────────

/// A node in the todo tree (hierarchical view).
#[derive(Debug, Clone)]
pub struct GqlTodoNode {
    pub todo: GqlTodo,
    pub children: Vec<GqlTodoNode>,
}

#[Object]
impl GqlTodoNode {
    async fn todo(&self) -> &GqlTodo {
        &self.todo
    }
    async fn children(&self) -> &[GqlTodoNode] {
        &self.children
    }
}

// ── Summary ───────────────────────────────────────────────────────────────────

/// Count of todos for a single status bucket.
#[derive(Debug, Clone, SimpleObject)]
pub struct GqlTodoStatusCount {
    pub status: GqlTodoStatus,
    pub count: i32,
}

/// Aggregate statistics for the todo collection.
#[derive(Debug, Clone, SimpleObject)]
pub struct GqlTodoSummary {
    pub total: i32,
    pub by_status: Vec<GqlTodoStatusCount>,
    pub overdue_count: i32,
    pub upcoming_week_count: i32,
    pub focus_active: bool,
}

// ── Search ────────────────────────────────────────────────────────────────────

/// Search mode for the `searchTodos` query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum, Default)]
pub enum GqlSearchMode {
    #[default]
    Keyword,
    Semantic,
    Hybrid,
}

/// A single search result with relevance score.
#[derive(Debug, Clone)]
pub struct GqlSearchResult {
    pub todo: GqlTodo,
    pub score: Option<f64>,
    pub match_type: GqlSearchMode,
}

#[Object]
impl GqlSearchResult {
    async fn todo(&self) -> &GqlTodo {
        &self.todo
    }
    async fn score(&self) -> Option<f64> {
        self.score
    }
    async fn match_type(&self) -> GqlSearchMode {
        self.match_type
    }
}
