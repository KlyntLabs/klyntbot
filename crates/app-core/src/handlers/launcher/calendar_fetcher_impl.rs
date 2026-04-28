use feature_launcher::{CalendarEvent as LauncherCalendarEvent, CalendarFetcher};
use feature_productivity::repos::ProductivityRepos;
use std::sync::Arc;

pub struct AppCalendarFetcher {
    repos: Arc<ProductivityRepos>,
}

impl AppCalendarFetcher {
    pub fn new(repos: Arc<ProductivityRepos>) -> Self {
        Self { repos }
    }
}

#[async_trait::async_trait]
impl CalendarFetcher for AppCalendarFetcher {
    async fn upcoming_events(
        &self,
        lookback_days: u32,
        lookahead_days: u32,
    ) -> Vec<LauncherCalendarEvent> {
        let now = jiff::Timestamp::now();
        let day_secs = 86_400i64;
        let from = now
            .saturating_sub(jiff::SignedDuration::from_secs(
                lookback_days as i64 * day_secs,
            ))
            .unwrap_or(now);
        let to = now
            .saturating_add(jiff::SignedDuration::from_secs(
                lookahead_days as i64 * day_secs,
            ))
            .unwrap_or(now);

        let from_str = from.strftime("%Y-%m-%dT%H:%M:%SZ").to_string();
        let to_str = to.strftime("%Y-%m-%dT%H:%M:%SZ").to_string();

        match self
            .repos
            .calendar_events
            .list_range(&from_str, &to_str)
            .await
        {
            Ok(events) => events
                .into_iter()
                .filter_map(|e| {
                    let starts_at = e.started_at.parse::<jiff::Timestamp>().ok()?;
                    let ends_at = e.ended_at.parse::<jiff::Timestamp>().ok()?;
                    Some(LauncherCalendarEvent {
                        event_id: e.id,
                        title: e.title,
                        starts_at,
                        ends_at,
                    })
                })
                .collect(),
            Err(_) => vec![],
        }
    }
}
