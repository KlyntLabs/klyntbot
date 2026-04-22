use std::sync::Arc;

use tracing::info;

use bus::DomainEventBus;

use crate::handler::ProductivityHandler;
use crate::repos::{IntelligenceSessionRepo, NarrativeRepo, QualityScoreRepo};
use crate::types::*;

pub struct NarrativeGenerator {
    session_repo: IntelligenceSessionRepo,
    quality_repo: QualityScoreRepo,
    narrative_repo: NarrativeRepo,
    #[allow(dead_code)]
    event_bus: Arc<DomainEventBus>,
    handler: Option<Arc<dyn ProductivityHandler>>,
}

impl NarrativeGenerator {
    pub fn new(
        session_repo: IntelligenceSessionRepo,
        quality_repo: QualityScoreRepo,
        narrative_repo: NarrativeRepo,
        event_bus: Arc<DomainEventBus>,
        handler: Option<Arc<dyn ProductivityHandler>>,
    ) -> Self {
        Self {
            session_repo,
            quality_repo,
            narrative_repo,
            event_bus,
            handler,
        }
    }

    /// Get an existing narrative or generate a new one for the given date.
    pub async fn get_or_generate(&self, date: &str) -> common::Result<Narrative> {
        if let Some(existing) = self.narrative_repo.get_by_date(date).await? {
            return Ok(existing);
        }
        self.generate(date).await
    }

    /// Generate a narrative for the given date.
    pub async fn generate(&self, date: &str) -> common::Result<Narrative> {
        let next_date = next_day(date);
        let sessions = self.session_repo.list_range(date, &next_date).await?;

        let metrics = compute_metrics(&sessions);
        let quality = self.quality_repo.get_daily(date).await?;

        let context = build_context(date, &metrics, quality.as_ref());

        let narrative_text = if let Some(ref handler) = self.handler {
            match handler.generate_narrative(&context).await {
                Ok(text) => text,
                Err(_) => generate_template_narrative(date, &metrics, quality.as_ref()),
            }
        } else {
            generate_template_narrative(date, &metrics, quality.as_ref())
        };

        let sentiment = infer_sentiment(quality.as_ref());

        let narrative = Narrative {
            id: uuid::Uuid::new_v4().to_string(),
            narrative_date: date.to_string(),
            narrative_text: narrative_text.clone(),
            key_moments: None,
            sentiment: Some(sentiment.clone()),
            total_focus_mins: metrics.total_focus_mins,
            total_meeting_mins: metrics.total_meeting_mins,
            total_break_mins: metrics.total_break_mins,
            quality_score: quality.as_ref().map(|q| q.overall_score),
            top_categories: if metrics.top_categories.is_empty() {
                None
            } else {
                serde_json::to_string(&metrics.top_categories).ok()
            },
            created_at: jiff::Timestamp::now().to_string(),
        };

        self.narrative_repo.upsert(&narrative).await?;

        let _excerpt = narrative_text.chars().take(100).collect::<String>();
        info!(date = %date, sentiment = %sentiment, "narrative generated");

        // NarrativeGenerated variant deleted; no-op.

        Ok(narrative)
    }
}

fn compute_metrics(sessions: &[ProductivitySession]) -> NarrativeMetrics {
    let mut focus_secs: i64 = 0;
    let mut meeting_secs: i64 = 0;
    let mut break_secs: i64 = 0;
    let mut category_counts: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();

    for s in sessions {
        let dur = s.duration_secs.unwrap_or(0);
        match s.session_type.as_str() {
            "focus" => focus_secs += dur,
            "meeting" => meeting_secs += dur,
            "break" => break_secs += dur,
            _ => {}
        }
        if let Some(ref cat) = s.dominant_category {
            *category_counts.entry(cat.clone()).or_default() += dur;
        }
    }

    let mut cats: Vec<(String, i64)> = category_counts.into_iter().collect();
    cats.sort_by(|a, b| b.1.cmp(&a.1));
    let top_categories: Vec<String> = cats.into_iter().take(3).map(|(c, _)| c).collect();

    NarrativeMetrics {
        total_focus_mins: focus_secs / 60,
        total_meeting_mins: meeting_secs / 60,
        total_break_mins: break_secs / 60,
        quality_score: None,
        top_categories,
    }
}

fn build_context(date: &str, metrics: &NarrativeMetrics, quality: Option<&QualityScore>) -> String {
    let mut lines = vec![format!("Date: {date}")];
    lines.push(format!(
        "Focus: {}min, Meeting: {}min, Break: {}min",
        metrics.total_focus_mins, metrics.total_meeting_mins, metrics.total_break_mins
    ));
    if let Some(q) = quality {
        lines.push(format!(
            "Quality: {:.0}/100 (focus={:.0}% continuity={:.0}%)",
            q.overall_score,
            q.focus_depth * 100.0,
            q.continuity * 100.0
        ));
    }
    if !metrics.top_categories.is_empty() {
        lines.push(format!(
            "Top categories: {}",
            metrics.top_categories.join(", ")
        ));
    }
    lines.join("\n")
}

