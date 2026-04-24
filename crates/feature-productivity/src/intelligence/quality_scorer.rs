use std::sync::Arc;

use tracing::info;

use bus::{DomainEvent, DomainEventBus};

use crate::repos::{IntelligenceSessionRepo, QualityScoreRepo};
use crate::types::*;

pub struct QualityScorer {
    session_repo: IntelligenceSessionRepo,
    score_repo: QualityScoreRepo,
    event_bus: Arc<DomainEventBus>,
    weights: ScoreWeights,
}

/// Baseline distraction count per session (used to normalize distraction_inv).
const DISTRACTION_BASELINE: f64 = 20.0;
/// Baseline context switches per session (used to normalize continuity).
const CONTEXT_SWITCH_BASELINE: f64 = 30.0;

impl QualityScorer {
    pub fn new(
        session_repo: IntelligenceSessionRepo,
        score_repo: QualityScoreRepo,
        event_bus: Arc<DomainEventBus>,
    ) -> Self {
        Self {
            session_repo,
            score_repo,
            event_bus,
            weights: ScoreWeights::default(),
        }
    }

    pub fn weights(&self) -> &ScoreWeights {
        &self.weights
    }

    pub fn set_weights(&mut self, weights: ScoreWeights) {
        self.weights = weights;
    }

