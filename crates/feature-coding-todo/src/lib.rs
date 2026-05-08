//! Coding TodoWrite — LLM-first scratchpad for multi-step coding tasks.
//!
//! See `docs/superpowers/specs/2026-05-07-coding-todowrite-design.md`.

pub mod diff;
pub mod errors;
pub mod events;
pub mod id;
pub mod injector;
pub mod migrations;
pub mod plan_mode;
pub mod render;
pub mod tool;
pub mod types;
pub mod util;
pub mod validation;
pub mod view;

pub use events::CodingTodoEvent;
pub use injector::PlanModeInjector;
pub use view::{CodingTodoView, PlanModeView};

use std::sync::Arc;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub struct CodingTodoFeature {
    repo: storage::repos::TodoRepo,
    bus: Arc<bus::DomainEventBus>,
}

impl CodingTodoFeature {
    pub fn new(repo: storage::repos::TodoRepo, bus: Arc<bus::DomainEventBus>) -> Self {
        Self { repo, bus }
    }

    pub fn migration_sql() -> String {
        migrations::coding_todo_migration().sql
    }
}

#[async_trait::async_trait]
impl FeaturePackage for CodingTodoFeature {
    fn name(&self) -> &str {
        "coding_todo"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![Arc::new(tool::CodingTodoTool::new(
            self.repo.clone(),
            self.bus.clone(),
        ))]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![migrations::coding_todo_migration()]
    }

    async fn health_check(&self) -> common::Result<HealthStatus> {
        // TODO: add a simple health check query
        Ok(HealthStatus::Healthy)
    }
}
