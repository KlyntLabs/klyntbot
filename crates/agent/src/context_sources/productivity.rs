//! Productivity context source — active focus session, today's summary, and weekly patterns.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::{DailyAggregator, ProductivityPatternAnalyzer, ProductivityPatterns};
use tokio::sync::Mutex;

const PRODUCTIVITY_CACHE_TTL_SECS: i64 = 60;
const PATTERN_CACHE_TTL_SECS: i64 = 600; // Patterns change slowly — 10 min cache

struct CachedValue {
    content: String,
    expires_at: DateTime<Utc>,
}

pub struct ProductivityContextSource {
    repos: ProductivityRepos,
    aggregator: DailyAggregator,
    pattern_analyzer: ProductivityPatternAnalyzer,
    cache: Mutex<Option<CachedValue>>,
    pattern_cache: Mutex<Option<(ProductivityPatterns, DateTime<Utc>)>>,
}

impl ProductivityContextSource {
    pub fn new(repos: ProductivityRepos) -> Self {
        let aggregator = DailyAggregator::new(repos.clone());
        let pattern_analyzer = ProductivityPatternAnalyzer::new(repos.clone());
        Self {
            repos,
            aggregator,
            pattern_analyzer,
            cache: Mutex::new(None),
            pattern_cache: Mutex::new(None),
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
    async fn get_patterns(&self) -> Option<ProductivityPatterns> {
        {
            let cache = self.pattern_cache.lock().await;
            if let Some((ref patterns, expires_at)) = *cache {
                if Utc::now() < expires_at {
                    return Some(patterns.clone());
                }
            }
        } // Lock released before async work

        match self.pattern_analyzer.analyze(14).await {
            Ok(patterns) => {
                let expires = Utc::now() + Duration::seconds(PATTERN_CACHE_TTL_SECS);
                *self.pattern_cache.lock().await = Some((patterns.clone(), expires));
                Some(patterns)
            }
            Err(_) => None,
        }
    }

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

        // 3. Weekly patterns (with separate longer TTL cache)
        if let Some(patterns) = self.get_patterns().await {
            if patterns.days_analyzed >= 3 {
                let mut pattern_lines = vec!["## Patterns (last 14 days)".to_string()];

                if !patterns.peak_focus_hours.is_empty() {
                    let hours: Vec<String> = patterns
                        .peak_focus_hours
                        .iter()
                        .map(|h| format_hour(*h))
                        .collect();
                    pattern_lines.push(format!("- Peak focus hours: {}", hours.join(", ")));
                }

                if patterns.avg_session_mins > 0.0 {
                    pattern_lines.push(format!(
                        "- Avg focus session: {:.0}min",
                        patterns.avg_session_mins
                    ));
                }

                if patterns.productive_ratio > 0.0 {
                    pattern_lines.push(format!(
                        "- Productive ratio: {:.0}%",
                        patterns.productive_ratio * 100.0
                    ));
                }

                if let Some(day) = patterns.best_day_of_week {
                    pattern_lines.push(format!("- Most productive day: {}", day));
                }

                sections.push(pattern_lines.join("\n"));
            }
        }

        sections.join("\n\n")
    }
}

fn format_hour(hour: u32) -> String {
    match hour {
        0 => "12am".to_string(),
        1..=11 => format!("{}am", hour),
        12 => "12pm".to_string(),
        _ => format!("{}pm", hour - 12),
    }
}
