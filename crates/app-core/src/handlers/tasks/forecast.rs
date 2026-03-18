use desktop_shared::commands::{ForecastRiskResponse, TaskForecastResponse};
use desktop_shared::errors::ApiError;
use feature_tasks::types::{ForecastContext, Task};

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    /// Generate an AI forecast for a task's completion time.
    pub async fn task_forecast(&self, task_id: String) -> Result<TaskForecastResponse, ApiError> {
        let task_row = self
            .repos
            .tasks
            .get_or_err(&task_id)
            .await
            .map_err(map_storage_err)?;

        let task = Task::from(task_row);

        if task.estimated_minutes.is_none() {
            return Err(ApiError::new("VALIDATION", "Task has no estimate"));
        }

        let context = ForecastContext {
            min_sample_size: 5,
            lookback_days: 90,
            include_subtasks: false,
        };

        let handler = self.forecast_handler()?;
        let result = handler
            .forecast_task(&task, &context)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        let risks = result
            .risks
            .iter()
            .map(|r| ForecastRiskResponse {
                kind: format!("{:?}", r.kind).to_lowercase(),
                description: r.description.clone(),
                impact_minutes: r.impact_minutes,
            })
            .collect();

        Ok(TaskForecastResponse {
            estimated_minutes: result.estimated_minutes,
            confidence_low: result.confidence_low,
            confidence_high: result.confidence_high,
            methodology: result.methodology.name.clone(),
            sample_size: result.methodology.sample_size,
            data_quality: format!("{:?}", result.data_quality).to_lowercase(),
            risks,
        })
    }
}
