use desktop_shared::errors::ApiError;
use feature_launcher::{CalendarDashboard, DashboardData, ProductivityDashboard, TaskDashboard};
use feature_productivity::repos::ProductivityRepos;
use storage::Repos;

use crate::errors::{map_prod_err, map_storage_err};

/// Build dashboard data from current focus, calendar, tasks, and productivity.
pub async fn build_dashboard_data(
    repos: &Repos,
    prod_repos: Option<&ProductivityRepos>,
) -> Result<DashboardData, ApiError> {
    let now = jiff::Timestamp::now();
    let today = now.strftime("%Y-%m-%d").to_string();

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
                    let starts_at = e.started_at.parse::<jiff::Timestamp>().ok()?;
                    let ends_at = e.ended_at.parse::<jiff::Timestamp>().ok()?;
                    let minutes_until =
                        (starts_at.as_millisecond() - now.as_millisecond()) / 60_000;
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
    let today_date = now.to_zoned(jiff::tz::TimeZone::UTC).date();
    let start_of_today: jiff::Timestamp = today_date
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .map_err(|_| ApiError::new("INTERNAL", "timezone error"))?
        .timestamp();
    let start_of_tomorrow = start_of_today
        .checked_add(jiff::SignedDuration::from_secs(86400))
        .unwrap_or(start_of_today);
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
            due_date: t.due_date.map(|d| d.to_string()),
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
        calendar,
        tasks,
        productivity,
    })
}
