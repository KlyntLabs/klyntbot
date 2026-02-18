//! GraphQL output types wrapping the core domain models.

pub mod chat;
pub mod config;
pub mod goal;
pub mod plan;
pub mod project;
pub mod system;
pub mod todo;

pub use chat::{GqlCalendarSyncResult, GqlChatMessage, MessageRole};
pub use config::{GqlConfig, GqlConfigUpdateResult};
pub use goal::{CreateGoalInput, GqlGoal, GqlGoalStatus, GqlMetric, GoalFilter, UpdateGoalInput};
pub use plan::{
    CreatePlanInput, CreatePlanStepInput, GqlBacktrackEntry, GqlPlan, GqlPlanStatus, GqlPlanStep,
    GqlStepStatus, PlanFilter,
};
pub use project::{GqlProject, GqlProjectColor, GqlProjectStatus};
pub use system::{GqlCronJob, GqlSession, GqlSystemStatus};
pub use todo::{
    GqlAttachment, GqlAttachmentType, GqlSearchMode, GqlSearchResult, GqlTimeEntry, GqlTodo,
    GqlTodoNode, GqlTodoStatus, GqlTodoStatusCount, GqlTodoSummary,
};
