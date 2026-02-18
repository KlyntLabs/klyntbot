//! GraphQL input types for filtering, creating, and updating todos/projects.

use async_graphql::{Enum, InputObject, ID};
use chrono::{DateTime, Utc};

use super::types::{GqlProjectColor, GqlProjectStatus, GqlTodoStatus};

// ── Todo Filters ──────────────────────────────────────────────────────────────

/// Filter criteria for the `todos` query.
#[derive(Debug, Default, InputObject)]
pub struct TodoFilterInput {
    pub status: Option<GqlTodoStatus>,
    pub project_id: Option<ID>,
    pub parent_id: Option<ID>,
    pub tag: Option<String>,
    pub priority_min: Option<i32>,
    pub priority_max: Option<i32>,
    pub due_before: Option<DateTime<Utc>>,
    pub due_after: Option<DateTime<Utc>>,
}

/// Sort order for todo lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum, Default)]
pub enum TodoSortOrder {
    #[default]
    CreatedAsc,
    CreatedDesc,
    PriorityAsc,
    DueDateAsc,
}

// ── Project Filters ───────────────────────────────────────────────────────────

/// Filter criteria for the `projects` query.
#[derive(Debug, Default, InputObject)]
pub struct ProjectFilterInput {
    pub status: Option<GqlProjectStatus>,
    pub tag: Option<String>,
}

// ── Create / Update Inputs ────────────────────────────────────────────────────

/// Input for creating a new todo.
#[derive(Debug, InputObject)]
pub struct CreateTodoInput {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Option<Vec<String>>,
    pub project_id: Option<ID>,
    pub parent_id: Option<ID>,
}

/// Input for partially updating an existing todo.
#[derive(Debug, InputObject)]
pub struct UpdateTodoInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<GqlTodoStatus>,
}

/// Input for creating a new project.
#[derive(Debug, InputObject)]
pub struct CreateProjectInput {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<GqlProjectColor>,
    pub tags: Option<Vec<String>>,
}

/// Input for partially updating an existing project.
#[derive(Debug, InputObject)]
pub struct UpdateProjectInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<GqlProjectColor>,
    pub tags: Option<Vec<String>>,
    pub status: Option<GqlProjectStatus>,
}
