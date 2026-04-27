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
    async fn upcoming_events(&self, lookback_days: u32, lookahead_days: u32) -> Vec<LauncherCalendarEvent> {
        let now = jiff::Timestamp::now();
        let from = now.saturating_sub(jiff::Span::new().days(lookback_days as i64));
        let to = now.saturating_add(jiff::Span::new().days(lookahead_days as i64));

        let from_str = from.strftime("%Y-%m-%dT%H:%M:%SZ").to_string();
        let to_str = to.strftime("%Y-%m-%dT%H:%M:%SZ").to_string();

        match self.repos.calendar_events.list_range(&from_str, &to_str).await {
            Ok(events) => events.into_iter().filter_map(|e| {
                let starts_at = jiff::Timestamp::from_str(&e.started_at).ok()?;
                let ends_at = jiff::Timestamp::from_str(&e.ended_at).ok()?;
                Some(LauncherCalendarEvent {
                    event_id: e.id,
                    title: e.title,
                    starts_at,
                    ends_at,
                })
            }).collect(),
            Err(_) => vec![],
        }
    }
}
