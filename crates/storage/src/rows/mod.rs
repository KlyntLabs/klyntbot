//! Row structs for `sqlx::FromRow` deserialization.

pub mod agent_task;
pub mod area;
pub mod cron;
pub mod custom_column;
pub mod entity_link;
pub mod held_notification;
pub mod key_result;
pub mod learning;
pub mod notification_log;
pub mod objective;
pub mod project;
pub mod project_source;
pub mod reforge_state;
pub mod scheduled_fire;
pub mod session;
pub mod session_context;
pub mod session_memory;
pub mod skill_version;
pub mod status;
pub mod subagent_instance;
pub mod task;
pub mod task_alarm;
pub mod task_group;
pub mod task_recurrence;
pub mod tool_usage;
pub mod trial;
pub mod usage;

pub use custom_column::{CustomColumnRow, CustomColumnValueRow};
pub use entity_link::EntityLinkRow;
pub use project_source::ProjectSourceRow;
pub use reforge_state::ReforgeStateRow;
pub use session_memory::SessionMemoryRow;
pub use skill_version::SkillVersionRow;
pub use status::{StatusLabelRow, StatusWorkflowRow};
pub use subagent_instance::{SubagentInstanceRow, SubagentStatus};
pub use task_group::TaskGroupRow;

#[cfg(test)]
mod serialization_tests;
