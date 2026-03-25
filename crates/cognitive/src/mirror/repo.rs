//! Repository for Phase 1 Mirror tables.

use chrono::{DateTime, Utc};
use common::Result;
use uuid::Uuid;

use crate::mirror::{
    FeedbackTarget, NarrativeSnippet, RoutingSnapshot, TrendNarrative, UserFeedback,
};

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct RoutingSnapshotRow {
    id: String,
    captured_at: String,
    window_hours: i64,
    total_messages: i64,
    distribution_json: String,
    fallback_rate: f64,
    avg_routing_confidence: f64,
    low_confidence_count: i64,
    user_feedback: Option<String>,
}

impl TryFrom<RoutingSnapshotRow> for RoutingSnapshot {
    type Error = common::KlyntbotError;

    fn try_from(row: RoutingSnapshotRow) -> Result<Self> {
        let captured_at = DateTime::parse_from_rfc3339(&row.captured_at)
            .map_err(|e| common::KlyntbotError::Storage(format!("bad captured_at: {e}")))?
            .with_timezone(&Utc);
        let distribution = serde_json::from_str(&row.distribution_json)?;
        let user_feedback = row
            .user_feedback
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        Ok(RoutingSnapshot {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| common::KlyntbotError::Storage(format!("bad uuid: {e}")))?,
            captured_at,
            window_hours: row.window_hours as u8,
            total_messages: row.total_messages as u32,
            distribution,
            fallback_rate: row.fallback_rate,
            avg_routing_confidence: row.avg_routing_confidence,
            low_confidence_count: row.low_confidence_count as u32,
            user_feedback,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct TrendNarrativeRow {
    id: String,
    generated_at: String,
    period_start: String,
    period_end: String,
    routing_summary: String,
    improvement_highlights_json: String,
    experiment_summary: String,
    meta_rule_updates_json: String,
    full_narrative: String,
    user_feedback: Option<String>,
}

impl TryFrom<TrendNarrativeRow> for TrendNarrative {
    type Error = common::KlyntbotError;

    fn try_from(row: TrendNarrativeRow) -> Result<Self> {
        let parse_dt = |s: &str| {
            DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&Utc))
                .map_err(|e| common::KlyntbotError::Storage(format!("bad datetime: {e}")))
        };
        let improvement_highlights = serde_json::from_str(&row.improvement_highlights_json)?;
        let meta_rule_updates = serde_json::from_str(&row.meta_rule_updates_json)?;
        let user_feedback = row
            .user_feedback
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        Ok(TrendNarrative {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| common::KlyntbotError::Storage(format!("bad uuid: {e}")))?,
            generated_at: parse_dt(&row.generated_at)?,
            period_start: parse_dt(&row.period_start)?,
            period_end: parse_dt(&row.period_end)?,
            routing_summary: row.routing_summary,
            improvement_highlights,
            experiment_summary: row.experiment_summary,
            meta_rule_updates,
            full_narrative: row.full_narrative,
            user_feedback,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SnippetRow {
    id: String,
    created_at: String,
    alert_type: String,
    headline: String,
    body: String,
    action_json: Option<String>,
    user_feedback: Option<String>,
    dismissed_at: Option<String>,
}

impl TryFrom<SnippetRow> for NarrativeSnippet {
    type Error = common::KlyntbotError;

    fn try_from(row: SnippetRow) -> Result<Self> {
        let created_at = DateTime::parse_from_rfc3339(&row.created_at)
            .map_err(|e| common::KlyntbotError::Storage(format!("bad created_at: {e}")))?
            .with_timezone(&Utc);
        let alert_type = serde_json::from_str(&format!("\"{}\"", row.alert_type))?;
        let suggested_action = row
            .action_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        let user_feedback = row
            .user_feedback
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        let dismissed_at = row
            .dismissed_at
            .as_deref()
            .map(|s| {
                DateTime::parse_from_rfc3339(s)
                    .map(|d| d.with_timezone(&Utc))
                    .map_err(|e| common::KlyntbotError::Storage(format!("bad dismissed_at: {e}")))
            })
            .transpose()?;
        Ok(NarrativeSnippet {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| common::KlyntbotError::Storage(format!("bad uuid: {e}")))?,
            created_at,
            alert_type,
            headline: row.headline,
            body: row.body,
            suggested_action,
            user_feedback,
            dismissed_at,
        })
    }
}

// ---------------------------------------------------------------------------
// Repo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MirrorRepo {
    pool: storage::StoragePool,
}

impl MirrorRepo {
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    fn db(&self) -> &sqlx::SqlitePool {
        self.pool.inner()
    }

    // -----------------------------------------------------------------------
    // Routing snapshots
    // -----------------------------------------------------------------------

    pub async fn insert_routing_snapshot(&self, snap: &RoutingSnapshot) -> Result<()> {
        let distribution_json = serde_json::to_string(&snap.distribution)?;
        let user_feedback = snap
            .user_feedback
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        sqlx::query(
            r#"
            INSERT INTO mirror_routing_snapshots
                (id, captured_at, window_hours, total_messages, distribution_json,
                 fallback_rate, avg_routing_confidence, low_confidence_count, user_feedback)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(snap.id.to_string())
        .bind(snap.captured_at.to_rfc3339())
        .bind(snap.window_hours as i64)
        .bind(snap.total_messages as i64)
        .bind(distribution_json)
        .bind(snap.fallback_rate)
        .bind(snap.avg_routing_confidence)
        .bind(snap.low_confidence_count as i64)
        .bind(user_feedback)
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_latest_routing_snapshot(&self) -> Result<Option<RoutingSnapshot>> {
        let row = sqlx::query_as::<_, RoutingSnapshotRow>(
            "SELECT * FROM mirror_routing_snapshots ORDER BY captured_at DESC LIMIT 1",
        )
        .fetch_optional(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        row.map(RoutingSnapshot::try_from).transpose()
    }

    pub async fn get_routing_history(&self, days: u32) -> Result<Vec<RoutingSnapshot>> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let rows = sqlx::query_as::<_, RoutingSnapshotRow>(
            "SELECT * FROM mirror_routing_snapshots WHERE captured_at >= ?1 ORDER BY captured_at DESC",
        )
        .bind(cutoff.to_rfc3339())
        .fetch_all(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        rows.into_iter().map(RoutingSnapshot::try_from).collect()
    }

    // -----------------------------------------------------------------------
    // Narrative snippets
    // -----------------------------------------------------------------------

    pub async fn insert_snippet(&self, snippet: &NarrativeSnippet) -> Result<()> {
        let alert_type = serde_json::to_string(&snippet.alert_type)?;
        // strip surrounding quotes produced by serde_json for unit variants
        let alert_type = alert_type.trim_matches('"').to_string();
        let action_json = snippet
            .suggested_action
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let user_feedback = snippet
            .user_feedback
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        sqlx::query(
            r#"
            INSERT INTO mirror_snippets
                (id, created_at, alert_type, headline, body, action_json, user_feedback, dismissed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(snippet.id.to_string())
        .bind(snippet.created_at.to_rfc3339())
        .bind(alert_type)
        .bind(&snippet.headline)
        .bind(&snippet.body)
        .bind(action_json)
        .bind(user_feedback)
        .bind(snippet.dismissed_at.map(|d| d.to_rfc3339()))
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_pending_snippets(&self) -> Result<Vec<NarrativeSnippet>> {
        let rows = sqlx::query_as::<_, SnippetRow>(
            "SELECT * FROM mirror_snippets WHERE dismissed_at IS NULL ORDER BY created_at DESC LIMIT 20",
        )
        .fetch_all(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        rows.into_iter().map(NarrativeSnippet::try_from).collect()
    }

    // -----------------------------------------------------------------------
    // Trend narratives
    // -----------------------------------------------------------------------

    pub async fn insert_trend_narrative(&self, narrative: &TrendNarrative) -> Result<()> {
        let improvement_highlights_json =
            serde_json::to_string(&narrative.improvement_highlights)?;
        let meta_rule_updates_json = serde_json::to_string(&narrative.meta_rule_updates)?;
        let user_feedback = narrative
            .user_feedback
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        sqlx::query(
            r#"
            INSERT INTO mirror_trend_narratives
                (id, generated_at, period_start, period_end, routing_summary,
                 improvement_highlights_json, experiment_summary, meta_rule_updates_json,
                 full_narrative, user_feedback)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(narrative.id.to_string())
        .bind(narrative.generated_at.to_rfc3339())
        .bind(narrative.period_start.to_rfc3339())
        .bind(narrative.period_end.to_rfc3339())
        .bind(&narrative.routing_summary)
        .bind(improvement_highlights_json)
        .bind(&narrative.experiment_summary)
        .bind(meta_rule_updates_json)
        .bind(&narrative.full_narrative)
        .bind(user_feedback)
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_latest_narrative(&self) -> Result<Option<TrendNarrative>> {
        let row = sqlx::query_as::<_, TrendNarrativeRow>(
            "SELECT * FROM mirror_trend_narratives ORDER BY generated_at DESC LIMIT 1",
        )
        .fetch_optional(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        row.map(TrendNarrative::try_from).transpose()
    }

    pub async fn get_narratives(&self, limit: u32) -> Result<Vec<TrendNarrative>> {
        let rows = sqlx::query_as::<_, TrendNarrativeRow>(
            "SELECT * FROM mirror_trend_narratives ORDER BY generated_at DESC LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        rows.into_iter().map(TrendNarrative::try_from).collect()
    }

    // -----------------------------------------------------------------------
    // Feedback
    // -----------------------------------------------------------------------

    pub async fn update_feedback(
        &self,
        target: &FeedbackTarget,
        item_id: Uuid,
        feedback: &UserFeedback,
    ) -> Result<()> {
        let feedback_json = serde_json::to_string(feedback)?;
        let id_str = item_id.to_string();
        let sql = match target {
            FeedbackTarget::Routing => {
                "UPDATE mirror_routing_snapshots SET user_feedback = ?1 WHERE id = ?2"
            }
            FeedbackTarget::Snippet => {
                "UPDATE mirror_snippets SET user_feedback = ?1 WHERE id = ?2"
            }
            FeedbackTarget::Narrative => {
                "UPDATE mirror_trend_narratives SET user_feedback = ?1 WHERE id = ?2"
            }
        };
        sqlx::query(sql)
            .bind(feedback_json)
            .bind(id_str)
            .execute(self.db())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Cleanup / retention
    // -----------------------------------------------------------------------

    /// Delete hourly routing snapshots older than `max_age_days`.
    /// Daily aggregates (window_hours != 1) are preserved.
    pub async fn cleanup_old_snapshots(&self, max_age_days: u32) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let result = sqlx::query(
            "DELETE FROM mirror_routing_snapshots WHERE captured_at < ?1 AND window_hours = 1",
        )
        .bind(cutoff.to_rfc3339())
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    pub async fn cleanup_old_snippets(&self, max_age_days: u32) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let result =
            sqlx::query("DELETE FROM mirror_snippets WHERE created_at < ?1")
                .bind(cutoff.to_rfc3339())
                .execute(self.db())
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::mirror::{MirrorAlertType, SkillRouteStats, SuggestedAction};
    use crate::repos::cognitive_migrations;

    async fn setup() -> MirrorRepo {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive_migrations())
            .await
            .unwrap();
        MirrorRepo::new(pool)
    }

    fn make_snapshot() -> RoutingSnapshot {
        let mut distribution = HashMap::new();
        distribution.insert(
            "general".to_string(),
            SkillRouteStats {
                count: 10,
                percentage: 100.0,
                avg_confidence: 0.9,
                top_triggers: vec!["hello".to_string()],
            },
        );
        RoutingSnapshot {
            id: Uuid::new_v4(),
            captured_at: Utc::now(),
            window_hours: 1,
            total_messages: 10,
            distribution,
            fallback_rate: 0.05,
            avg_routing_confidence: 0.9,
            low_confidence_count: 1,
            user_feedback: None,
        }
    }

    fn make_snippet() -> NarrativeSnippet {
        NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            alert_type: MirrorAlertType::RoutingDrift,
            headline: "Routing drifted".to_string(),
            body: "Finance skill usage dropped 20%.".to_string(),
            suggested_action: Some(SuggestedAction::ViewDetails),
            user_feedback: None,
            dismissed_at: None,
        }
    }

    fn make_narrative() -> TrendNarrative {
        TrendNarrative {
            id: Uuid::new_v4(),
            generated_at: Utc::now(),
            period_start: Utc::now() - chrono::Duration::days(7),
            period_end: Utc::now(),
            routing_summary: "Stable week".to_string(),
            improvement_highlights: vec!["Better task routing".to_string()],
            experiment_summary: "No active experiments".to_string(),
            meta_rule_updates: vec![],
            full_narrative: "This week was pretty stable overall.".to_string(),
            user_feedback: None,
        }
    }

    #[tokio::test]
    async fn test_insert_and_get_routing_snapshot() {
        let repo = setup().await;
        let snap = make_snapshot();
        repo.insert_routing_snapshot(&snap).await.unwrap();

        let latest = repo.get_latest_routing_snapshot().await.unwrap();
        assert!(latest.is_some());
        let got = latest.unwrap();
        assert_eq!(got.id, snap.id);
        assert_eq!(got.total_messages, 10);
        assert_eq!(got.window_hours, 1);
        assert!((got.fallback_rate - 0.05).abs() < 1e-9);
        assert!(got.distribution.contains_key("general"));

        let history = repo.get_routing_history(7).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, snap.id);
    }

    #[tokio::test]
    async fn test_insert_and_get_snippet() {
        let repo = setup().await;
        let snippet = make_snippet();
        repo.insert_snippet(&snippet).await.unwrap();

        let pending = repo.get_pending_snippets().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, snippet.id);
        assert_eq!(pending[0].headline, "Routing drifted");
        assert_eq!(pending[0].alert_type, MirrorAlertType::RoutingDrift);
        assert!(pending[0].dismissed_at.is_none());
    }

    #[tokio::test]
    async fn test_insert_and_get_narrative() {
        let repo = setup().await;
        let narrative = make_narrative();
        repo.insert_trend_narrative(&narrative).await.unwrap();

        let latest = repo.get_latest_narrative().await.unwrap();
        assert!(latest.is_some());
        let got = latest.unwrap();
        assert_eq!(got.id, narrative.id);
        assert_eq!(got.routing_summary, "Stable week");
        assert_eq!(got.improvement_highlights, vec!["Better task routing"]);

        let all = repo.get_narratives(10).await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_feedback_update() {
        let repo = setup().await;

        // Routing feedback
        let snap = make_snapshot();
        repo.insert_routing_snapshot(&snap).await.unwrap();
        repo.update_feedback(&FeedbackTarget::Routing, snap.id, &UserFeedback::Helpful)
            .await
            .unwrap();
        let got = repo.get_latest_routing_snapshot().await.unwrap().unwrap();
        assert_eq!(got.user_feedback, Some(UserFeedback::Helpful));

        // Snippet feedback
        let snippet = make_snippet();
        repo.insert_snippet(&snippet).await.unwrap();
        repo.update_feedback(
            &FeedbackTarget::Snippet,
            snippet.id,
            &UserFeedback::NotHelpful,
        )
        .await
        .unwrap();
        let pending = repo.get_pending_snippets().await.unwrap();
        assert_eq!(pending[0].user_feedback, Some(UserFeedback::NotHelpful));

        // Narrative feedback
        let narrative = make_narrative();
        repo.insert_trend_narrative(&narrative).await.unwrap();
        repo.update_feedback(
            &FeedbackTarget::Narrative,
            narrative.id,
            &UserFeedback::Dismissed,
        )
        .await
        .unwrap();
        let got = repo.get_latest_narrative().await.unwrap().unwrap();
        assert_eq!(got.user_feedback, Some(UserFeedback::Dismissed));
    }

    #[tokio::test]
    async fn test_retention_cleanup() {
        let repo = setup().await;

        // Insert a recent snapshot (should survive cleanup)
        let recent_snap = make_snapshot();
        repo.insert_routing_snapshot(&recent_snap).await.unwrap();

        // Insert an old snapshot by overriding captured_at to 100 days ago
        let old_id = Uuid::new_v4();
        let old_time = Utc::now() - chrono::Duration::days(100);
        sqlx::query(
            "INSERT INTO mirror_routing_snapshots
             (id, captured_at, window_hours, total_messages, distribution_json,
              fallback_rate, avg_routing_confidence, low_confidence_count)
             VALUES (?1, ?2, 1, 5, '{}', 0.0, 0.9, 0)",
        )
        .bind(old_id.to_string())
        .bind(old_time.to_rfc3339())
        .execute(repo.db())
        .await
        .unwrap();

        let deleted = repo.cleanup_old_snapshots(30).await.unwrap();
        assert_eq!(deleted, 1);

        let history = repo.get_routing_history(365).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, recent_snap.id);

        // Snippet cleanup
        let snippet = make_snippet();
        repo.insert_snippet(&snippet).await.unwrap();
        // Insert old snippet
        let old_snippet_id = Uuid::new_v4();
        let old_snippet_time = Utc::now() - chrono::Duration::days(100);
        sqlx::query(
            "INSERT INTO mirror_snippets
             (id, created_at, alert_type, headline, body)
             VALUES (?1, ?2, 'RoutingDrift', 'Old headline', 'Old body')",
        )
        .bind(old_snippet_id.to_string())
        .bind(old_snippet_time.to_rfc3339())
        .execute(repo.db())
        .await
        .unwrap();

        let deleted_snippets = repo.cleanup_old_snippets(30).await.unwrap();
        assert_eq!(deleted_snippets, 1);
    }
}
