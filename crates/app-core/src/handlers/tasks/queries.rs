use desktop_shared::commands::{ObjectiveResponse, ProjectResponse, TodayTaskResponse};
use desktop_shared::errors::ApiError;
use storage::{ProjectFilter, TaskFilter, TaskRow};

use crate::errors::map_storage_err;
use crate::state::AppCore;

use super::converters::{action_to_today_task, kr_to_response, objective_to_response};

impl AppCore {
    pub async fn today_tasks(&self) -> Result<Vec<TodayTaskResponse>, ApiError> {
        let now = jiff::Timestamp::now();
        let today_date = now.to_zoned(jiff::tz::TimeZone::UTC).date();
        let start_of_today: jiff::Timestamp = today_date
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .map(|z| z.timestamp())
            .unwrap_or(now);
        let start_of_tomorrow = start_of_today
            .checked_add(jiff::SignedDuration::from_secs(86400))
            .unwrap_or(start_of_today);

        // Run all three queries concurrently — SqlitePool is safe for parallel reads
        let doing_filter = TaskFilter {
            status: Some("doing".to_string()),
            ..Default::default()
        };
        let due_today_filter = TaskFilter {
            due_after: Some(start_of_today),
            due_before: Some(start_of_tomorrow),
            ..Default::default()
        };
        let (doing, due_today, overdue) = tokio::try_join!(
            self.repos.tasks.list(&doing_filter),
            self.repos.tasks.list(&due_today_filter),
            self.repos.tasks.overdue(),
        )
        .map_err(map_storage_err)?;

        // Merge + deduplicate by ID
        let mut seen = std::collections::HashSet::new();
        let mut all_rows: Vec<TaskRow> = Vec::new();
        for row in overdue.into_iter().chain(doing).chain(due_today) {
            if !row.completed && row.status != "archived" && seen.insert(row.id.clone()) {
                all_rows.push(row);
            }
        }

        // Sort: overdue first, then by priority (P1 first), then by due_date
        all_rows.sort_by(|a, b| {
            let now_jiff = now;
            let a_overdue = a.due_date.is_some_and(|d| *d < now_jiff) as u8;
            let b_overdue = b.due_date.is_some_and(|d| *d < now_jiff) as u8;
            b_overdue
                .cmp(&a_overdue)
                .then(a.priority.unwrap_or(99).cmp(&b.priority.unwrap_or(99)))
                .then(a.due_date.cmp(&b.due_date))
        });

        Ok(all_rows
            .iter()
            .map(|row| action_to_today_task(row, now))
            .collect())
    }

    /// Get the next upcoming task (earliest `due_date > now`, not completed).
    /// Used by the tray countdown to show a task deadline timer.
    pub async fn next_upcoming_task(&self) -> Option<TaskRow> {
        let filter = TaskFilter {
            due_after: Some(jiff::Timestamp::now()),
            limit: Some(1),
            ..Default::default()
        };
        let tasks = self.repos.tasks.list(&filter).await.ok()?;
        tasks.into_iter().find(|t| !t.completed)
    }

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

    pub async fn objective_list_for_tasks(
        &self,
        project_id: Option<String>,
    ) -> Result<Vec<ObjectiveResponse>, ApiError> {
        let objectives = self
            .repos
            .objectives
            .list(project_id.as_deref(), None)
            .await
            .map_err(map_storage_err)?;

        // Fetch all key results concurrently to avoid N+1
        let kr_futures = objectives
            .iter()
            .map(|o| self.repos.key_results.list(Some(&o.id)));
        let all_krs = futures_util::future::try_join_all(kr_futures)
            .await
            .map_err(map_storage_err)?;

        let results = objectives
            .iter()
            .zip(all_krs)
            .map(|(o, kr_rows)| {
                let krs = if kr_rows.is_empty() {
                    None
                } else {
                    Some(kr_rows.iter().map(kr_to_response).collect())
                };
                objective_to_response(o, krs)
            })
            .collect();
        Ok(results)
    }
}
