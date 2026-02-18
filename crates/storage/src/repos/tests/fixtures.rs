//! Shared test fixtures for storage repo tests.
//!
//! Provides ephemeral Postgres containers and sample domain objects
//! for use across all repo test modules.

// TODO: Uncomment when dependencies are added to Cargo.toml
// use testcontainers::{clients::Cli, images::postgres::Postgres, Container};
// use crate::{StoragePool, Repos};
// use crate::repos::*;

// Spin up an ephemeral Postgres 16 container and return a connected StoragePool.
//
// Important: The returned container MUST be held alive (not dropped) for the
// duration of the test. Dropping it shuts down Postgres.
//
// pub async fn test_pool() -> (StoragePool, Container<'static, Postgres>) { ... }

// Create a Repos aggregate from a test pool.
// pub async fn test_repos() -> (Repos, Container<'static, Postgres>) { ... }

// ---------------------------------------------------------------------------
// Sample domain objects (to be uncommented when testcontainers are wired up)
// ---------------------------------------------------------------------------

// sample_todo() -> tools::todo_types::Todo
// sample_project() -> tools::project_types::Project
// sample_goal() -> goal::Goal
// sample_plan_with_steps() -> (plan::Plan, Vec<plan::PlanStep>)
// sample_embedding_384(seed: u8) -> Vec<f32>
