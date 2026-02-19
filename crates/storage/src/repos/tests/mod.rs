//! Unit tests for all storage repository implementations.
//!
//! Each repo module below tests its repository in isolation against
//! an ephemeral Postgres container (testcontainers-rs).
//!
//! Test naming convention: `{repo}_{operation}_{scenario}`

pub mod fixtures;
pub mod todo_repo_tests;
