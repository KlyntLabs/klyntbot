//! Unit tests for all storage repository implementations.
//!
//! Each repo module below tests its repository in isolation against
//! an ephemeral Postgres container (testcontainers-rs).
//!
//! Test naming convention: `{repo}_{operation}_{scenario}`

pub mod fixtures;
pub mod todo_repo_tests;

// Finance module repo tests — TDD skeletons, fail until implementation is added.
pub mod finance_account_repo_tests;
pub mod finance_budget_repo_tests;
pub mod finance_goal_repo_tests;
pub mod finance_investment_repo_tests;
pub mod finance_liability_repo_tests;
pub mod finance_transaction_repo_tests;
