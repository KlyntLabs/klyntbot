//! Forecast handler trait for estimation accuracy and project forecasting.
//!
//! Defined here (Layer 4) for dependency inversion. The implementation
//! lives in the agent crate (Layer 5) and uses historical estimation
//! data to predict task completion times and project timelines.

use async_trait::async_trait;
use common::Result;

use crate::types::{
    AccuracyReport, AccuracyScope, ForecastContext, ProjectForecast, Task, TaskForecast,
};

/// Handler for generating task and project forecasts.
///
/// Uses historical estimation data (actual vs. estimated minutes)
/// to predict completion times, identify risks, and report on
/// estimation accuracy trends.
#[async_trait]
pub trait ForecastHandler: Send + Sync {
    /// Forecast completion time for a single task.
    ///
    /// Analyzes historical data for similar tasks (by tags, complexity,
    /// energy level) to predict how long this task will actually take.
    async fn forecast_task(&self, task: &Task, context: &ForecastContext) -> Result<TaskForecast>;

    /// Forecast completion timeline for an entire project.
    ///
    /// Aggregates task-level forecasts and accounts for dependencies,
    /// parallelism, and historical project-level estimation patterns.
    async fn forecast_project(
        &self,
        project_id: &str,
        context: &ForecastContext,
    ) -> Result<ProjectForecast>;

    /// Report on estimation accuracy over time.
    ///
    /// Analyzes historical estimation records within the given scope
    /// to compute deviation statistics and identify trends.
    async fn accuracy_stats(&self, scope: &AccuracyScope) -> Result<AccuracyReport>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_forecast_handler_is_object_safe() {
        fn _check(_: Arc<dyn ForecastHandler>) {}
    }
}
