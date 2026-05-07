use desktop_shared::commands::{ObjectiveResponse, ProjectResponse, TodayTaskResponse};
use desktop_shared::errors::ApiError;
use storage::{ProjectFilter, TaskRow};

use crate::errors::map_storage_err;
use crate::state::AppCore;

use super::converters::{action_to_today_task, kr_to_response, objective_to_response};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn today_tasks(&self) -> Result<Vec<TodayTaskResponse>, ApiError> {
        let now = jiff::Timestamp::now();
        let rows = feature_tasks::api::today_tasks(&self.repos.tasks)
            .await
            .map_err(map_storage_err)?;

        Ok(rows
            .iter()
            .map(|row| action_to_today_task(row, now))
            .collect())
    }

    /// Get the next upcoming task (earliest `due_date > now`, not completed).
    /// Used by the tray countdown to show a task deadline timer.
    #[tracing::instrument(skip(self))]
    pub async fn next_upcoming_task(&self) -> Option<TaskRow> {
        feature_tasks::api::next_upcoming_task(&self.repos.tasks)
            .await
            .ok()
            .flatten()
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn project_list_for_tasks(
        &self,
        area_id: Option<String>,
    ) -> Result<Vec<ProjectResponse>, ApiError> {
        let filter = ProjectFilter {
            area_id,
            status: Some("active".to_string()),
            ..Default::default()
        };
        let projects = self
            .repos
            .projects
            .list(&filter)
            .await
            .map_err(map_storage_err)?;

        // Fetch all project metadata concurrently to avoid N+1
        let response_futures = projects
            .iter()
            .map(|p| super::super::projects::build_project_response(self, p));
        let results = futures_util::future::try_join_all(response_futures).await?;
        Ok(results)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn objective_list_for_tasks(
        &self,
        project_id: Option<String>,
    ) -> Result<Vec<ObjectiveResponse>, ApiError> {
        let pairs = feature_tasks::api::objective_list(
            &self.repos.objectives,
            &self.repos.key_results,
            project_id.as_deref(),
        )
        .await
        .map_err(map_storage_err)?;

        let results = pairs
            .into_iter()
            .map(|(o, kr_rows)| {
                let krs = if kr_rows.is_empty() {
                    None
                } else {
                    Some(kr_rows.iter().map(kr_to_response).collect())
                };
                objective_to_response(&o, krs)
            })
            .collect();
        Ok(results)
    }
}
