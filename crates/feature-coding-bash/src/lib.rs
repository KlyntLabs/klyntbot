//! Background bash tasks for coding mode.
//!
//! Spec: `docs/superpowers/specs/2026-05-08-coding-background-bash-design.md`

pub mod attach;
pub mod gate;
pub mod injector;
pub mod intelligence;
pub mod migrations;
pub mod render;
pub mod ring;
pub mod spawner;
pub mod supervisor;
pub mod tools;
pub mod view;

pub use attach::{generate_attach_token, tokens_eq_constant_time};
pub use injector::BackgroundJobsInjector;
pub use intelligence::ExecutionIntelligenceInjector;
pub use supervisor::JobSupervisor;
pub use view::{BashJobView, BashJobsPanelView};

use std::sync::Arc;

use bus::DomainEventBus;
use common::Result;
use storage::repos::BashJobRepo;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

#[allow(dead_code)]
pub struct CodingBashFeature {
    supervisor: Arc<JobSupervisor>,
    repo: BashJobRepo,
    bus: Arc<DomainEventBus>,
}

impl CodingBashFeature {
    pub fn new(
        supervisor: Arc<JobSupervisor>,
        repo: BashJobRepo,
        bus: Arc<DomainEventBus>,
    ) -> Self {
        Self {
            supervisor,
            repo,
            bus,
        }
    }

    pub fn supervisor(&self) -> Arc<JobSupervisor> {
        self.supervisor.clone()
    }
}

#[async_trait::async_trait]
impl FeaturePackage for CodingBashFeature {
    fn name(&self) -> &str {
        "coding_bash"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![
            Arc::new(tools::coding_task_list::CodingTaskListTool),
            Arc::new(tools::coding_task_output::CodingTaskOutputTool),
            Arc::new(tools::coding_task_stop::CodingTaskStopTool),
            Arc::new(tools::coding_task_stdin::CodingTaskStdinTool),
            Arc::new(tools::coding_task_resize::CodingTaskResizeTool),
        ]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![migrations::coding_background_jobs_migration()]
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
