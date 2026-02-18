//! GraphQL types wrapping goal::Goal and related types.

use async_graphql::{Enum, InputObject, Object, SimpleObject, ID};
use chrono::{DateTime, Utc};
use goal::{Goal as DomainGoal, GoalStatus as DomainGoalStatus, Metric as DomainMetric};
use uuid::Uuid;

// ── Metric ───────────────────────────────────────────────────────────────────

/// A quantifiable progress indicator for a goal.
#[derive(SimpleObject, Clone)]
pub struct GqlMetric {
    pub name: String,
    pub current: f64,
    pub target: f64,
    pub unit: String,
    pub progress_percentage: f64,
}

impl From<DomainMetric> for GqlMetric {
    fn from(m: DomainMetric) -> Self {
        let pct = m.progress_percentage();
        Self {
            name: m.name,
            current: m.current,
            target: m.target,
            unit: m.unit,
            progress_percentage: pct,
        }
    }
}

// ── GoalStatus ───────────────────────────────────────────────────────────────

/// Status of a goal.
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum GqlGoalStatus {
    Active,
    Paused,
    Achieved,
    Abandoned,
}

impl From<DomainGoalStatus> for GqlGoalStatus {
    fn from(s: DomainGoalStatus) -> Self {
        match s {
            DomainGoalStatus::Active => GqlGoalStatus::Active,
            DomainGoalStatus::Paused => GqlGoalStatus::Paused,
            DomainGoalStatus::Achieved => GqlGoalStatus::Achieved,
            DomainGoalStatus::Abandoned => GqlGoalStatus::Abandoned,
        }
    }
}

impl From<GqlGoalStatus> for DomainGoalStatus {
    fn from(s: GqlGoalStatus) -> Self {
        match s {
            GqlGoalStatus::Active => DomainGoalStatus::Active,
            GqlGoalStatus::Paused => DomainGoalStatus::Paused,
            GqlGoalStatus::Achieved => DomainGoalStatus::Achieved,
            GqlGoalStatus::Abandoned => DomainGoalStatus::Abandoned,
        }
    }
}

// ── GqlGoal ──────────────────────────────────────────────────────────────────

/// A strategic goal tracked in the dashboard.
pub struct GqlGoal(pub DomainGoal);

#[Object]
impl GqlGoal {
    async fn id(&self) -> ID {
        self.0.id.to_string().into()
    }

    async fn title(&self) -> &str {
        &self.0.title
    }

    async fn description(&self) -> &str {
        &self.0.description
    }

    async fn status(&self) -> GqlGoalStatus {
        self.0.status.clone().into()
    }

    async fn priority(&self) -> i32 {
        self.0.priority as i32
    }

    async fn target_date(&self) -> Option<DateTime<Utc>> {
        self.0.target_date
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }

    async fn updated_at(&self) -> DateTime<Utc> {
        self.0.updated_at
    }

    async fn metrics(&self) -> Vec<GqlMetric> {
        self.0
            .metrics
            .iter()
            .cloned()
            .map(GqlMetric::from)
            .collect()
    }

    async fn linked_project_ids(&self) -> Vec<String> {
        self.0
            .linked_project_ids
            .iter()
            .map(Uuid::to_string)
            .collect()
    }

    async fn progress_percentage(&self) -> f64 {
        if self.0.metrics.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.0.metrics.iter().map(|m| m.progress_percentage()).sum();
        sum / self.0.metrics.len() as f64
    }
}

// ── Input types ───────────────────────────────────────────────────────────────

/// Input for creating a new goal.
#[derive(InputObject)]
pub struct CreateGoalInput {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub target_date: Option<DateTime<Utc>>,
    pub status: Option<GqlGoalStatus>,
}

/// Input for updating an existing goal.
#[derive(InputObject)]
pub struct UpdateGoalInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub target_date: Option<DateTime<Utc>>,
    pub status: Option<GqlGoalStatus>,
}

/// Filter for listing goals.
#[derive(InputObject, Default)]
pub struct GoalFilter {
    pub status: Option<GqlGoalStatus>,
}
