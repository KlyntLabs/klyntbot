//! MirrorFacade — public API for the Mirror self-reflection layer.
//!
//! Tauri commands and MCP handlers call this facade rather than accessing the
//! repo or subscribers directly.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use common::Result;

use crate::mirror::{
    FeedbackTarget, MetaRule, MetaRuleStatus, MirrorRepo, MirrorResponse, MirrorState,
    NarrativeContext, NarrativeHandler, NarrativeSnippet, RoutingSnapshot, TrendNarrative,
    UserFeedback,
};

// ---------------------------------------------------------------------------
// MirrorFacade
// ---------------------------------------------------------------------------

/// Public API surface for the Mirror self-reflection layer.
///
/// Provides state queries, user feedback submission, weekly narrative
/// generation, and conversational mirror responses.
pub struct MirrorFacade {
    pub(crate) repo: MirrorRepo,
    narrative_handler: Option<Arc<dyn NarrativeHandler>>,
}

impl MirrorFacade {
    /// Create a new facade backed by the given repo. No narrative handler is
    /// wired by default — call [`with_narrative_handler`](Self::with_narrative_handler)
    /// before using LLM-powered methods.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            narrative_handler: None,
        }
    }

    /// Attach a [`NarrativeHandler`] implementation (typically the agent-layer
    /// LLM wrapper). Returns `self` for builder-style chaining.
    pub fn with_narrative_handler(mut self, handler: Arc<dyn NarrativeHandler>) -> Self {
        self.narrative_handler = Some(handler);
        self
    }

    // -----------------------------------------------------------------------
    // State queries
    // -----------------------------------------------------------------------

    /// Compose the current [`MirrorState`] from the three primary repo queries.
    pub async fn get_state(&self) -> Result<MirrorState> {
        let (last_routing_snapshot, latest_trend_narrative, pending_snippets, active_meta_rules, pending_meta_rules) = tokio::try_join!(
            self.repo.get_latest_routing_snapshot(),
            self.repo.get_latest_narrative(),
            self.repo.get_pending_snippets(),
            self.repo.get_meta_rules_by_status(MetaRuleStatus::Active),
            self.repo.get_meta_rules_by_status(MetaRuleStatus::Pending),
        )?;

        Ok(MirrorState {
            last_routing_snapshot,
            latest_trend_narrative,
            pending_snippets,
            active_meta_rules,
            pending_meta_rules,
        })
    }

    /// Return routing snapshots captured within the last `days` days.
    pub async fn get_routing_history(&self, days: u32) -> Result<Vec<RoutingSnapshot>> {
        self.repo.get_routing_history(days).await
    }

    /// Return the most recent `limit` trend narratives, newest first.
    pub async fn get_narratives(&self, limit: u32) -> Result<Vec<TrendNarrative>> {
        self.repo.get_narratives(limit).await
    }

    /// Return all non-dismissed narrative snippets (up to 20, newest first).
    pub async fn get_pending_snippets(&self) -> Result<Vec<NarrativeSnippet>> {
        self.repo.get_pending_snippets().await
    }

    // -----------------------------------------------------------------------
    // Meta-rule management
    // -----------------------------------------------------------------------

    /// Approve a pending meta-rule, transitioning it to [`MetaRuleStatus::Active`].
    pub async fn approve_meta_rule(&self, rule_id: Uuid) -> Result<()> {
        self.repo
            .update_meta_rule_status(rule_id, MetaRuleStatus::Active)
            .await
    }

    /// Dismiss a pending meta-rule, transitioning it to [`MetaRuleStatus::Disabled`].
    pub async fn dismiss_meta_rule(&self, rule_id: Uuid) -> Result<()> {
        self.repo
            .update_meta_rule_status(rule_id, MetaRuleStatus::Disabled)
            .await
    }

    /// Return active and pending meta-rules as `(active, pending)`.
    pub async fn get_meta_rules(&self) -> Result<(Vec<MetaRule>, Vec<MetaRule>)> {
        let active = self
            .repo
            .get_meta_rules_by_status(MetaRuleStatus::Active)
            .await?;
        let pending = self
            .repo
            .get_meta_rules_by_status(MetaRuleStatus::Pending)
            .await?;
        Ok((active, pending))
    }

    // -----------------------------------------------------------------------
    // User actions
    // -----------------------------------------------------------------------

    /// Record user feedback for a narrative, snippet, or routing snapshot.
    pub async fn submit_feedback(
        &self,
        item_id: Uuid,
        target: FeedbackTarget,
        feedback: UserFeedback,
    ) -> Result<()> {
        self.repo.update_feedback(&target, item_id, &feedback).await
    }

    // -----------------------------------------------------------------------
    // Weekly narrative generation (called by cron)
    // -----------------------------------------------------------------------

    /// Generate a weekly trend narrative from the last 7 days of routing data
    /// and persist it to the repo.
    ///
    /// Returns an error if no [`NarrativeHandler`] has been configured.
    pub async fn generate_weekly_narrative(&self) -> Result<TrendNarrative> {
        let handler = self.require_handler()?;

        let period_end = Utc::now();
        let period_start = period_end - Duration::days(7);

        let snapshots = self.repo.get_routing_history(7).await?;

        let ctx = build_narrative_context((period_start, period_end), snapshots);

        let generated = handler.generate_narrative(ctx).await?;

        let narrative = TrendNarrative {
            id: Uuid::new_v4(),
            generated_at: Utc::now(),
            period_start,
            period_end,
            routing_summary: generated.routing_summary,
            improvement_highlights: generated.improvement_highlights,
            experiment_summary: String::new(),
            meta_rule_updates: vec![],
            full_narrative: generated.full_narrative,
            user_feedback: None,
        };

        self.repo.insert_trend_narrative(&narrative).await?;

        Ok(narrative)
    }

    // -----------------------------------------------------------------------
    // Conversational mirror (called by MirrorInput UI)
    // -----------------------------------------------------------------------

    /// Answer a user's direct mirror query using routing data from `period`.
    ///
    /// If `period` is `None`, defaults to the last 14 days.
    /// Returns an error if no [`NarrativeHandler`] has been configured.
    pub async fn generate_mirror_response(
        &self,
        query: String,
        period: Option<(DateTime<Utc>, DateTime<Utc>)>,
    ) -> Result<MirrorResponse> {
        let handler = self.require_handler()?;

        let (period_start, period_end) = period.unwrap_or_else(|| {
            let end = Utc::now();
            let start = end - Duration::days(14);
            (start, end)
        });

        let days = (period_end - period_start).num_days().max(1) as u32;
        let snapshots = self.repo.get_routing_history(days).await?;

        let ctx = build_narrative_context((period_start, period_end), snapshots);

        let answer = handler.generate_mirror_response(&query, ctx).await?;

        Ok(MirrorResponse {
            answer,
            data_sources_used: vec!["routing_snapshots".to_string()],
        })
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn require_handler(&self) -> Result<&dyn NarrativeHandler> {
        self.narrative_handler.as_deref().ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "NarrativeHandler not configured".to_string(),
            ))
        })
    }
}

