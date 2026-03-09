//! Productivity context source — intelligence layer data for agent context.
//!
//! Tier 1 (60s TTL): active session, quality score, current energy
//! Tier 2 (600s TTL): 14-day patterns, peak windows, tracking rules count
//! Tier 3 (event-driven): latest narrative

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::{DailyAggregator, ProductivityPatternAnalyzer, ProductivityPatterns};
use tokio::sync::Mutex;
use tracing::debug;

const PRODUCTIVITY_CACHE_TTL_SECS: i64 = 60;
const PATTERN_CACHE_TTL_SECS: i64 = 600;

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
        // Check cache without holding the lock across the async build
        {
            let cache = self.cache.lock().await;
            if let Some(ref cached) = *cache {
                if Utc::now() < cached.expires_at {
                    return if cached.content.is_empty() {
                        None
                    } else {
                        Some(cached.content.clone())
                    };
                }
            }
        }

        let content = self.build_context().await;
        let result = if content.is_empty() {
            None
        } else {
            Some(format!("# Productivity Context\n\n{}", content))
        };

        *self.cache.lock().await = Some(CachedValue {
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
        }

        match self.pattern_analyzer.analyze(14).await {
            Ok(patterns) => {
                let expires = Utc::now() + Duration::seconds(PATTERN_CACHE_TTL_SECS);
                *self.pattern_cache.lock().await = Some((patterns.clone(), expires));
                Some(patterns)
            }
            Err(e) => {
                debug!("pattern analysis failed: {e}");
                None
            }
        }
    }

    async fn build_context(&self) -> String {
        let mut sections = Vec::new();

        // Tier 1: Active intelligence session + quality + energy
        if let Ok(Some(session)) = self.repos.intelligence_sessions.get_active().await {
            let mut line = format!(
                "## Current Session\nType: {} | Category: {}",
                session.session_type,
                session.dominant_category.as_deref().unwrap_or("unknown")
            );
            if let Some(purity) = session.category_purity {
                line.push_str(&format!(" | Purity: {:.0}%", purity * 100.0));
            }
            sections.push(line);
        }

        // Today's quality score from intelligence layer
        let today = Utc::now().format("%Y-%m-%d").to_string();
        if let Ok(Some(score)) = self.repos.quality_scores.get_daily(&today).await {
            sections.push(format!(
                "## Quality\nToday's score: {:.0}/100 (focus={:.0}% continuity={:.0}% distraction_inv={:.0}%)",
                score.overall_score,
                score.focus_depth * 100.0,
                score.continuity * 100.0,
                score.distraction_inv * 100.0,
            ));
        }

        // Today's session summary
        if let Ok(summary) = self.repos.intelligence_sessions.today_summary().await {
            if summary.total_sessions > 0 {
                let hours = summary.total_duration as f64 / 3600.0;
                let quality_str = summary
                    .avg_quality
                    .map(|q| format!(" Avg quality: {:.0}.", q))
                    .unwrap_or_default();
                sections.push(format!(
                    "## Today\n{} sessions, {:.1}h tracked.{}",
                    summary.total_sessions, hours, quality_str
                ));
            }
        }

        // Energy forecasts for today
        if let Ok(forecasts) = self.repos.forecasts.list_for_date(&today).await {
            if !forecasts.is_empty() {
                let energy_lines: Vec<String> = forecasts
                    .iter()
                    .filter(|f| f.forecast_type == "energy")
                    .take(3)
                    .map(|f| {
                        format!(
                            "- {}-{}: energy {:.0}% (confidence {:.0}%)",
                            f.window_start.as_deref().unwrap_or("?"),
                            f.window_end.as_deref().unwrap_or("?"),
                            f.predicted_value * 100.0,
                            f.confidence * 100.0,
                        )
                    })
                    .collect();
                if !energy_lines.is_empty() {
                    sections.push(format!("## Energy Forecast\n{}", energy_lines.join("\n")));
                }
            }
        }

        // Tier 1 fallback: old-style active focus session
        if sections.is_empty() {
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
        }

        // Tier 1 fallback: old daily summary
        if sections.len() <= 1 {
            if let Ok(summary) = self.aggregator.compute_today().await {
                let active_hours = summary.total_active_secs as f64 / 3600.0;
                let productive_hours = summary.productive_secs as f64 / 3600.0;
                let distracting_hours = summary.distracting_secs as f64 / 3600.0;

                let score_str = summary
                    .productivity_score
                    .map(|s| format!(" Score: {:.0}/100.", s))
                    .unwrap_or_default();

                sections.push(format!(
                    "## Today (Legacy)\n{:.1}h active ({:.1}h productive, {:.1}h distracting).{}",
                    active_hours, productive_hours, distracting_hours, score_str
                ));
            }
        }

        // Tier 2: Weekly patterns
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

        // Tier 3: Latest narrative
        let yesterday = (Utc::now() - Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        for date in [&today, &yesterday] {
            if let Ok(Some(narrative)) = self.repos.narratives.get_by_date(date).await {
                let excerpt: String = narrative.narrative_text.chars().take(200).collect();
                sections.push(format!("## Narrative ({})\n{}", date, excerpt));
                break;
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
