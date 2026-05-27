//! MirrorFacade — public API for the Mirror self-reflection layer.
//!
//! Tauri commands and MCP handlers call this facade rather than accessing the
//! repo or signal sources directly.

use std::sync::Arc;

use dashmap::DashMap;
use jiff::Timestamp;
use tokio::task::JoinHandle;
use uuid::Uuid;

use common::Result;

use crate::{
    AutotunerBridge, BrainVersion, FeedbackTarget, MetaRule, MetaRuleAction, MetaRuleSource,
    MetaRuleStatus, MirrorRepo, MirrorResponse, MirrorState, NarrativeContext, NarrativeHandler,
    NarrativeSnippet, RoutingSnapshot, TrendNarrative, TrialPreview, UserFeedback,
};
use cognitive_memory::repos::EpisodicMemoryRepo;
use cognitive_memory::types::ProceduralRule;

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
    autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
    pub(crate) active_timers: Option<Arc<DashMap<String, JoinHandle<()>>>>,
    episodic_repo: Option<EpisodicMemoryRepo>,
    rule_repo: Option<cognitive_memory::repos::ProceduralRuleRepo>,
    text_embedder: Option<Arc<dyn cognitive_memory::embedder::TextEmbedder>>,
    approval_history: Option<Arc<crate::sources::ApprovalHistorySource>>,
}

