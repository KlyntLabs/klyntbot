//! Productivity context source — active focus session and today's summary.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::DailyAggregator;
use tokio::sync::Mutex;

const PRODUCTIVITY_CACHE_TTL_SECS: i64 = 60;

struct CachedValue {
    content: String,
    expires_at: DateTime<Utc>,
}

pub struct ProductivityContextSource {
    repos: ProductivityRepos,
    aggregator: DailyAggregator,
    cache: Mutex<Option<CachedValue>>,
}

impl ProductivityContextSource {
    pub fn new(repos: ProductivityRepos) -> Self {
        let aggregator = DailyAggregator::new(repos.clone());
        Self {
            repos,
            aggregator,
            cache: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ContextSource for ProductivityContextSource {
    fn name(&self) -> &str {
        "productivity"
    }

    fn priority(&self) -> u8 {
        55
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let mut cache = self.cache.lock().await;

        // Check TTL cache
        if let Some(ref cached) = *cache {
            if Utc::now() < cached.expires_at {
                return if cached.content.is_empty() {
                    None
                } else {
                    Some(cached.content.clone())
                };
            }
        }

        // Cache miss — build context
        let content = self.build_context().await;
        let result = if content.is_empty() {
            None
        } else {
            Some(format!("# Productivity Context\n\n{}", content))
        };

        *cache = Some(CachedValue {
            content: result.clone().unwrap_or_default(),
            expires_at: Utc::now() + Duration::seconds(PRODUCTIVITY_CACHE_TTL_SECS),
        });

        result
    }
}

impl ProductivityContextSource {
    async fn build_context(&self) -> String {
        let mut sections = Vec::new();

        // 1. Active focus session
        if let Ok(Some(session)) = self.repos.sessions.get_active().await {
            let elapsed = (Utc::now() - session.started_at).num_minutes();
            let target = session.target_mins.unwrap_or(45);
            let remaining = (target - elapsed).max(0);
            let mut focus_line = format!(
                "## Current Focus\nFocusing for {}min ({}min remaining).",
                elapsed, remaining
            );
            if session.interruptions > 0 {
                focus_line.push_str(&format!(" {} interruptions.", session.interruptions));
            }
            sections.push(focus_line);
        }

        // 2. Today's summary (use cached if available to avoid expensive recomputation)
        let today = Utc::now().format("%Y-%m-%d").to_string();
        if let Ok(summary) = self.aggregator.get_or_compute(&today).await {
            let active_hours = summary.total_active_secs as f64 / 3600.0;
            let productive_hours = summary.productive_secs as f64 / 3600.0;
            let distracting_hours = summary.distracting_secs as f64 / 3600.0;

            let mut today_line = format!(
                "## Today\n{:.1}h active ({:.1}h productive, {:.1}h distracting).",
                active_hours, productive_hours, distracting_hours
            );

            if summary.focus_sessions_count > 0 {
                today_line.push_str(&format!(
                    " {} focus session(s).",
                    summary.focus_sessions_count
                ));
            }

            sections.push(today_line);
        }

        sections.join("\n\n")
    }
}