    /// Score a single session. Computes 8 components and a weighted sum.
    pub async fn score_session(&self, session_id: &str) -> common::Result<Option<QualityScore>> {
        let session = match self.session_repo.get(session_id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let duration = session.duration_secs.unwrap_or(0) as f64;
        if duration <= 0.0 {
            return Ok(None);
        }

        let purity = session.category_purity.unwrap_or(0.5);
        let distraction_count = session.distraction_count as f64;
        let context_switches = session.context_switches as f64;
        let okr_alignment = session.okr_alignment.unwrap_or(0.5);

        // Component calculations (each 0.0–1.0)
        let focus_depth = purity;
        let distraction_inv = (1.0 - distraction_count / DISTRACTION_BASELINE).max(0.0);
        let continuity = (1.0 - context_switches / CONTEXT_SWITCH_BASELINE).max(0.0);
        // Use okr_alignment as a proxy for task completion until explicit tracking exists
        let task_completion = session.okr_alignment.unwrap_or(0.5);
        // deep_work_ratio: linear scale to 90 min (5400s) threshold
        let deep_work_ratio = (duration / 5400.0).min(1.0);
        // avg_session_length: normalized to target 45 min = 1.0
        let duration_mins = duration / 60.0;
        let avg_session_length = (duration_mins / 45.0).min(1.0);
        // meeting_focus_ratio: default 1.0 for individual sessions (meetings scored at daily level)
        let meeting_focus_ratio = 1.0;

        let w = &self.weights;
        let overall = (focus_depth * w.focus_depth
            + okr_alignment * w.okr_alignment
            + distraction_inv * w.distraction_inv
            + task_completion * w.task_completion
            + continuity * w.continuity
            + deep_work_ratio * w.deep_work_ratio
            + avg_session_length * w.avg_session_length
            + meeting_focus_ratio * w.meeting_focus_ratio)
            .clamp(0.0, 1.0);

        // Scale to 0–100
        let overall_100 = (overall * 100.0).round();

        let score_date = session
            .started_at
            .split('T')
            .next()
            .unwrap_or("")
            .to_string();
        let now = jiff::Timestamp::now().to_string();
        let weights_json = serde_json::to_string(&self.weights).ok();

        let score = QualityScore {
            id: uuid::Uuid::new_v4().to_string(),
            score_date: score_date.clone(),
            session_id: Some(session_id.to_string()),
            overall_score: overall_100,
            focus_depth,
            okr_alignment,
            distraction_inv,
            task_completion,
            continuity,
            deep_work_ratio,
            avg_session_length,
            meeting_focus_ratio,
            weights_json,
            explanation: Some(format!(
                "focus={:.0}% distraction_inv={:.0}% continuity={:.0}% deep_work={:.0}% session_len={:.0}%",
                focus_depth * 100.0,
                distraction_inv * 100.0,
                continuity * 100.0,
                deep_work_ratio * 100.0,
                avg_session_length * 100.0,
            )),
            created_at: now,
        };

        self.score_repo.upsert(&score).await?;

        info!(
            session_id = %session_id,
            overall = %overall_100,
            "session quality scored"
        );

        self.event_bus.publish(DomainEvent::QualityScored {
            score_date,
            session_id: Some(session_id.to_string()),
            overall_score: overall_100,
            components: format_components(&score),
        });

        Ok(Some(score))
    }

    /// Compute a daily aggregate score from all session scores for a given date.
    pub async fn score_day(&self, date: &str) -> common::Result<Option<QualityScore>> {
        let next_date = next_day(date);
        let scores = self.score_repo.list_range(date, &next_date).await?;
        let session_scores: Vec<&QualityScore> =
            scores.iter().filter(|s| s.session_id.is_some()).collect();

        if session_scores.is_empty() {
            return Ok(None);
        }

        let n = session_scores.len() as f64;
        let avg = |f: fn(&QualityScore) -> f64| -> f64 {
            session_scores.iter().map(|s| f(s)).sum::<f64>() / n
        };

        let overall = avg(|s| s.overall_score);
        let focus_depth = avg(|s| s.focus_depth);
        let okr_alignment = avg(|s| s.okr_alignment);
        let distraction_inv = avg(|s| s.distraction_inv);
        let task_completion = avg(|s| s.task_completion);
        let continuity = avg(|s| s.continuity);
        let avg_session_length = avg(|s| s.avg_session_length);
        let meeting_focus_ratio = avg(|s| s.meeting_focus_ratio);

        // deep_work_ratio at daily level: fraction of sessions >= 90 min
        let deep_work_ratio = {
            let deep_count = session_scores
                .iter()
                .filter(|s| s.deep_work_ratio >= 1.0)
                .count() as f64;
            deep_count / n
        };

        let now = jiff::Timestamp::now().to_string();
        let daily = QualityScore {
            id: uuid::Uuid::new_v4().to_string(),
            score_date: date.to_string(),
            session_id: None,
            overall_score: overall.round(),
            focus_depth,
            okr_alignment,
            distraction_inv,
            task_completion,
            continuity,
            deep_work_ratio,
            avg_session_length,
            meeting_focus_ratio,
            weights_json: serde_json::to_string(&self.weights).ok(),
            explanation: Some(format!("Averaged from {} sessions", session_scores.len())),
            created_at: now,
        };

        self.score_repo.upsert(&daily).await?;

        self.event_bus.publish(DomainEvent::QualityScored {
            score_date: date.to_string(),
            session_id: None,
            overall_score: daily.overall_score,
            components: format_components(&daily),
        });

        Ok(Some(daily))
    }
}

fn format_components(score: &QualityScore) -> String {
    format!(
        "fd={:.2} okr={:.2} di={:.2} tc={:.2} co={:.2} dw={:.2} asl={:.2} mfr={:.2}",
        score.focus_depth,
        score.okr_alignment,
        score.distraction_inv,
        score.task_completion,
        score.continuity,
        score.deep_work_ratio,
        score.avg_session_length,
        score.meeting_focus_ratio,
    )
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
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    fn make_bus() -> Arc<DomainEventBus> {
        Arc::new(DomainEventBus::new(64))
    }

    fn sample_session(
        id: &str,
        purity: f64,
        distractions: i64,
        switches: i64,
    ) -> ProductivitySession {
        ProductivitySession {
            id: id.to_string(),
            session_type: "focus".to_string(),
            started_at: "2026-03-09T10:00:00Z".to_string(),
            ended_at: Some("2026-03-09T11:00:00Z".to_string()),
            duration_secs: Some(3600),
            dominant_category: Some("coding".to_string()),
            category_purity: Some(purity),
            quality_score: None,
            source: "auto".to_string(),
            app_breakdown: None,
            context_switches: switches,
            distraction_count: distractions,
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
    async fn test_score_session_computes_components() {
        let pool = setup_pool().await;
        let bus = make_bus();
        let session_repo = IntelligenceSessionRepo::new(pool.clone());
        let score_repo = QualityScoreRepo::new(pool.clone());

        let session = sample_session("qs-test-1", 0.9, 5, 10);
        session_repo.create(&session).await.unwrap();

        let scorer =
            QualityScorer::new(IntelligenceSessionRepo::new(pool.clone()), score_repo, bus);

        let score = scorer.score_session("qs-test-1").await.unwrap();
        assert!(score.is_some());
        let score = score.unwrap();

        // focus_depth = purity = 0.9
        assert!((score.focus_depth - 0.9).abs() < 0.01);
        // distraction_inv = 1.0 - 5/20 = 0.75
        assert!((score.distraction_inv - 0.75).abs() < 0.01);
        // continuity = 1.0 - 10/30 ≈ 0.667
        assert!((score.continuity - 0.667).abs() < 0.01);
        // okr_alignment = 0.8
        assert!((score.okr_alignment - 0.8).abs() < 0.01);
        // overall > 0 (it's a weighted combination)
        assert!(score.overall_score > 0.0);
        assert!(score.overall_score <= 100.0);
    }

    #[tokio::test]
    async fn test_score_day_aggregates_sessions() {
        let pool = setup_pool().await;
        let bus = make_bus();
        let session_repo = IntelligenceSessionRepo::new(pool.clone());
        let _score_repo = QualityScoreRepo::new(pool.clone());

        // Create two sessions
        let s1 = sample_session("qs-day-1", 0.9, 5, 10);
        let mut s2 = sample_session("qs-day-2", 0.7, 15, 20);
        s2.started_at = "2026-03-09T14:00:00Z".to_string();
        s2.ended_at = Some("2026-03-09T15:00:00Z".to_string());
        session_repo.create(&s1).await.unwrap();
        session_repo.create(&s2).await.unwrap();

        let scorer = QualityScorer::new(
            IntelligenceSessionRepo::new(pool.clone()),
            QualityScoreRepo::new(pool.clone()),
            bus,
        );

        // Score both sessions first
        scorer.score_session("qs-day-1").await.unwrap();
        scorer.score_session("qs-day-2").await.unwrap();

        // Score the day
        let daily = scorer.score_day("2026-03-09").await.unwrap();
        assert!(daily.is_some());
        let daily = daily.unwrap();
        assert!(daily.session_id.is_none()); // daily score has no session_id
        assert!(daily.overall_score > 0.0);
    }

    #[tokio::test]
    async fn test_weights_default() {
        let w = ScoreWeights::default();
        let sum = w.focus_depth
            + w.okr_alignment
            + w.distraction_inv
            + w.task_completion
            + w.continuity
            + w.deep_work_ratio
            + w.avg_session_length
            + w.meeting_focus_ratio;
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_update_weights() {
        let pool = setup_pool().await;
        let bus = make_bus();
        let session_repo = IntelligenceSessionRepo::new(pool.clone());
        let score_repo = QualityScoreRepo::new(pool.clone());

        let session = sample_session("qs-wt-1", 0.9, 5, 10);
        session_repo.create(&session).await.unwrap();

        let mut scorer =
            QualityScorer::new(IntelligenceSessionRepo::new(pool.clone()), score_repo, bus);

        // Score with default weights
        let score1 = scorer.score_session("qs-wt-1").await.unwrap().unwrap();

        // Change weights: all weight on focus_depth
        scorer.set_weights(ScoreWeights {
            focus_depth: 1.0,
            okr_alignment: 0.0,
            distraction_inv: 0.0,
            task_completion: 0.0,
            continuity: 0.0,
            deep_work_ratio: 0.0,
            avg_session_length: 0.0,
            meeting_focus_ratio: 0.0,
        });

        // Need a new session since we already scored this one
        let mut s2 = sample_session("qs-wt-2", 0.9, 5, 10);
        s2.id = "qs-wt-2".to_string();
        session_repo.create(&s2).await.unwrap();
        let score2 = scorer.score_session("qs-wt-2").await.unwrap().unwrap();

        // With 100% weight on focus_depth (0.9), overall should be 90
        assert!((score2.overall_score - 90.0).abs() < 1.0);
        // Scores should differ since weights changed
        assert!((score1.overall_score - score2.overall_score).abs() > 0.1);
    }
}