fn generate_template_narrative(
    date: &str,
    metrics: &NarrativeMetrics,
    quality: Option<&QualityScore>,
) -> String {
    let total_mins =
        metrics.total_focus_mins + metrics.total_meeting_mins + metrics.total_break_mins;
    let quality_desc = match quality {
        Some(q) if q.overall_score >= 80.0 => "an excellent",
        Some(q) if q.overall_score >= 60.0 => "a good",
        Some(q) if q.overall_score >= 40.0 => "a moderate",
        Some(_) => "a challenging",
        None => "a",
    };

    let mut parts = vec![format!(
        "On {date}, you had {quality_desc} day with {total_mins} minutes of tracked activity."
    )];

    if metrics.total_focus_mins > 0 {
        parts.push(format!(
            "You spent {} minutes in focused work.",
            metrics.total_focus_mins
        ));
    }
    if metrics.total_meeting_mins > 0 {
        parts.push(format!(
            "{} minutes were in meetings.",
            metrics.total_meeting_mins
        ));
    }
    if !metrics.top_categories.is_empty() {
        parts.push(format!(
            "Your main activities were: {}.",
            metrics.top_categories.join(", ")
        ));
    }

    parts.join(" ")
}

fn infer_sentiment(quality: Option<&QualityScore>) -> String {
    match quality {
        Some(q) if q.overall_score >= 70.0 => "positive".to_string(),
        Some(q) if q.overall_score >= 40.0 => "neutral".to_string(),
        Some(_) => "negative".to_string(),
        None => "neutral".to_string(),
    }
}

fn next_day(date: &str) -> String {
    date.parse::<jiff::civil::Date>()
        .ok()
        .and_then(|d| d.checked_add(jiff::Span::new().days(1)).ok())
        .map(|d| d.to_string())
        .unwrap_or_else(|| date.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::IntelligenceSessionRepo;
    use crate::ProductivityFeature;
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    fn make_bus() -> Arc<DomainEventBus> {
        Arc::new(DomainEventBus::new(64))
    }

    fn sample_session(id: &str, session_type: &str, duration: i64) -> ProductivitySession {
        ProductivitySession {
            id: id.to_string(),
            session_type: session_type.to_string(),
            started_at: "2026-03-09T10:00:00Z".to_string(),
            ended_at: Some("2026-03-09T11:00:00Z".to_string()),
            duration_secs: Some(duration),
            dominant_category: Some("coding".to_string()),
            category_purity: Some(0.9),
            quality_score: None,
            source: "auto".to_string(),
            app_breakdown: None,
            context_switches: 5,
            distraction_count: 2,
            predicted_energy: None,
            okr_alignment: Some(0.8),
            notes: None,
            tags: None,
            action_id: None,
            project_id: None,
            target_mins: None,
            actual_mins: None,
            interruptions: None,
            distraction_events: None,
            completed: None,
            created_at: "2026-03-09T10:00:00Z".to_string(),
            updated_at: "2026-03-09T11:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_generate_with_no_handler_returns_fallback() {
        let pool = setup_pool().await;
        let bus = make_bus();

        let session_repo = IntelligenceSessionRepo::new(pool.clone());
        let s1 = sample_session("narr-s1", "focus", 3600);
        session_repo.create(&s1).await.unwrap();

        let gen = NarrativeGenerator::new(
            IntelligenceSessionRepo::new(pool.clone()),
            QualityScoreRepo::new(pool.clone()),
            NarrativeRepo::new(pool.clone()),
            bus,
            None, // no handler
        );

        let narrative = gen.generate("2026-03-09").await.unwrap();
        assert!(!narrative.narrative_text.is_empty());
        assert!(narrative.narrative_text.contains("60 minutes"));
        assert_eq!(narrative.total_focus_mins, 60);
    }

    #[tokio::test]
    async fn test_get_or_generate_caches() {
        let pool = setup_pool().await;
        let bus = make_bus();

        let session_repo = IntelligenceSessionRepo::new(pool.clone());
        let s1 = sample_session("narr-cache-1", "focus", 1800);
        session_repo.create(&s1).await.unwrap();

        let gen = NarrativeGenerator::new(
            IntelligenceSessionRepo::new(pool.clone()),
            QualityScoreRepo::new(pool.clone()),
            NarrativeRepo::new(pool.clone()),
            bus,
            None,
        );

        let first = gen.get_or_generate("2026-03-09").await.unwrap();
        let second = gen.get_or_generate("2026-03-09").await.unwrap();

        // Same narrative returned (cached via DB)
        assert_eq!(first.id, second.id);
        assert_eq!(first.narrative_text, second.narrative_text);
    }
}
