use crate::search::{fuzzy_match, SearchSource};
use crate::types::{LauncherItem, LauncherItemKind};
use async_trait::async_trait;
use jiff::Timestamp;
use std::sync::Arc;

#[async_trait]
pub trait CalendarFetcher: Send + Sync {
    async fn upcoming_events(&self, lookback_days: u32, lookahead_days: u32) -> Vec<CalendarEvent>;
}

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub event_id: String,
    pub title: String,
    pub starts_at: Timestamp,
    pub ends_at: Timestamp,
}

pub struct CalendarSource {
    fetcher: Arc<dyn CalendarFetcher>,
    lookback_days: u32,
    lookahead_days: u32,
}

impl CalendarSource {
    pub fn new(fetcher: Arc<dyn CalendarFetcher>, lookback_days: u32, lookahead_days: u32) -> Self {
        Self {
            fetcher,
            lookback_days,
            lookahead_days,
        }
    }
}

#[async_trait]
impl SearchSource for CalendarSource {
    fn name(&self) -> &'static str {
        "calendar"
    }
    fn prefix(&self) -> Option<&'static str> {
        Some("c/")
    }
    fn cache_ttl(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(60))
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let events = self
            .fetcher
            .upcoming_events(self.lookback_days, self.lookahead_days)
            .await;
        if query.is_empty() {
            return events
                .into_iter()
                .take(limit)
                .map(|e| event_to_item(&e, 0.6))
                .collect();
        }
        let scored = fuzzy_match(query, &events, |e| e.title.as_str(), limit);
        scored
            .into_iter()
            .map(|(score, e)| {
                let normalized = (score as f64 / 1000.0) * 0.85;
                event_to_item(e, normalized)
            })
            .collect()
    }
}

fn event_to_item(e: &CalendarEvent, score: f64) -> LauncherItem {
    let tz = jiff::tz::TimeZone::system();
    let subtitle = format!(
        "{} → {}",
        e.starts_at.to_zoned(tz.clone()).strftime("%-I:%M %p"),
        e.ends_at.to_zoned(tz).strftime("%-I:%M %p"),
    );
    LauncherItem {
        id: format!("cal:{}", e.event_id),
        title: e.title.clone(),
        subtitle: Some(subtitle),
        icon: Some("📅".to_string()),
        kind: LauncherItemKind::Calendar {
            event_id: e.event_id.clone(),
            starts_at: e.starts_at,
        },
        score,
        no_view: false,
        arguments: vec![],
        pinned: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubFetcher(Vec<CalendarEvent>);
    #[async_trait]
    impl CalendarFetcher for StubFetcher {
        async fn upcoming_events(&self, _: u32, _: u32) -> Vec<CalendarEvent> {
            self.0.clone()
        }
    }

    #[tokio::test]
    async fn empty_query_returns_top_events() {
        let events = vec![CalendarEvent {
            event_id: "1".into(),
            title: "Standup".into(),
            starts_at: Timestamp::now(),
            ends_at: Timestamp::now(),
        }];
        let src = CalendarSource::new(Arc::new(StubFetcher(events)), 1, 7);
        let r = src.search("", 10).await;
        assert_eq!(r.len(), 1);
    }

    #[tokio::test]
    async fn fuzzy_match_orders_by_relevance() {
        let events = vec![
            CalendarEvent {
                event_id: "1".into(),
                title: "Sprint Planning".into(),
                starts_at: Timestamp::now(),
                ends_at: Timestamp::now(),
            },
            CalendarEvent {
                event_id: "2".into(),
                title: "1:1 with Manager".into(),
                starts_at: Timestamp::now(),
                ends_at: Timestamp::now(),
            },
        ];
        let src = CalendarSource::new(Arc::new(StubFetcher(events)), 1, 7);
        let r = src.search("planning", 10).await;
        assert!(r[0].title.contains("Planning"));
    }
}
