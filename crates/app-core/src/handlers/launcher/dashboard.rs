use chrono::Utc;
use desktop_shared::errors::ApiError;
use feature_launcher::{
    CalendarDashboard, DashboardData, FocusDashboard, ProductivityDashboard, TaskDashboard,
};
use feature_productivity::repos::ProductivityRepos;
use storage::Repos;

use crate::errors::{map_prod_err, map_storage_err};

/// Build dashboard data from current focus, calendar, tasks, and productivity.
pub async fn build_dashboard_data(
    repos: &Repos,
    prod_repos: Option<&ProductivityRepos>,
) -> Result<DashboardData, ApiError> {
    let now = Utc::now();
    let today = now.format("%Y-%m-%d").to_string();

    // Focus session
    let focus = match prod_repos {
        Some(pr) => {
            let session = pr.sessions.get_active().await.map_err(map_prod_err)?;
            match session {
                Some(s) => {
                    let elapsed = (now - s.started_at).num_seconds();
                    let target_secs = s.target_mins.map(|m| m * 60);
                    let task_name = if let Some(ref action_id) = s.action_id {
                        repos
                            .tasks
                            .get(action_id)
                            .await
                            .ok()
                            .flatten()
                            .map(|t| t.title)
                    } else {
                        None
                    };
                    Some(FocusDashboard {
                        task_name,
                        elapsed_secs: elapsed,
                        target_secs,
                        session_id: s.id.clone(),
                    })
                }
                None => None,
            }
        }
        None => None,
    };

    // Calendar events for today
    let calendar = match prod_repos {
        Some(pr) => {
            let events = pr
                .calendar_events
                .list_for_date(&today)
                .await
                .map_err(map_prod_err)?;
            events
                .into_iter()
                .filter_map(|e| {
                    let starts_at = chrono::DateTime::parse_from_rfc3339(&e.started_at)
                        .ok()?
                        .with_timezone(&Utc);
                    let ends_at = chrono::DateTime::parse_from_rfc3339(&e.ended_at)
                        .ok()?
                        .with_timezone(&Utc);
                    let minutes_until = (starts_at - now).num_minutes();
                    Some(CalendarDashboard {
                        event_id: e.id,
                        title: e.title,
                        starts_at,
                        ends_at,
                        minutes_until,
                    })
                })
                .collect()
        }
        None => vec![],
    };

    // Today's tasks (doing + due today)
    let start_of_today = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let start_of_tomorrow = start_of_today + chrono::Duration::days(1);
    let doing_filter = storage::repos::TaskFilter {
        status: Some("doing".to_string()),
        ..Default::default()
    };
    let due_today_filter = storage::repos::TaskFilter {
        due_after: Some(start_of_today),
        due_before: Some(start_of_tomorrow),
        ..Default::default()
    };
    let (doing, due_today) = tokio::try_join!(
        repos.tasks.list(&doing_filter),
        repos.tasks.list(&due_today_filter),
    )
    .map_err(map_storage_err)?;

    let mut seen = std::collections::HashSet::new();
    let tasks: Vec<TaskDashboard> = doing
        .into_iter()
        .chain(due_today)
        .filter(|t| seen.insert(t.id.clone()))
        .take(5)
        .map(|t| TaskDashboard {
            id: t.id.clone(),
            title: t.title.clone(),
            status: t.status.clone(),
            project_name: t.project_id.clone(),
            due_date: t.due_date.map(|d| d.to_rfc3339()),
        })
        .collect();

    // Productivity summary
    let productivity = match prod_repos {
        Some(pr) => {
            let summary = pr.summaries.get(&today).await.map_err(map_prod_err)?;
            match summary {
                Some(s) => {
                    let total_minutes = s.total_active_secs / 60;
                    let (top_cat, top_pct) = s
                        .top_categories
                        .first()
                        .map(|c| {
                            let pct = if s.total_active_secs > 0 {
                                (c.duration_secs as f64 / s.total_active_secs as f64) * 100.0
                            } else {
                                0.0
                            };
                            (c.category.clone(), pct)
                        })
                        .unwrap_or_else(|| ("None".to_string(), 0.0));
                    let score = if s.total_active_secs > 0 {
                        ((s.productive_secs as f64 / s.total_active_secs as f64) * 100.0) as i64
                    } else {
                        0
                    };
                    ProductivityDashboard {
                        total_minutes,
                        top_category: top_cat,
                        top_category_pct: top_pct,
                        score,
                    }
                }
                None => ProductivityDashboard {
                    total_minutes: 0,
                    top_category: "None".to_string(),
                    top_category_pct: 0.0,
                    score: 0,
                },
            }
        }
        None => ProductivityDashboard {
            total_minutes: 0,
            top_category: "None".to_string(),
            top_category_pct: 0.0,
            score: 0,
        },
    };

    Ok(DashboardData {
        focus,
        calendar,
        tasks,
        productivity,
    })
}
