//! Tracking handlers — categories, apps, goals, time entries, insights, projects.

use chrono::Utc;
use desktop_shared::commands::{
    ActivityCategoryResponse, CategoryRulesResponse, GoalProgressResponse, InsightCardResponse,
    ProductivityProjectResponse, TimeEntryResponse, TrackedAppResponse,
};
use desktop_shared::errors::ApiError;

use super::converters::{
    insight_to_response, project_to_response, rules_from_response, rules_to_response,
};
use crate::errors::{map_prod_err, parse_date_or_err};
use crate::state::AppCore;

impl AppCore {
    pub async fn productivity_categories(&self) -> Result<Vec<ActivityCategoryResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let categories = repos.categories.list_all().await.map_err(map_prod_err)?;
        Ok(categories
            .into_iter()
            .map(|c| ActivityCategoryResponse {
                id: c.id,
                name: c.name,
                category_type: c.category_type.to_string(),
                color: c.color,
                icon: c.icon,
                is_system: c.is_system,
                rules: c.rules.map(rules_to_response),
            })
            .collect())
    }

    pub async fn productivity_tracked_apps(&self) -> Result<Vec<TrackedAppResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let rows = repos.events.tracked_apps().await.map_err(map_prod_err)?;
        Ok(rows
            .into_iter()
            .map(|r| TrackedAppResponse {
                display_name: r.display_name,
                app_name: r.app_name,
                site_name: r.site_name,
                category_id: r.category_id,
                category_name: r.category_name,
                total_secs: r.total_secs,
                event_count: r.event_count,
            })
            .collect())
    }

    pub async fn productivity_goals(&self) -> Result<Vec<GoalProgressResponse>, ApiError> {
        let aggregator = self.aggregator()?;
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let results = aggregator.check_goals(&today).await.map_err(map_prod_err)?;
        Ok(results
            .into_iter()
            .map(|(goal, current, met)| GoalProgressResponse {
                id: goal.id.unwrap_or(0),
                goal_type: goal.goal_type.to_string(),
                metric: goal.metric.to_string(),
                target_value: goal.target_value,
                current_value: current,
                met,
                project_id: goal.project_id.clone(),
            })
            .collect())
    }

    pub async fn productivity_time_entries(
        &self,
        date: String,
    ) -> Result<Vec<TimeEntryResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let start = parse_date_or_err(&date)?;
        let end = start + chrono::Duration::days(1);
        let entries = repos
            .time_entries
            .list_range(&start, &end)
            .await
            .map_err(map_prod_err)?;
        Ok(entries
            .into_iter()
            .map(|e| TimeEntryResponse {
                id: e.id.unwrap_or(0),
                description: e.description,
                category_id: e.category_id,
                project_id: e.project_id,
                started_at: e.started_at,
                duration_secs: e.duration_secs,
                source: e.source,
            })
            .collect())
    }

    pub async fn productivity_goal_create(
        &self,
        goal_type: String,
        metric: String,
        target_value: f64,
    ) -> Result<GoalProgressResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let gt: feature_productivity::types::GoalType = goal_type
            .parse()
            .map_err(|_| ApiError::new("VALIDATION", "Invalid goal_type. Use: daily, weekly"))?;
        let gm: feature_productivity::types::GoalMetric = metric.parse().map_err(|_| {
            ApiError::new(
                "VALIDATION",
                "Invalid metric. Use: productive_hours, focus_sessions, productivity_score, max_distracting_mins",
            )
        })?;
        let goal = feature_productivity::types::ProductivityGoal {
            id: None,
            goal_type: gt,
            metric: gm,
            target_value,
            enabled: true,
            project_id: None,
            created_at: Utc::now(),
        };
        let id = repos.goals.insert(&goal).await.map_err(map_prod_err)?;
        Ok(GoalProgressResponse {
            id,
            goal_type: goal.goal_type.to_string(),
            metric: goal.metric.to_string(),
            target_value: goal.target_value,
            current_value: 0.0,
            met: false,
            project_id: goal.project_id,
        })
    }

    pub async fn productivity_goal_delete(&self, id: i64) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.goals.delete(id).await.map_err(map_prod_err)?;
        Ok(())
    }

    pub async fn productivity_goal_toggle(&self, id: i64, enabled: bool) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos
            .goals
            .set_enabled(id, enabled)
            .await
            .map_err(map_prod_err)?;
        Ok(())
    }

    pub async fn productivity_time_entry_create(
        &self,
        description: String,
        duration_mins: i64,
        category_id: Option<String>,
        project_id: Option<String>,
    ) -> Result<TimeEntryResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let now = Utc::now();
        let started_at = now - chrono::Duration::minutes(duration_mins);
        let duration_secs = duration_mins * 60;
        let entry = feature_productivity::types::TimeEntry {
            id: None,
            description,
            category_id,
            project_id,
            started_at,
            duration_secs,
            source: "manual".to_string(),
            created_at: now,
        };
        let id = repos
            .time_entries
            .insert(&entry)
            .await
            .map_err(map_prod_err)?;
        Ok(TimeEntryResponse {
            id,
            description: entry.description,
            category_id: entry.category_id,
            project_id: entry.project_id,
            started_at,
            duration_secs,
            source: entry.source,
        })
    }

    pub async fn productivity_time_entry_delete(&self, id: i64) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.time_entries.delete(id).await.map_err(map_prod_err)?;
        Ok(())
    }

    pub async fn productivity_category_upsert(
        &self,
        id: String,
        name: String,
        category_type: String,
        color: Option<String>,
        icon: Option<String>,
        rules: Option<CategoryRulesResponse>,
    ) -> Result<ActivityCategoryResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let ct: feature_productivity::types::CategoryType =
            category_type.parse().map_err(|_| {
                ApiError::new(
                    "VALIDATION",
                    "Invalid category_type. Use: productive, neutral, distracting",
                )
            })?;
        let cat_rules = rules.map(rules_from_response);
        let cat = feature_productivity::types::ActivityCategory {
            id,
            name,
            category_type: ct,
            color,
            icon,
            rules: cat_rules,
            is_system: false,
        };
        repos.categories.upsert(&cat).await.map_err(map_prod_err)?;
        // Refresh the in-memory categorizer so the background tracker uses updated rules
        self.refresh_categorizer(&repos.categories).await;
        Ok(ActivityCategoryResponse {
            id: cat.id,
            name: cat.name,
            category_type: cat.category_type.to_string(),
            color: cat.color,
            icon: cat.icon,
            is_system: false,
            rules: cat.rules.map(rules_to_response),
        })
    }

    pub async fn productivity_category_delete(&self, id: String) -> Result<bool, ApiError> {
        let repos = self.productivity_repos()?;
        let result = repos.categories.delete(&id).await.map_err(map_prod_err)?;
        // Refresh the in-memory categorizer so deleted category stops being applied
        self.refresh_categorizer(&repos.categories).await;
        Ok(result)
    }

    /// Re-assign all historical events for a given app/site to a new category.
    pub async fn productivity_recategorize_app(
        &self,
        app_name: String,
        site_name: Option<String>,
        new_category_id: String,
    ) -> Result<u64, ApiError> {
        let repos = self.productivity_repos()?;
        let rows = repos
            .events
            .recategorize_app(&app_name, site_name.as_deref(), &new_category_id)
            .await
            .map_err(map_prod_err)?;
        Ok(rows)
    }

    // ── V2: Insights & Auto-Focus ─────────────────────────────────────

    pub async fn productivity_insights(
        &self,
        date: Option<String>,
    ) -> Result<Vec<InsightCardResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let date = date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
        let engine = feature_productivity::insights::InsightEngine::new(repos.clone());
        // Generate any missing insights (idempotent)
        let _ = engine
            .generate_for_date(&date)
            .await
            .map_err(map_prod_err)?;
        // Always return ALL stored (non-dismissed) insights for the date
        let cards = repos
            .insights
            .list_for_date(&date)
            .await
            .map_err(map_prod_err)?;
        Ok(cards.into_iter().map(insight_to_response).collect())
    }

    pub async fn productivity_insight_dismiss(&self, id: String) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.insights.dismiss(&id).await.map_err(map_prod_err)?;
        Ok(())
    }

    // ── V3: Project Tracking ──────────────────────────────────────────

    pub async fn productivity_projects_list(
        &self,
    ) -> Result<Vec<ProductivityProjectResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let projects = repos.projects.list_all().await.map_err(map_prod_err)?;
        Ok(projects.into_iter().map(project_to_response).collect())
    }

    pub async fn productivity_project_upsert(
        &self,
        id: String,
        display_name: String,
        path: String,
        url_patterns: Option<Vec<String>>,
        color: Option<String>,
    ) -> Result<ProductivityProjectResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let project = feature_productivity::types::ProductivityProject {
            id: id.clone(),
            display_name,
            path,
            url_patterns: url_patterns.unwrap_or_default(),
            color,
            is_auto_detected: false,
            created_at: Utc::now(),
        };
        repos
            .projects
            .upsert(&project)
            .await
            .map_err(map_prod_err)?;
        Ok(project_to_response(project))
    }

    pub async fn productivity_project_delete(&self, id: String) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.projects.delete(&id).await.map_err(map_prod_err)?;
        Ok(())
    }

    /// Refresh the in-memory categorizer from DB so the background tracker
    /// picks up category changes immediately.
    async fn refresh_categorizer(&self, repo: &feature_productivity::repos::ActivityCategoryRepo) {
        if let Some(ref engine) = self.productivity_engine {
            let engine = engine.lock().await;
            let mut categorizer = engine.categorizer().write().await;
            if let Err(e) = categorizer.refresh(repo).await {
                tracing::warn!("Failed to refresh categorizer: {e}");
            }
        }
    }
}