impl MirrorFacade {
    /// Create a new facade backed by the given repo. No narrative handler is
    /// wired by default — call [`with_narrative_handler`](Self::with_narrative_handler)
    /// before using LLM-powered methods.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            narrative_handler: None,
            autotuner_bridge: None,
            active_timers: None,
            episodic_repo: None,
            rule_repo: None,
            text_embedder: None,
            approval_history: None,
        }
    }

    /// Attach a [`NarrativeHandler`] implementation (typically the agent-layer
    /// LLM wrapper). Returns `self` for builder-style chaining.
    pub fn with_narrative_handler(mut self, handler: Arc<dyn NarrativeHandler>) -> Self {
        self.narrative_handler = Some(handler);
        self
    }

    /// Attach an [`AutotunerBridge`] so that [`revert_to_version`](Self::revert_to_version)
    /// can apply parameter changes. Returns `self` for builder-style chaining.
    pub fn with_autotuner_bridge(mut self, bridge: Arc<dyn AutotunerBridge>) -> Self {
        self.autotuner_bridge = Some(bridge);
        self
    }

    /// Attach shared active timers for trial preview cancellation.
    pub fn with_active_timers(mut self, timers: Arc<DashMap<String, JoinHandle<()>>>) -> Self {
        self.active_timers = Some(timers);
        self
    }

    /// Attach the approval history source for pattern suggestion.
    pub fn with_approval_history(
        mut self,
        source: Arc<crate::sources::ApprovalHistorySource>,
    ) -> Self {
        self.approval_history = Some(source);
        self
    }

    /// Suggest a grant pattern for a tool call based on approval history.
    pub async fn suggest_approval_pattern(
        &self,
        tool: &str,
        path: Option<&str>,
    ) -> Option<crate::sources::approval_history::SuggestedPattern> {
        self.approval_history
            .as_ref()?
            .suggest_pattern(tool, path)
            .await
    }

    /// Attach an [`EpisodicMemoryRepo`] so that key mirror actions (weekly
    /// narrative, meta-rule approval, trial kill) are persisted as episodic
    /// memories. Returns `self` for builder-style chaining.
    pub fn with_episodic_repo(mut self, repo: EpisodicMemoryRepo) -> Self {
        self.episodic_repo = Some(repo);
        self
    }

    /// Attach a [`ProceduralRuleRepo`](cognitive_memory::repos::ProceduralRuleRepo) so that
    /// approved meta-rules are promoted to procedural rules. Returns `self` for
    /// builder-style chaining.
    pub fn with_rule_repo(mut self, repo: cognitive_memory::repos::ProceduralRuleRepo) -> Self {
        self.rule_repo = Some(repo);
        self
    }

    /// Attach a [`TextEmbedder`](cognitive_memory::embedder::TextEmbedder) for semantic snippet similarity in voice echo.
    pub fn with_text_embedder(
        mut self,
        embedder: Arc<dyn cognitive_memory::embedder::TextEmbedder>,
    ) -> Self {
        self.text_embedder = Some(embedder);
        self
    }

    // -----------------------------------------------------------------------
    // State queries
    // -----------------------------------------------------------------------

    /// Compose the current [`MirrorState`] from the primary repo queries.
    pub async fn get_state(&self) -> Result<MirrorState> {
        let (
            last_routing_snapshot,
            latest_trend_narrative,
            pending_snippets,
            active_meta_rules,
            pending_meta_rules,
            latest_brain_version,
            recent_trial_previews,
        ) = tokio::try_join!(
            self.repo.get_latest_routing_snapshot(),
            self.repo.get_latest_narrative(),
            self.repo.get_pending_snippets(),
            self.repo.get_meta_rules_by_status(MetaRuleStatus::Active),
            self.repo.get_meta_rules_by_status(MetaRuleStatus::Pending),
            self.repo.get_latest_brain_version(),
            self.repo.get_recent_trial_previews(),
        )?;

        Ok(MirrorState {
            last_routing_snapshot,
            latest_trend_narrative,
            pending_snippets,
            active_meta_rules,
            pending_meta_rules,
            latest_brain_version,
            recent_trial_previews,
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
    // Brain version queries & revert
    // -----------------------------------------------------------------------

    /// Return all brain versions, newest first.
    pub async fn get_brain_versions(&self) -> Result<Vec<BrainVersion>> {
        self.repo.get_brain_versions().await
    }

    /// Revert DB state only — used in tests without autotuner bridge.
    pub(crate) async fn revert_to_version_db_only(&self, target: u32) -> Result<BrainVersion> {
        let target_v = self.repo.get_brain_version(target).await?.ok_or_else(|| {
            common::KlyntbotError::StorageNotFound(format!("Brain version {target} not found"))
        })?;
        self.repo.mark_versions_reverted_after(target).await?;
        let next = self.repo.get_next_version_number().await?;
        let new_v = BrainVersion {
            version: next,
            trial_id: None,
            promoted_at: Timestamp::now(),
            params: target_v.params.clone(),
            reason: format!("Reverted to v{target}"),
            parent_version: Some(next - 1),
            metrics_at_promotion: target_v.metrics_at_promotion.clone(),
            reverted: false,
        };
        self.repo.insert_brain_version(&new_v).await?;
        Ok(new_v)
    }

    /// Full revert: updates DB and applies params via [`AutotunerBridge`].
    pub async fn revert_to_version(&self, target: u32) -> Result<BrainVersion> {
        let new_v = self.revert_to_version_db_only(target).await?;
        if let Some(bridge) = &self.autotuner_bridge {
            bridge
                .apply_champion(new_v.params.clone(), new_v.reason.clone())
                .await?;
        }
        Ok(new_v)
    }

    // -----------------------------------------------------------------------
    // Meta-rule management
    // -----------------------------------------------------------------------

    /// Approve a pending meta-rule, transitioning it to [`MetaRuleStatus::Active`].
    ///
    /// If a [`ProceduralRuleRepo`](cognitive_memory::repos::ProceduralRuleRepo) is attached,
    /// the meta-rule is also promoted to a procedural rule. If a similar rule already
    /// exists its `signal_count` is incremented instead of creating a duplicate.
    pub async fn approve_meta_rule(&self, rule_id: Uuid) -> Result<()> {
        self.repo
            .update_meta_rule_status(rule_id, MetaRuleStatus::Active)
            .await?;

        self.write_episodic(format!("Approved meta-rule: {rule_id}"), None, 0.8);

        // Promote to procedural rule if a rule_repo is wired.
        if let Some(ref rule_repo) = self.rule_repo {
            if let Ok(Some(meta)) = self.repo.get_meta_rule_by_id(rule_id).await {
                let rule_text = meta_rule_to_rule_text(&meta);
                let domain = "general"; // must be in RULE_DOMAINS for context injection
                match rule_repo.find_similar(&rule_text, domain).await {
                    Ok(Some(existing)) => {
                        // Similar rule exists — reinforce it rather than duplicate.
                        if let Err(e) = rule_repo.increment_signal_count(&existing.id).await {
                            tracing::warn!(
                                "mirror: failed to increment signal_count for similar rule {}: {e}",
                                existing.id
                            );
                        }
                    }
                    Ok(None) => {
                        let now = Timestamp::now().to_string();
                        let procedural = ProceduralRule {
                            id: Uuid::new_v4().to_string(),
                            domain: domain.to_string(),
                            rule_text,
                            confidence: meta.effectiveness_score,
                            source: "mirror".to_string(),
                            signal_count: 1,
                            created_at: now.clone(),
                            updated_at: now,
                            active: true,
                            project_id: None,
                            scope_type: "system".to_string(),
                            scope_id: None,
                            effectiveness_score: meta.effectiveness_score,
                            stability: 1.0,
                            scope_repo_id: None,
                            last_applied: None,
                            application_count: 0,
                            metadata: None,
                        };
                        if let Err(e) = rule_repo.upsert(&procedural).await {
                            tracing::warn!("mirror: failed to promote meta-rule {rule_id} to procedural rule: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "mirror: find_similar failed during meta-rule promotion: {e}"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Dismiss a pending meta-rule, transitioning it to [`MetaRuleStatus::Disabled`].
    pub async fn dismiss_meta_rule(&self, rule_id: Uuid) -> Result<()> {
        self.repo
            .update_meta_rule_status(rule_id, MetaRuleStatus::Disabled)
            .await
    }

    /// Return active and pending meta-rules as `(active, pending)`.
    pub async fn get_meta_rules(&self) -> Result<(Vec<MetaRule>, Vec<MetaRule>)> {
        let (active, pending) = tokio::try_join!(
            self.repo.get_meta_rules_by_status(MetaRuleStatus::Active),
            self.repo.get_meta_rules_by_status(MetaRuleStatus::Pending),
        )?;
        Ok((active, pending))
    }

    /// Create a user-authored meta-rule from free-form text.
    /// The rule starts in Pending status with default effectiveness.
    pub async fn create_meta_rule_from_text(&self, text: String) -> Result<MetaRule> {
        let rule = MetaRule {
            id: Uuid::new_v4(),
            trigger_condition: text,
            action: MetaRuleAction::SurfaceInsight {
                message: String::new(),
            },
            source: MetaRuleSource::UserCreated,
            effectiveness_score: 0.5,
            status: MetaRuleStatus::Pending,
            signal_count: 0,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
        };
        self.repo.insert_meta_rule(&rule).await?;
        Ok(rule)
    }

    // -----------------------------------------------------------------------
    // Trial preview actions
    // -----------------------------------------------------------------------

    pub async fn kill_trial(&self, trial_id: &str) -> Result<()> {
        if let Some(timers) = &self.active_timers {
            if let Some((_, handle)) = timers.remove(trial_id) {
                handle.abort();
            }
        }

        self.write_episodic(format!("Killed experiment trial {trial_id}"), None, 0.7);

        // MirrorTrialKilled event removed; no longer publishing domain event

        Ok(())
    }

    pub async fn continue_trial(&self, trial_id: &str) -> Result<()> {
        if let Some(timers) = &self.active_timers {
            timers.remove(trial_id);
        }
        Ok(())
    }

    pub async fn get_trial_previews(&self) -> Result<Vec<TrialPreview>> {
        self.repo.get_recent_trial_previews().await
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

        let period_end = Timestamp::now();
        let period_start = period_end - jiff::SignedDuration::from_secs(7 * 86400);

        let snapshots = self.repo.get_routing_history(7).await?;

        let ctx = build_narrative_context((period_start, period_end), snapshots);

        let generated = handler.generate_narrative(ctx).await?;

        let narrative = TrendNarrative {
            id: Uuid::new_v4(),
            generated_at: Timestamp::now(),
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

        self.write_episodic(
            format!("Weekly reflection: {}", narrative.full_narrative),
            Some(narrative.routing_summary.clone()),
            0.9,
        );

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
        period: Option<(Timestamp, Timestamp)>,
    ) -> Result<MirrorResponse> {
        let handler = self.require_handler()?;

        let (period_start, period_end) = period.unwrap_or_else(|| {
            let end = Timestamp::now();
            let start = end - jiff::SignedDuration::from_secs(14 * 86400);
            (start, end)
        });

        let days = ((period_end.as_millisecond() - period_start.as_millisecond()) / 86_400_000)
            .max(1) as u32;
        let snapshots = self.repo.get_routing_history(days).await?;

        let ctx = build_narrative_context((period_start, period_end), snapshots);

        let answer = handler.generate_mirror_response(&query, ctx).await?;

        Ok(MirrorResponse {
            answer,
            data_sources_used: vec!["routing_snapshots".to_string()],
            proposed_meta_rule: None,
        })
    }

    // -----------------------------------------------------------------------
    // Voice echo — semantic snippet lookup
    // -----------------------------------------------------------------------

    /// Find the most semantically relevant recent snippet for a voice partial transcript.
    ///
    /// Uses on-the-fly embedding similarity (cosine) against recent undismissed snippets.
    /// Returns `None` if no text embedder is available or no snippet scores above threshold.
    pub async fn get_recent_voice_relevant_snippet(&self, query: &str) -> Option<String> {
        let embedder = self.text_embedder.as_ref()?;

        let snippets = self.repo.get_pending_snippets().await.ok()?;
        if snippets.is_empty() {
            return None;
        }

        let query_embedding = embedder.embed(query).await.ok()?;

        let mut best_score = 0.0f64;
        let mut best_text: Option<String> = None;

        for snippet in &snippets {
            let snippet_text = format!("{} {}", snippet.headline, snippet.body);
            if let Ok(snippet_embedding) = embedder.embed(&snippet_text).await {
                let score =
                    common::helpers::cosine_similarity(&query_embedding, &snippet_embedding);
                if score > best_score {
                    best_score = score;
                    best_text = Some(snippet.headline.clone());
                }
            }
        }

        if best_score >= 0.45 {
            best_text
        } else {
            None
        }
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

    /// Fire-and-forget write of an episodic memory with `domain = "mirror"`.
    fn write_episodic(&self, content: String, summary: Option<String>, importance: f64) {
        if let Some(ref episodic) = self.episodic_repo {
            let repo = episodic.clone();
            let now = Timestamp::now().to_string();
            tokio::spawn(async move {
                let mem = cognitive_memory::types::EpisodicMemory {
                    id: Uuid::new_v4().to_string(),
                    domain: "mirror".to_string(),
                    content,
                    summary,
                    importance,
                    occurred_at: now.clone(),
                    recorded_at: now,
                    stability: 1.0,
                    last_accessed: None,
                    access_count: 0,
                    project_id: None,
                    scope_type: "system".to_string(),
                    scope_id: None,
                    kind: None,
                    scope_repo_id: None,
                    metadata: None,
                    actor_id: None,
                    tier: "raw".to_string(),
                    parent_id: None,
                    child_count: 0,
                    rolled_up_at: None,
                };
                if let Err(e) = repo.insert(&mem).await {
                    tracing::warn!("mirror: failed to write episodic memory: {e}");
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Meta-rule helpers
// ---------------------------------------------------------------------------

/// Convert a [`MetaRule`] into a plain-English rule text suitable for a [`ProceduralRule`].
fn meta_rule_to_rule_text(meta: &MetaRule) -> String {
    let action_desc = match &meta.action {
        MetaRuleAction::AdjustRouting { skill, direction } => {
            format!("route more toward the {skill} skill ({direction})")
        }
        MetaRuleAction::ForceClarification => "ask for clarification before responding".to_string(),
        MetaRuleAction::SwitchMode { mode } => {
            format!("switch to {mode} mode")
        }
        MetaRuleAction::CreateExperiment { hypothesis } => {
            format!("create an experiment to test: {hypothesis}")
        }
        MetaRuleAction::SurfaceInsight { message } => {
            if message.is_empty() {
                "surface a relevant insight".to_string()
            } else {
                format!("surface insight: {message}")
            }
        }
        MetaRuleAction::Custom { payload } => {
            format!("apply custom behavior: {payload}")
        }
    };
    format!("When {}, {}.", meta.trigger_condition, action_desc)
}

// ---------------------------------------------------------------------------
// Context assembly helpers
// ---------------------------------------------------------------------------

fn build_narrative_context(
    period: (Timestamp, Timestamp),
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
// Skill proposal management — KCA Track 12
// ---------------------------------------------------------------------------

/// A row from the `skill_proposals` table for listing pending proposals.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SkillProposalRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub avg_confidence: f64,
    pub proposed_at: String,
}

impl MirrorFacade {
    /// KCA Track 12 — approve a skill proposal, write its SKILL.md to disk, mark approved.
    pub async fn approve_skill_proposal(
        &self,
        proposal_id: &str,
        skills_dir: &std::path::Path,
    ) -> common::Result<std::path::PathBuf> {
        // Atomic UPDATE — avoids SELECT-then-UPDATE race.
        let update = sqlx::query(
            "UPDATE skill_proposals SET status = 'approved', decided_at = datetime('now'), decided_by = 'user' WHERE id = ?1 AND status = 'pending'",
        )
        .bind(proposal_id)
        .execute(self.repo.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        if update.rows_affected() == 0 {
            return Err(common::KlyntbotError::Storage(format!(
                "proposal {proposal_id} not found or not pending"
            )));
        }

        let row: (String, String, String) = sqlx::query_as(
            "SELECT name, yaml_frontmatter, body_markdown FROM skill_proposals WHERE id = ?1",
        )
        .bind(proposal_id)
        .fetch_one(self.repo.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let dir = skills_dir.join(&row.0);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join("SKILL.md");
        let content = format!("{}\n\n{}", row.1, row.2);
        tokio::fs::write(&path, content).await?;

        Ok(path)
    }

    /// Reject a skill proposal.
    pub async fn reject_skill_proposal(
        &self,
        proposal_id: &str,
        _reason: Option<&str>,
    ) -> common::Result<()> {
        sqlx::query(
            "UPDATE skill_proposals SET status = 'rejected', decided_at = datetime('now'), decided_by = 'user' WHERE id = ?1",
        )
        .bind(proposal_id)
        .execute(self.repo.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    /// List pending skill proposals ordered by proposed_at DESC.
    pub async fn list_pending_skill_proposals(
        &self,
        limit: usize,
    ) -> common::Result<Vec<SkillProposalRow>> {
        let lim = limit as i64;
        sqlx::query_as::<_, SkillProposalRow>(
            "SELECT id, name, description, avg_confidence, proposed_at FROM skill_proposals WHERE status = 'pending' ORDER BY proposed_at DESC LIMIT ?1",
        )
        .bind(lim)
        .fetch_all(self.repo.db())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{
        MetaRule, MetaRuleAction, MetaRuleSource, MetaRuleStatus, MirrorAlertType, SkillRouteStats,
    };

    async fn setup() -> MirrorFacade {
        MirrorFacade::new(crate::test_mirror_repo().await)
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
            trigger_condition: "user corrects tasks twice".to_string(),
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
        let snippet = crate::NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Timestamp::now(),
            alert_type: MirrorAlertType::RoutingDrift,
            headline: "Test snippet".to_string(),
            body: "Body text".to_string(),
            suggested_action: None,
            user_feedback: None,
            dismissed_at: None,
            coding_alert_kind: None,
            coding_alert_severity: None,
        };
        facade.repo.insert_snippet(&snippet).await.unwrap();

        let snippets = facade.get_pending_snippets().await.unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].id, snippet.id);
    }

    // -- Brain version tests -----------------------------------------------

    fn make_brain_version(version: u32, parent: Option<u32>) -> crate::BrainVersion {
        crate::BrainVersion {
            version,
            trial_id: None,
            promoted_at: Timestamp::now(),
            params: serde_json::json!({"temperature": 0.7}),
            reason: format!("v{version}"),
            parent_version: parent,
            metrics_at_promotion: serde_json::json!({}),
            reverted: false,
        }
    }

    #[tokio::test]
    async fn test_get_brain_versions() {
        let facade = setup().await;
        facade
            .repo
            .insert_brain_version(&make_brain_version(1, None))
            .await
            .unwrap();
        facade
            .repo
            .insert_brain_version(&make_brain_version(2, Some(1)))
            .await
            .unwrap();
        facade
            .repo
            .insert_brain_version(&make_brain_version(3, Some(2)))
            .await
            .unwrap();

        let versions = facade.get_brain_versions().await.unwrap();
        assert_eq!(versions.len(), 3);
        // DESC order
        assert_eq!(versions[0].version, 3);
        assert_eq!(versions[1].version, 2);
        assert_eq!(versions[2].version, 1);
    }

    #[tokio::test]
    async fn test_revert_to_version() {
        let facade = setup().await;
        facade
            .repo
            .insert_brain_version(&make_brain_version(1, None))
            .await
            .unwrap();
        facade
            .repo
            .insert_brain_version(&make_brain_version(2, Some(1)))
            .await
            .unwrap();

        let new_v = facade.revert_to_version_db_only(1).await.unwrap();
        assert_eq!(new_v.version, 3);
        assert_eq!(new_v.params, serde_json::json!({"temperature": 0.7}));
        assert_eq!(new_v.reason, "Reverted to v1");
        assert_eq!(new_v.parent_version, Some(2));

        // v2 should be marked reverted
        let v2 = facade.repo.get_brain_version(2).await.unwrap().unwrap();
        assert!(v2.reverted);

        // v1 should NOT be marked reverted
        let v1 = facade.repo.get_brain_version(1).await.unwrap().unwrap();
        assert!(!v1.reverted);
    }

    #[tokio::test]
    async fn test_get_state_includes_brain_version() {
        let facade = setup().await;
        facade
            .repo
            .insert_brain_version(&make_brain_version(1, None))
            .await
            .unwrap();

        let state = facade.get_state().await.unwrap();
        assert!(state.latest_brain_version.is_some());
        assert_eq!(state.latest_brain_version.unwrap().version, 1);
    }

    #[tokio::test]
    async fn approve_skill_proposal_writes_skill_to_disk_and_marks_proposal_approved() {
        let facade = setup().await;
        let tmpdir = tempfile::tempdir().unwrap();

        // Insert proposal.
        let id = "p1";
        sqlx::query(
            r#"INSERT INTO skill_proposals (id, name, description, yaml_frontmatter, body_markdown, source_rule_ids, avg_confidence, status)
               VALUES (?1, 'rust-test-then-lint', 'Test + lint Rust', '---
name: rust-test-then-lint
---', '# Body
', '[]', 0.85, 'pending')"#,
        ).bind(id)
        .execute(facade.repo.db()).await.unwrap();

        let path = facade
            .approve_skill_proposal(id, tmpdir.path())
            .await
            .unwrap();
        assert!(path.exists(), "SKILL.md should be written");

        // Status should be approved.
        let row: (String,) = sqlx::query_as("SELECT status FROM skill_proposals WHERE id = ?1")
            .bind(id)
            .fetch_one(facade.repo.db())
            .await
            .unwrap();
        assert_eq!(row.0, "approved");
    }
}
