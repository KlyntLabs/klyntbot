//! Unit tests for all storage repository implementations.
//!
//! Each repo module tests its repository in isolation against
//! an ephemeral SQLite pool (via `StoragePool::connect(tempdir)`).
//!
//! Test naming convention: `{repo}_{operation}_{scenario}`

pub mod held_notifications_tests;
pub mod notification_log_tests;
pub mod retrieval_feedback_tests;
pub mod scheduled_fires_tests;
pub mod session_schema_tests;
pub mod task_alarms_tests;
pub mod task_recurrence_tests;