// ---------------------------------------------------------------------------
// Context assembly helpers
// ---------------------------------------------------------------------------

fn build_narrative_context(
    period: (DateTime<Utc>, DateTime<Utc>),
    routing_snapshots: Vec<RoutingSnapshot>,
) -> NarrativeContext {
    // Aggregate skill usage percentages across all snapshots.
    let mut skill_totals: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for snap in &routing_snapshots {
        for (skill, stats) in &snap.distribution {
            *skill_totals.entry(skill.clone()).or_insert(0.0) += stats.percentage;
        }
    }

    let total_weight: f64 = skill_totals.values().sum();
    let mut top_skills: Vec<(String, f64)> = skill_totals
        .into_iter()
        .map(|(skill, weight)| {
            let pct = if total_weight > 0.0 {
                weight / total_weight * 100.0
            } else {
                0.0
            };
            (skill, pct)
        })
        .collect();
    top_skills.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let correction_count = 0u32;
    let past_narrative_feedback: Vec<UserFeedback> = routing_snapshots
        .iter()
        .filter_map(|s| s.user_feedback.clone())
        .collect();

    NarrativeContext {
        period,
        routing_snapshots,
        correction_count,
        top_skills_by_usage: top_skills,
        past_narrative_feedback,
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
        MetaRule, MetaRuleAction, MetaRuleSource, MetaRuleStatus, MirrorAlertType, SkillRouteStats,
    };

    async fn setup() -> MirrorFacade {
        MirrorFacade::new(crate::mirror::test_mirror_repo().await)
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

    #[tokio::test]
    async fn test_get_state_empty() {
        let facade = setup().await;
        let state = facade.get_state().await.unwrap();
        assert!(state.last_routing_snapshot.is_none());
        assert!(state.latest_trend_narrative.is_none());
        assert!(state.pending_snippets.is_empty());
    }

    #[tokio::test]
    async fn test_get_state_with_snapshot() {
        let facade = setup().await;
        let snap = make_snapshot();
        facade.repo.insert_routing_snapshot(&snap).await.unwrap();
        let state = facade.get_state().await.unwrap();
        assert!(state.last_routing_snapshot.is_some());
        assert_eq!(state.last_routing_snapshot.unwrap().id, snap.id);
    }

    #[tokio::test]
    async fn test_submit_feedback_routing() {
        let facade = setup().await;
        let snap = make_snapshot();
        facade.repo.insert_routing_snapshot(&snap).await.unwrap();

        facade
            .submit_feedback(snap.id, FeedbackTarget::Routing, UserFeedback::Helpful)
            .await
            .unwrap();

        let got = facade
            .repo
            .get_latest_routing_snapshot()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.user_feedback, Some(UserFeedback::Helpful));
    }

    #[tokio::test]
    async fn test_get_routing_history() {
        let facade = setup().await;
        let snap = make_snapshot();
        facade.repo.insert_routing_snapshot(&snap).await.unwrap();

        let history = facade.get_routing_history(7).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, snap.id);
    }

    #[tokio::test]
    async fn test_get_narratives_empty() {
        let facade = setup().await;
        let narratives = facade.get_narratives(10).await.unwrap();
        assert!(narratives.is_empty());
    }

    #[tokio::test]
    async fn test_get_pending_snippets_empty() {
        let facade = setup().await;
        let snippets = facade.get_pending_snippets().await.unwrap();
        assert!(snippets.is_empty());
    }

    #[tokio::test]
    async fn test_generate_weekly_narrative_no_handler() {
        let facade = setup().await;
        let result = facade.generate_weekly_narrative().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("NarrativeHandler not configured"));
    }

    #[tokio::test]
    async fn test_generate_mirror_response_no_handler() {
        let facade = setup().await;
        let result = facade
            .generate_mirror_response("How am I doing?".to_string(), None)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("NarrativeHandler not configured"));
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_approve_meta_rule() {
        let facade = setup().await;
        let rule = make_meta_rule();
        facade.repo.insert_meta_rule(&rule).await.unwrap();

        facade.approve_meta_rule(rule.id).await.unwrap();

        let state = facade.get_state().await.unwrap();
        assert_eq!(state.active_meta_rules.len(), 1);
        assert_eq!(state.active_meta_rules[0].id, rule.id);
        assert!(state.pending_meta_rules.is_empty());
    }

    #[tokio::test]
    async fn test_dismiss_meta_rule() {
        let facade = setup().await;
        let rule = make_meta_rule();
        facade.repo.insert_meta_rule(&rule).await.unwrap();

        facade.dismiss_meta_rule(rule.id).await.unwrap();

        let state = facade.get_state().await.unwrap();
        assert!(state.active_meta_rules.is_empty());
        assert!(state.pending_meta_rules.is_empty());
    }

    #[tokio::test]
    async fn test_get_state_includes_meta_rules() {
        let facade = setup().await;
        let rule = make_meta_rule();
        facade.repo.insert_meta_rule(&rule).await.unwrap();

        let state = facade.get_state().await.unwrap();
        assert_eq!(state.pending_meta_rules.len(), 1);
        assert_eq!(state.pending_meta_rules[0].id, rule.id);
        assert!(state.active_meta_rules.is_empty());
    }

    #[tokio::test]
    async fn test_get_pending_snippets_returns_undismissed() {
        let facade = setup().await;
        let snippet = crate::mirror::NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            alert_type: MirrorAlertType::RoutingDrift,
            headline: "Test snippet".to_string(),
            body: "Body text".to_string(),
            suggested_action: None,
            user_feedback: None,
            dismissed_at: None,
        };
        facade.repo.insert_snippet(&snippet).await.unwrap();

        let snippets = facade.get_pending_snippets().await.unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].id, snippet.id);
    }
}
