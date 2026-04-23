//! Repository for Phase 1 Mirror tables.

use common::Result;
use jiff::Timestamp;
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use crate::mirror::{
    BrainVersion, CategorySpend, FeedbackTarget, FinanceDriftSnapshot, MetaRule, MetaRuleAction,
    MetaRuleSource, MetaRuleStatus, NarrativeSnippet, PreviewRecommendation, RoutingSnapshot,
    TaskFocusSnapshot, TrendNarrative, TrialEarlySignals, TrialPreview, UserFeedback,
};

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse an RFC3339 timestamp string to Timestamp
fn parse_rfc3339(s: &str) -> common::Result<Timestamp> {
    s.parse::<Timestamp>()
        .map_err(|e| common::KlyntbotError::Storage(format!("bad timestamp: {e}")))
}

/// Serialize a serde enum variant to a bare string (strips JSON quotes)
fn enum_to_str<T: serde::Serialize>(val: &T) -> common::Result<String> {
    Ok(serde_json::to_string(val)?.trim_matches('"').to_string())
}

/// Deserialize a bare string back to a serde enum variant
fn str_to_enum<T: serde::de::DeserializeOwned>(s: &str) -> common::Result<T> {
    serde_json::from_str(&format!("\"{}\"", s))
        .map_err(|e| common::KlyntbotError::Storage(format!("bad enum value '{}': {e}", s)))
}

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
        let captured_at = parse_rfc3339(&row.captured_at)?;
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
            generated_at: parse_rfc3339(&row.generated_at)?,
            period_start: parse_rfc3339(&row.period_start)?,
            period_end: parse_rfc3339(&row.period_end)?,
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
        let created_at = parse_rfc3339(&row.created_at)?;
        let alert_type = str_to_enum(&row.alert_type)?;
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
        let dismissed_at = row.dismissed_at.as_deref().map(parse_rfc3339).transpose()?;
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

#[derive(Debug, sqlx::FromRow)]
struct MetaRuleRow {
    id: String,
    trigger_condition: String,
    action_json: String,
    source: String,
    effectiveness_score: f64,
    status: String,
    signal_count: i64,
    created_at: String,
    updated_at: String,
}

impl TryFrom<MetaRuleRow> for MetaRule {
    type Error = common::KlyntbotError;

    fn try_from(row: MetaRuleRow) -> Result<Self> {
        let action: MetaRuleAction = serde_json::from_str(&row.action_json)?;
        let source: MetaRuleSource = str_to_enum(&row.source)?;
        let status: MetaRuleStatus = str_to_enum(&row.status)?;
        Ok(MetaRule {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| common::KlyntbotError::Storage(format!("bad uuid: {e}")))?,
            trigger_condition: row.trigger_condition,
            action,
            source,
            effectiveness_score: row.effectiveness_score,
            status,
            signal_count: row.signal_count as u32,
            created_at: parse_rfc3339(&row.created_at)?,
            updated_at: parse_rfc3339(&row.updated_at)?,
        })
    }
}

#[derive(sqlx::FromRow)]
struct BrainVersionRow {
    version: i32,
    trial_id: Option<String>,
    promoted_at: String,
    params_json: String,
    reason: String,
    parent_version: Option<i32>,
    metrics_json: String,
    reverted: i32,
}

impl TryFrom<BrainVersionRow> for BrainVersion {
    type Error = common::KlyntbotError;

