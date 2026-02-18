//! Unit tests for all storage repository implementations.
//!
//! Each repo module below tests its repository in isolation against
//! an ephemeral Postgres container (testcontainers-rs).
//!
//! Test naming convention: `{repo}_{operation}_{scenario}`

pub mod calendar_sync_repo_tests;
pub mod conv_embedding_repo_tests;
pub mod cron_repo_tests;
pub mod embedding_repo_tests;
pub mod fixtures;
pub mod goal_repo_tests;
pub mod outcome_repo_tests;
pub mod plan_repo_tests;
pub mod project_repo_tests;
pub mod session_repo_tests;
pub mod strategy_repo_tests;
pub mod todo_repo_tests;
pub mod usage_repo_tests;
