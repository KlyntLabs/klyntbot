//! Unit tests for all storage repository implementations.
//!
//! Each repo module tests its repository in isolation against
//! an ephemeral SQLite pool (via `StoragePool::connect(tempdir)`).
//!
//! Test naming convention: `{repo}_{operation}_{scenario}`

// Finance module repo tests — TDD skeletons, fail until implementation is added.
pub mod finance_account_repo_tests;
pub mod finance_budget_repo_tests;
pub mod finance_goal_repo_tests;
pub mod finance_investment_repo_tests;
pub mod finance_liability_repo_tests;
pub mod finance_transaction_repo_tests;
pub mod retrieval_feedback_tests;
pub mod scheduled_fires_tests;
pub mod task_alarms_tests;
pub mod task_recurrence_tests;