    fn try_from(row: BrainVersionRow) -> Result<Self> {
        Ok(BrainVersion {
            version: row.version as u32,
            trial_id: row.trial_id,
            promoted_at: parse_rfc3339(&row.promoted_at)?,
            params: serde_json::from_str(&row.params_json)?,
            reason: row.reason,
            parent_version: row.parent_version.map(|v| v as u32),
            metrics_at_promotion: serde_json::from_str(&row.metrics_json)?,
            reverted: row.reverted != 0,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct TrialPreviewRow {
    id: String,
    trial_id: String,
    started_at: String,
    preview_at: String,
    messages_scored: i64,
    early_signals_json: String,
    recommendation: String,
    narrative: String,
}

impl TryFrom<TrialPreviewRow> for TrialPreview {
    type Error = common::KlyntbotError;

    fn try_from(row: TrialPreviewRow) -> Result<Self> {
        let early_signals: TrialEarlySignals = serde_json::from_str(&row.early_signals_json)?;
        let recommendation: PreviewRecommendation = str_to_enum(&row.recommendation)?;
        Ok(TrialPreview {
            id: Uuid::parse_str(&row.id)
                .map_err(|e| common::KlyntbotError::Storage(format!("bad uuid: {e}")))?,
            trial_id: row.trial_id,
            started_at: parse_rfc3339(&row.started_at)?,
            preview_at: parse_rfc3339(&row.preview_at)?,
            messages_scored: row.messages_scored as u32,
            early_signals,
            recommendation,
            narrative: row.narrative,
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
        .bind(snap.captured_at.to_string())
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
        let cutoff = Timestamp::now() - jiff::SignedDuration::from_secs(days as i64 * 86400);
        let rows = sqlx::query_as::<_, RoutingSnapshotRow>(
            "SELECT * FROM mirror_routing_snapshots WHERE captured_at >= ?1 ORDER BY captured_at DESC",
        )
        .bind(cutoff.to_string())
        .fetch_all(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        rows.into_iter().map(RoutingSnapshot::try_from).collect()
    }

    // -----------------------------------------------------------------------
    // Narrative snippets
    // -----------------------------------------------------------------------

    pub async fn insert_snippet(&self, snippet: &NarrativeSnippet) -> Result<()> {
        let alert_type = enum_to_str(&snippet.alert_type)?;
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
        .bind(snippet.created_at.to_string())
        .bind(alert_type)
        .bind(&snippet.headline)
        .bind(&snippet.body)
        .bind(action_json)
        .bind(user_feedback)
        .bind(snippet.dismissed_at.map(|d| d.to_string()))
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
        let improvement_highlights_json = serde_json::to_string(&narrative.improvement_highlights)?;
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
        .bind(narrative.generated_at.to_string())
        .bind(narrative.period_start.to_string())
        .bind(narrative.period_end.to_string())
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
        let cutoff =
            Timestamp::now() - jiff::SignedDuration::from_secs(max_age_days as i64 * 86400);
        let result = sqlx::query(
            "DELETE FROM mirror_routing_snapshots WHERE captured_at < ?1 AND window_hours = 1",
        )
        .bind(cutoff.to_string())
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    pub async fn cleanup_old_snippets(&self, max_age_days: u32) -> Result<u64> {
        let cutoff =
            Timestamp::now() - jiff::SignedDuration::from_secs(max_age_days as i64 * 86400);
        let result = sqlx::query("DELETE FROM mirror_snippets WHERE created_at < ?1")
            .bind(cutoff.to_string())
            .execute(self.db())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    /// Delete trend narratives older than `max_age_days`. Returns rows removed.
    pub async fn cleanup_old_trend_narratives(&self, max_age_days: u32) -> Result<u64> {
        let cutoff =
            Timestamp::now() - jiff::SignedDuration::from_secs(max_age_days as i64 * 86400);
        let result = sqlx::query("DELETE FROM mirror_trend_narratives WHERE generated_at < ?1")
            .bind(cutoff.to_string())
            .execute(self.db())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    /// Delete `Disabled` meta-rules older than `max_age_days`. Active/Pending
    /// rules are never auto-removed. Returns rows removed.
    pub async fn cleanup_old_meta_rules(&self, max_age_days: u32) -> Result<u64> {
        let cutoff =
            Timestamp::now() - jiff::SignedDuration::from_secs(max_age_days as i64 * 86400);
        let result = sqlx::query(
            "DELETE FROM mirror_meta_rules
             WHERE status = 'Disabled' AND updated_at < ?1",
        )
        .bind(cutoff.to_string())
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    /// Delete brain versions marked `reverted = 1` older than `max_age_days`.
    /// Promoted versions are never deleted.
    pub async fn cleanup_reverted_brain_versions(&self, max_age_days: u32) -> Result<u64> {
        let cutoff =
            Timestamp::now() - jiff::SignedDuration::from_secs(max_age_days as i64 * 86400);
        let result = sqlx::query(
            "DELETE FROM mirror_brain_versions
             WHERE reverted = 1 AND promoted_at < ?1",
        )
        .bind(cutoff.to_string())
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    // -----------------------------------------------------------------------
    // Meta-rules
    // -----------------------------------------------------------------------

    pub async fn insert_meta_rule(&self, rule: &MetaRule) -> Result<()> {
        let action_json = serde_json::to_string(&rule.action)?;
        let source = enum_to_str(&rule.source)?;
        let status = enum_to_str(&rule.status)?;
        sqlx::query(
            r#"
            INSERT INTO mirror_meta_rules
                (id, trigger_condition, action_json, source, effectiveness_score,
                 status, signal_count, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(rule.id.to_string())
        .bind(&rule.trigger_condition)
        .bind(action_json)
        .bind(source)
        .bind(rule.effectiveness_score)
        .bind(status)
        .bind(rule.signal_count as i64)
        .bind(rule.created_at.to_string())
        .bind(rule.updated_at.to_string())
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_meta_rule_by_id(&self, id: Uuid) -> Result<Option<MetaRule>> {
        let row = sqlx::query_as::<_, MetaRuleRow>("SELECT * FROM mirror_meta_rules WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(self.db())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        row.map(MetaRule::try_from).transpose()
    }

    pub async fn get_meta_rules_by_status(&self, status: MetaRuleStatus) -> Result<Vec<MetaRule>> {
        let status_str = enum_to_str(&status)?;
        let rows = sqlx::query_as::<_, MetaRuleRow>(
            "SELECT * FROM mirror_meta_rules WHERE status = ?1 ORDER BY created_at DESC",
        )
        .bind(status_str)
        .fetch_all(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        rows.into_iter().map(MetaRule::try_from).collect()
    }

    pub async fn update_meta_rule_status(&self, id: Uuid, status: MetaRuleStatus) -> Result<()> {
        let status_str = enum_to_str(&status)?;
        sqlx::query("UPDATE mirror_meta_rules SET status = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(status_str)
            .bind(Timestamp::now().to_string())
            .bind(id.to_string())
            .execute(self.db())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn increment_meta_rule_signal(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE mirror_meta_rules SET signal_count = signal_count + 1, updated_at = ?1 \
             WHERE id = ?2",
        )
        .bind(Timestamp::now().to_string())
        .bind(id.to_string())
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn decay_meta_rule_effectiveness(&self, id: Uuid, decay_factor: f64) -> Result<()> {
        sqlx::query(
            "UPDATE mirror_meta_rules \
             SET effectiveness_score = effectiveness_score * ?1, updated_at = ?2 \
             WHERE id = ?3",
        )
        .bind(decay_factor)
        .bind(Timestamp::now().to_string())
        .bind(id.to_string())
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Brain versions
    // -----------------------------------------------------------------------

    pub async fn insert_brain_version(&self, v: &BrainVersion) -> Result<()> {
        let params_json = serde_json::to_string(&v.params)?;
        let metrics_json = serde_json::to_string(&v.metrics_at_promotion)?;
        sqlx::query(
            r#"
            INSERT INTO mirror_brain_versions
                (version, trial_id, promoted_at, params_json, reason,
                 parent_version, metrics_json, reverted)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(v.version as i64)
        .bind(&v.trial_id)
        .bind(v.promoted_at.to_string())
        .bind(params_json)
        .bind(&v.reason)
        .bind(v.parent_version.map(|p| p as i64))
        .bind(metrics_json)
        .bind(v.reverted as i64)
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_latest_brain_version(&self) -> Result<Option<BrainVersion>> {
        let row = sqlx::query_as::<_, BrainVersionRow>(
            "SELECT * FROM mirror_brain_versions ORDER BY version DESC LIMIT 1",
        )
        .fetch_optional(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        row.map(BrainVersion::try_from).transpose()
    }

    pub async fn get_brain_versions(&self) -> Result<Vec<BrainVersion>> {
        let rows = sqlx::query_as::<_, BrainVersionRow>(
            "SELECT * FROM mirror_brain_versions ORDER BY version DESC LIMIT 50",
        )
        .fetch_all(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        rows.into_iter().map(BrainVersion::try_from).collect()
    }

    pub async fn get_brain_version(&self, version: u32) -> Result<Option<BrainVersion>> {
        let row = sqlx::query_as::<_, BrainVersionRow>(
            "SELECT * FROM mirror_brain_versions WHERE version = ?1",
        )
        .bind(version as i64)
        .fetch_optional(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        row.map(BrainVersion::try_from).transpose()
    }

    pub async fn get_next_version_number(&self) -> Result<u32> {
        let row: (i64,) =
            sqlx::query_as("SELECT COALESCE(MAX(version), 0) FROM mirror_brain_versions")
                .fetch_one(self.db())
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok((row.0 + 1) as u32)
    }

    pub async fn mark_versions_reverted_after(&self, target_version: u32) -> Result<()> {
        sqlx::query("UPDATE mirror_brain_versions SET reverted = 1 WHERE version > ?1")
            .bind(target_version as i64)
            .execute(self.db())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Trial previews
    // -----------------------------------------------------------------------

    pub async fn insert_trial_preview(&self, preview: &TrialPreview) -> Result<()> {
        let early_signals_json = serde_json::to_string(&preview.early_signals)?;
        let recommendation = enum_to_str(&preview.recommendation)?;
        sqlx::query(
            r#"
            INSERT INTO mirror_trial_previews
                (id, trial_id, started_at, preview_at, messages_scored,
                 early_signals_json, recommendation, narrative)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(preview.id.to_string())
        .bind(&preview.trial_id)
        .bind(preview.started_at.to_string())
        .bind(preview.preview_at.to_string())
        .bind(preview.messages_scored as i64)
        .bind(early_signals_json)
        .bind(recommendation)
        .bind(&preview.narrative)
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_recent_trial_previews(&self) -> Result<Vec<TrialPreview>> {
        let rows = sqlx::query_as::<_, TrialPreviewRow>(
            "SELECT * FROM mirror_trial_previews ORDER BY preview_at DESC LIMIT 10",
        )
        .fetch_all(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        rows.into_iter().map(TrialPreview::try_from).collect()
    }

    pub async fn get_trial_preview_by_trial_id(
        &self,
        trial_id: &str,
    ) -> Result<Option<TrialPreview>> {
        let row = sqlx::query_as::<_, TrialPreviewRow>(
            "SELECT * FROM mirror_trial_previews WHERE trial_id = ?1",
        )
        .bind(trial_id)
        .fetch_optional(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        row.map(TrialPreview::try_from).transpose()
    }

    pub async fn cleanup_old_trial_previews(&self, max_age_days: u32) -> Result<u64> {
        let cutoff =
            Timestamp::now() - jiff::SignedDuration::from_secs(max_age_days as i64 * 86400);
        let result = sqlx::query("DELETE FROM mirror_trial_previews WHERE preview_at < ?1")
            .bind(cutoff.to_string())
            .execute(self.db())
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    // -----------------------------------------------------------------------
    // Task focus snapshots
    // -----------------------------------------------------------------------

    pub async fn insert_task_focus_snapshot(&self, snap: &TaskFocusSnapshot) -> Result<()> {
        let top_json = serde_json::to_string(&snap.top_tasks)
            .map_err(|e| common::KlyntbotError::Storage(format!("serialize top_tasks: {e}")))?;
        sqlx::query(
            "INSERT INTO mirror_task_focus_snapshots
             (id, captured_at, window_hours, focus_changes, tasks_completed,
              completion_rate, longest_unfinished_secs, top_tasks_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(snap.id.to_string())
        .bind(snap.captured_at.to_string())
        .bind(snap.window_hours as i64)
        .bind(snap.focus_changes as i64)
        .bind(snap.tasks_completed as i64)
        .bind(snap.completion_rate)
        .bind(snap.longest_unfinished_secs)
        .bind(top_json)
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("insert task focus: {e}")))?;
        Ok(())
    }

    pub async fn get_latest_task_focus_snapshot(&self) -> Result<Option<TaskFocusSnapshot>> {
        let row = sqlx::query(
            "SELECT id, captured_at, window_hours, focus_changes, tasks_completed,
                    completion_rate, longest_unfinished_secs, top_tasks_json
             FROM mirror_task_focus_snapshots ORDER BY captured_at DESC LIMIT 1",
        )
        .fetch_optional(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("latest task focus: {e}")))?;
        let Some(row) = row else { return Ok(None) };
        let id: String = row.try_get(0)?;
        let captured_at: String = row.try_get(1)?;
        let window_hours: i64 = row.try_get(2)?;
        let focus_changes: i64 = row.try_get(3)?;
        let tasks_completed: i64 = row.try_get(4)?;
        let completion_rate: f64 = row.try_get(5)?;
        let longest_unfinished_secs: Option<i64> = row.try_get(6)?;
        let top_tasks_json: String = row.try_get(7)?;
        let top_tasks: Vec<(String, u32)> = serde_json::from_str(&top_tasks_json)
            .map_err(|e| common::KlyntbotError::Storage(format!("deserialize top_tasks: {e}")))?;
        Ok(Some(TaskFocusSnapshot {
            id: Uuid::parse_str(&id)
                .map_err(|e| common::KlyntbotError::Storage(format!("uuid: {e}")))?,
            captured_at: captured_at
                .parse()
                .map_err(|e| common::KlyntbotError::Storage(format!("ts: {e}")))?,
            window_hours: window_hours as u8,
            focus_changes: focus_changes as u32,
            tasks_completed: tasks_completed as u32,
            completion_rate,
            longest_unfinished_secs,
            top_tasks,
        }))
    }

    // -----------------------------------------------------------------------
    // Finance drift snapshots
    // -----------------------------------------------------------------------

    pub async fn insert_finance_drift_snapshot(&self, snap: &FinanceDriftSnapshot) -> Result<()> {
        let cat_json = serde_json::to_string(&snap.per_category)
            .map_err(|e| common::KlyntbotError::Storage(format!("serialize categories: {e}")))?;
        sqlx::query(
            "INSERT INTO mirror_finance_drift_snapshots
             (id, captured_at, window_hours, total_transactions, over_budget_count, per_category_json)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(snap.id.to_string())
        .bind(snap.captured_at.to_string())
        .bind(snap.window_hours as i64)
        .bind(snap.total_transactions as i64)
        .bind(snap.over_budget_count as i64)
        .bind(cat_json)
        .execute(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("insert finance drift: {e}")))?;
        Ok(())
    }

    pub async fn get_latest_finance_drift_snapshot(&self) -> Result<Option<FinanceDriftSnapshot>> {
        let row = sqlx::query(
            "SELECT id, captured_at, window_hours, total_transactions, over_budget_count, per_category_json
             FROM mirror_finance_drift_snapshots ORDER BY captured_at DESC LIMIT 1",
        )
        .fetch_optional(self.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("latest finance drift: {e}")))?;
        let Some(row) = row else { return Ok(None) };
        let id: String = row.try_get(0)?;
        let captured_at: String = row.try_get(1)?;
        let window_hours: i64 = row.try_get(2)?;
        let total_transactions: i64 = row.try_get(3)?;
        let over_budget_count: i64 = row.try_get(4)?;
        let per_category_json: String = row.try_get(5)?;
        let per_category: HashMap<String, CategorySpend> = serde_json::from_str(&per_category_json)
            .map_err(|e| common::KlyntbotError::Storage(format!("deserialize categories: {e}")))?;
        Ok(Some(FinanceDriftSnapshot {
            id: Uuid::parse_str(&id)
                .map_err(|e| common::KlyntbotError::Storage(format!("uuid: {e}")))?,
            captured_at: captured_at
                .parse()
                .map_err(|e| common::KlyntbotError::Storage(format!("ts: {e}")))?,
            window_hours: window_hours as u8,
            total_transactions: total_transactions as u32,
            over_budget_count: over_budget_count as u32,
            per_category,
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::mirror::{
        MetaRuleAction, MirrorAlertType, PreviewRecommendation, SkillRouteStats, SuggestedAction,
        TrendDirection, TrialEarlySignals, TrialPreview,
    };

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
            captured_at: Timestamp::now(),
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
            created_at: Timestamp::now(),
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
            generated_at: Timestamp::now(),
            period_start: Timestamp::now() - jiff::SignedDuration::from_secs(7 * 86400),
            period_end: Timestamp::now(),
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
        let repo = crate::mirror::test_mirror_repo().await;
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
        let repo = crate::mirror::test_mirror_repo().await;
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
        let repo = crate::mirror::test_mirror_repo().await;
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
        let repo = crate::mirror::test_mirror_repo().await;

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
        let repo = crate::mirror::test_mirror_repo().await;

        // Insert a recent snapshot (should survive cleanup)
        let recent_snap = make_snapshot();
        repo.insert_routing_snapshot(&recent_snap).await.unwrap();

        // Insert an old snapshot by overriding captured_at to 100 days ago
        let old_id = Uuid::new_v4();
        let old_time = Timestamp::now() - jiff::SignedDuration::from_secs(100 * 86400);
        sqlx::query(
            "INSERT INTO mirror_routing_snapshots
             (id, captured_at, window_hours, total_messages, distribution_json,
              fallback_rate, avg_routing_confidence, low_confidence_count)
             VALUES (?1, ?2, 1, 5, '{}', 0.0, 0.9, 0)",
        )
        .bind(old_id.to_string())
        .bind(old_time.to_string())
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
        let old_snippet_time = Timestamp::now() - jiff::SignedDuration::from_secs(100 * 86400);
        sqlx::query(
            "INSERT INTO mirror_snippets
             (id, created_at, alert_type, headline, body)
             VALUES (?1, ?2, 'RoutingDrift', 'Old headline', 'Old body')",
        )
        .bind(old_snippet_id.to_string())
        .bind(old_snippet_time.to_string())
        .execute(repo.db())
        .await
        .unwrap();

        let deleted_snippets = repo.cleanup_old_snippets(30).await.unwrap();
        assert_eq!(deleted_snippets, 1);
    }

    fn make_meta_rule() -> MetaRule {
        MetaRule {
            id: Uuid::new_v4(),
            trigger_condition: "user corrects finance twice".to_string(),
            action: MetaRuleAction::ForceClarification,
            source: MetaRuleSource::CorrectionDerived,
            effectiveness_score: 0.5,
            status: MetaRuleStatus::Pending,
            signal_count: 0,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn test_insert_and_get_meta_rule() {
        let repo = crate::mirror::test_mirror_repo().await;
        let rule = make_meta_rule();
        repo.insert_meta_rule(&rule).await.unwrap();

        let rules = repo
            .get_meta_rules_by_status(MetaRuleStatus::Pending)
            .await
            .unwrap();
        assert_eq!(rules.len(), 1);
        let got = &rules[0];
        assert_eq!(got.id, rule.id);
        assert_eq!(got.trigger_condition, "user corrects finance twice");
        assert_eq!(got.source, MetaRuleSource::CorrectionDerived);
        assert_eq!(got.status, MetaRuleStatus::Pending);
        assert_eq!(got.signal_count, 0);
        assert!((got.effectiveness_score - 0.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_approve_meta_rule() {
        let repo = crate::mirror::test_mirror_repo().await;
        let rule = make_meta_rule();
        repo.insert_meta_rule(&rule).await.unwrap();

        repo.update_meta_rule_status(rule.id, MetaRuleStatus::Active)
            .await
            .unwrap();

        let pending = repo
            .get_meta_rules_by_status(MetaRuleStatus::Pending)
            .await
            .unwrap();
        assert!(pending.is_empty());

        let active = repo
            .get_meta_rules_by_status(MetaRuleStatus::Active)
            .await
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].status, MetaRuleStatus::Active);
    }

    #[tokio::test]
    async fn test_dismiss_meta_rule() {
        let repo = crate::mirror::test_mirror_repo().await;
        let rule = make_meta_rule();
        repo.insert_meta_rule(&rule).await.unwrap();

        repo.update_meta_rule_status(rule.id, MetaRuleStatus::Disabled)
            .await
            .unwrap();

        let pending = repo
            .get_meta_rules_by_status(MetaRuleStatus::Pending)
            .await
            .unwrap();
        assert!(pending.is_empty());

        let disabled = repo
            .get_meta_rules_by_status(MetaRuleStatus::Disabled)
            .await
            .unwrap();
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].status, MetaRuleStatus::Disabled);
    }

    #[tokio::test]
    async fn test_increment_signal_count() {
        let repo = crate::mirror::test_mirror_repo().await;
        let rule = make_meta_rule();
        repo.insert_meta_rule(&rule).await.unwrap();

        repo.increment_meta_rule_signal(rule.id).await.unwrap();
        repo.increment_meta_rule_signal(rule.id).await.unwrap();

        let rules = repo
            .get_meta_rules_by_status(MetaRuleStatus::Pending)
            .await
            .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].signal_count, 2);
    }

    fn make_brain_version(version: u32, parent: Option<u32>) -> BrainVersion {
        BrainVersion {
            version,
            trial_id: Some(format!("trial-{version}")),
            promoted_at: Timestamp::now(),
            params: serde_json::json!({"temperature": 0.7}),
            reason: format!("reason-{version}"),
            parent_version: parent,
            metrics_at_promotion: serde_json::json!({"improvement_pct": 5.0}),
            reverted: false,
        }
    }

    #[tokio::test]
    async fn test_insert_and_get_brain_version() {
        let repo = crate::mirror::test_mirror_repo().await;
        let v = make_brain_version(1, None);
        repo.insert_brain_version(&v).await.unwrap();

        let latest = repo.get_latest_brain_version().await.unwrap();
        assert!(latest.is_some());
        let got = latest.unwrap();
        assert_eq!(got.version, 1);
        assert_eq!(got.reason, "reason-1");
        assert_eq!(got.trial_id, Some("trial-1".to_string()));
        assert!(!got.reverted);
    }

    #[tokio::test]
    async fn test_get_brain_versions_ordered() {
        let repo = crate::mirror::test_mirror_repo().await;
        repo.insert_brain_version(&make_brain_version(1, None))
            .await
            .unwrap();
        repo.insert_brain_version(&make_brain_version(2, Some(1)))
            .await
            .unwrap();
        repo.insert_brain_version(&make_brain_version(3, Some(2)))
            .await
            .unwrap();

        let versions = repo.get_brain_versions().await.unwrap();
        assert_eq!(versions.len(), 3);
        // Should be DESC order
        assert_eq!(versions[0].version, 3);
        assert_eq!(versions[1].version, 2);
        assert_eq!(versions[2].version, 1);
    }

    #[tokio::test]
    async fn test_get_next_version_number() {
        let repo = crate::mirror::test_mirror_repo().await;

        // Empty table → next is 1
        let next = repo.get_next_version_number().await.unwrap();
        assert_eq!(next, 1);

        // After inserting version 1 → next is 2
        repo.insert_brain_version(&make_brain_version(1, None))
            .await
            .unwrap();
        let next = repo.get_next_version_number().await.unwrap();
        assert_eq!(next, 2);
    }

    #[tokio::test]
    async fn test_mark_versions_reverted() {
        let repo = crate::mirror::test_mirror_repo().await;
        repo.insert_brain_version(&make_brain_version(1, None))
            .await
            .unwrap();
        repo.insert_brain_version(&make_brain_version(2, Some(1)))
            .await
            .unwrap();
        repo.insert_brain_version(&make_brain_version(3, Some(2)))
            .await
            .unwrap();

        repo.mark_versions_reverted_after(1).await.unwrap();

        let v1 = repo.get_brain_version(1).await.unwrap().unwrap();
        let v2 = repo.get_brain_version(2).await.unwrap().unwrap();
        let v3 = repo.get_brain_version(3).await.unwrap().unwrap();

        assert!(!v1.reverted);
        assert!(v2.reverted);
        assert!(v3.reverted);
    }

    fn make_trial_preview(trial_id: &str, recommendation: PreviewRecommendation) -> TrialPreview {
        TrialPreview {
            id: Uuid::new_v4(),
            trial_id: trial_id.to_string(),
            started_at: Timestamp::now() - jiff::SignedDuration::from_secs(4 * 3600),
            preview_at: Timestamp::now(),
            messages_scored: 25,
            early_signals: TrialEarlySignals {
                correction_rate_delta: -0.15,
                confidence_trend: TrendDirection::Falling,
                dominant_skill_shift: Some("finance-management".to_string()),
                messages_scored: 0,
            },
            recommendation,
            narrative: "Correction rate worsened 15% vs champion".to_string(),
        }
    }

    #[tokio::test]
    async fn test_insert_and_get_trial_preview() {
        let repo = crate::mirror::test_mirror_repo().await;
        let preview = make_trial_preview("trial-abc", PreviewRecommendation::Kill);
        repo.insert_trial_preview(&preview).await.unwrap();
        let previews = repo.get_recent_trial_previews().await.unwrap();
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].trial_id, "trial-abc");
        assert_eq!(previews[0].recommendation, PreviewRecommendation::Kill);
    }

    #[tokio::test]
    async fn test_get_trial_preview_by_trial_id() {
        let repo = crate::mirror::test_mirror_repo().await;
        let preview = make_trial_preview("trial-abc", PreviewRecommendation::Kill);
        repo.insert_trial_preview(&preview).await.unwrap();
        let found = repo
            .get_trial_preview_by_trial_id("trial-abc")
            .await
            .unwrap();
        assert!(found.is_some());
        let not_found = repo
            .get_trial_preview_by_trial_id("trial-xyz")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn cleanup_old_trend_narratives_keeps_recent() {
        let repo = crate::mirror::test_mirror_repo().await;
        let old = TrendNarrative {
            id: Uuid::new_v4(),
            generated_at: Timestamp::now() - jiff::SignedDuration::from_secs(400 * 86400),
            period_start: Timestamp::now(),
            period_end: Timestamp::now(),
            routing_summary: "old".into(),
            improvement_highlights: vec![],
            experiment_summary: String::new(),
            meta_rule_updates: vec![],
            full_narrative: "old narrative".into(),
            user_feedback: None,
        };
        let recent = TrendNarrative {
            id: Uuid::new_v4(),
            generated_at: Timestamp::now(),
            ..old.clone()
        };
        repo.insert_trend_narrative(&old).await.unwrap();
        repo.insert_trend_narrative(&recent).await.unwrap();

        let deleted = repo.cleanup_old_trend_narratives(365).await.unwrap();
        assert_eq!(deleted, 1);

        let remaining = repo.get_narratives(100).await.unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[tokio::test]
    async fn cleanup_old_meta_rules_only_disabled() {
        let repo = crate::mirror::test_mirror_repo().await;
        let mk = |status: MetaRuleStatus, created_at: Timestamp| MetaRule {
            id: Uuid::new_v4(),
            trigger_condition: String::new(),
            action: MetaRuleAction::ForceClarification,
            source: MetaRuleSource::CorrectionDerived,
            effectiveness_score: 0.5,
            status,
            signal_count: 0,
            created_at,
            updated_at: created_at,
        };
        let old_disabled = mk(
            MetaRuleStatus::Disabled,
            Timestamp::now() - jiff::SignedDuration::from_secs(200 * 86400),
        );
        let old_active = mk(
            MetaRuleStatus::Active,
            Timestamp::now() - jiff::SignedDuration::from_secs(200 * 86400),
        );
        let recent_disabled = mk(MetaRuleStatus::Disabled, Timestamp::now());
        repo.insert_meta_rule(&old_disabled).await.unwrap();
        repo.insert_meta_rule(&old_active).await.unwrap();
        repo.insert_meta_rule(&recent_disabled).await.unwrap();

        let deleted = repo.cleanup_old_meta_rules(180).await.unwrap();
        assert_eq!(deleted, 1);

        // Active + recent disabled remain.
        assert_eq!(
            repo.get_meta_rules_by_status(MetaRuleStatus::Active)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            repo.get_meta_rules_by_status(MetaRuleStatus::Disabled)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
