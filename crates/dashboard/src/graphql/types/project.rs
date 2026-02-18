//! GraphQL types wrapping `tools::project_types` for the dashboard API.

use async_graphql::{Enum, Object, ID};
use chrono::{DateTime, Utc};

use tools::project_types::{Project, ProjectColor, ProjectStatus};

// ── Status Enum ───────────────────────────────────────────────────────────────

/// Project lifecycle status exposed via GraphQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum GqlProjectStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

impl From<ProjectStatus> for GqlProjectStatus {
    fn from(s: ProjectStatus) -> Self {
        match s {
            ProjectStatus::Active => Self::Active,
            ProjectStatus::Paused => Self::Paused,
            ProjectStatus::Completed => Self::Completed,
            ProjectStatus::Archived => Self::Archived,
        }
    }
}

impl From<GqlProjectStatus> for ProjectStatus {
    fn from(s: GqlProjectStatus) -> Self {
        match s {
            GqlProjectStatus::Active => Self::Active,
            GqlProjectStatus::Paused => Self::Paused,
            GqlProjectStatus::Completed => Self::Completed,
            GqlProjectStatus::Archived => Self::Archived,
        }
    }
}

// ── Color Enum ────────────────────────────────────────────────────────────────

/// Visual color label for a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum GqlProjectColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Gray,
}

impl From<ProjectColor> for GqlProjectColor {
    fn from(c: ProjectColor) -> Self {
        match c {
            ProjectColor::Red => Self::Red,
            ProjectColor::Orange => Self::Orange,
            ProjectColor::Yellow => Self::Yellow,
            ProjectColor::Green => Self::Green,
            ProjectColor::Blue => Self::Blue,
            ProjectColor::Purple => Self::Purple,
            ProjectColor::Gray => Self::Gray,
        }
    }
}

impl From<GqlProjectColor> for ProjectColor {
    fn from(c: GqlProjectColor) -> Self {
        match c {
            GqlProjectColor::Red => Self::Red,
            GqlProjectColor::Orange => Self::Orange,
            GqlProjectColor::Yellow => Self::Yellow,
            GqlProjectColor::Green => Self::Green,
            GqlProjectColor::Blue => Self::Blue,
            GqlProjectColor::Purple => Self::Purple,
            GqlProjectColor::Gray => Self::Gray,
        }
    }
}

// ── Project ───────────────────────────────────────────────────────────────────

/// A project grouping todos.
#[derive(Debug, Clone)]
pub struct GqlProject(pub Project);

#[Object]
impl GqlProject {
    async fn id(&self) -> ID {
        ID(self.0.id.clone())
    }
    async fn name(&self) -> &str {
        &self.0.name
    }
    async fn description(&self) -> Option<&str> {
        self.0.description.as_deref()
    }
    async fn color(&self) -> GqlProjectColor {
        self.0.color.into()
    }
    async fn tags(&self) -> &[String] {
        &self.0.tags
    }
    async fn status(&self) -> GqlProjectStatus {
        self.0.status.into()
    }
    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }
    async fn updated_at(&self) -> DateTime<Utc> {
        self.0.updated_at
    }
}

impl From<Project> for GqlProject {
    fn from(p: Project) -> Self {
        Self(p)
    }
}
