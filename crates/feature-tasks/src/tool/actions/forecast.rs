//! Forecast action: generate task/project forecasts and accuracy reports.

use common::Result;
use tools_core::ParamExtractor;
use tracing::info;

use super::super::TaskTool;
use crate::types::ForecastContext;

impl TaskTool {
    pub(crate) async fn handle_forecast_task(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.forecast_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Forecast handler not available".into())
        })?;

        let id = p.required_str("id")?;
        let task = self.require_task(id).await?;

        let context = ForecastContext {
            min_sample_size: self.config.forecast_min_sample_size,
            lookback_days: self.config.forecast_lookback_days,
            include_subtasks: false,
        };

        info!(task_id = %id, "Forecasting task");
        let forecast = handler.forecast_task(&task, &context).await?;

        let risks_str = if forecast.risks.is_empty() {
            "No risks identified.".to_string()
        } else {
            format!(
                "Risks:\n{}",
                forecast
                    .risks
                    .iter()
                    .map(|r| {
                        let impact = r
                            .impact_minutes
                            .map(|m| format!(" (+{m}min impact)"))
                            .unwrap_or_default();
                        format!("  - {:?}: {}{}", r.kind, r.description, impact)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        Ok(format!(
            "Forecast for '{}' ({}, quality: {:?}):\n\
             - Estimated: {}min\n\
             - Confidence range: {}min – {}min\n\
             {}",
            task.title,
            forecast.methodology.name,
            forecast.data_quality,
            forecast.estimated_minutes,
            forecast.confidence_low,
            forecast.confidence_high,
            risks_str,
        ))
    }

    pub(crate) async fn handle_forecast_project(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.forecast_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Forecast handler not available".into())
        })?;

        let project_id = p.required_str("project_id")?;

        let context = ForecastContext {
            min_sample_size: self.config.forecast_min_sample_size,
            lookback_days: self.config.forecast_lookback_days,
            include_subtasks: true,
        };

        info!(project_id, "Forecasting project");
        let forecast = handler.forecast_project(project_id, &context).await?;

        Ok(format!(
            "Project forecast (quality: {:?}):\n\
             - Tasks: {} remaining, {} completed\n\
             - Total estimated: {}min\n\
             - Confidence range: {}min – {}min",
            forecast.data_quality,
            forecast.remaining_tasks,
            forecast.completed_tasks,
            forecast.total_estimated_minutes,
            forecast.confidence_low,
            forecast.confidence_high,
        ))
    }

    pub(crate) async fn handle_accuracy_report(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.forecast_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Forecast handler not available".into())
        })?;

        let scope = crate::types::AccuracyScope {
            project_id: p.optional_str("project_id")?.map(String::from),
            area_id: p.optional_str("area_id")?.map(String::from),
            tags: vec![],
            lookback_days: Some(self.config.forecast_lookback_days),
        };

        let report = handler.accuracy_stats(&scope).await?;

        if report.sample_size == 0 {
            return Ok("No estimation data available yet. Complete some tasks with time tracking to see accuracy stats.".into());
        }

        let mut output = format!(
            "Estimation accuracy ({} tasks, {:?}):\n\
             - Mean deviation: {:.1}%\n\
             - Median deviation: {:.1}%\n\
             - P90 deviation: {:.1}%\n\
             - Trend: {:?}",
            report.sample_size,
            report.trend,
            report.mean_deviation_pct,
            report.median_deviation_pct,
            report.p90_deviation_pct,
            report.trend,
        );

        if !report.by_energy_level.is_empty() {
            output.push_str("\n\nBy energy level:");
            for (level, dev) in &report.by_energy_level {
                output.push_str(&format!("\n  - {level}: {dev:+.1}%"));
            }
        }

        Ok(output)
    }
}
