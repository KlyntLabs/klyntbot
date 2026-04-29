//! Session-end light Reforge pass.
//!
//! Triggered by `EventKind::SessionEnd`. Runs in < 2 s with zero LLM calls.
//! Three responsibilities:
//!
//! 1. **Hebbian bump** — for every co-retrieved pair this session, increment
//!    the `co_activation` counter via `CoActivationRepo::increment_pair`.
//! 2. **Within-session dedup** — collapse `episodic_memories{kind='fix_attempt'}`
//!    rows sharing a `problem_hash` to one survivor (highest importance) and
//!    write a derived "attempts_count" metadata field.
//! 3. **Session summary** — emit a deterministic ≤ 200-token markdown body to
//!    `session_summaries`. The Phase-4 SessionStart renderer reads this for
//!    its "Open threads" section.

use crate::causal::CausalEdgeDetector;
use crate::recall::telemetry::RecallInvocationRepo;
use crate::reforge::session_summary_repo::{SessionSummaryRepo, SessionSummaryRow};
use cognitive::repos::CommunityRepo;
use cognitive::repos::EntityRepo;
use cognitive::{CoActivationRepo, EpisodicMemoryRepo};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Outcome of one pass — surfaced to telemetry.
#[derive(Debug, Clone, Default)]
pub struct SessionEndReport {
    /// Number of co-activation pairs bumped.
    pub pairs_bumped: u32,
    /// Number of duplicate fix-attempt episodes removed.
    pub deduped_attempts: u32,
    /// Number of causal edges detected.
    pub causal_edges_detected: u32,
    /// Token count of the summary.
    pub summary_tokens: u32,
}

/// Session-end pass.
#[derive(Debug, Clone)]
pub struct SessionEndPass {
    summaries: SessionSummaryRepo,
    co_activation: CoActivationRepo,
    utilization: RecallInvocationRepo,
    causal_detector: Option<Arc<CausalEdgeDetector>>,
    entity_repo: Option<EntityRepo>,
    community_repo: Option<CommunityRepo>,
    community_handler:
        Option<Arc<dyn cognitive::services::community_membership_online::AsyncConfirmFn>>,
}

impl SessionEndPass {
    /// Construct.
    pub fn new(
        summaries: SessionSummaryRepo,
        co_activation: CoActivationRepo,
        utilization: RecallInvocationRepo,
    ) -> Self {
        Self {
            summaries,
            co_activation,
            utilization,
            causal_detector: None,
            entity_repo: None,
            community_repo: None,
            community_handler: None,
        }
    }

    /// Attach entity repo for online community membership (KCA Track 11).
    #[must_use]
    pub fn with_entity_repo(mut self, repo: EntityRepo) -> Self {
        self.entity_repo = Some(repo);
        self
    }

    /// Attach community repo for online community membership (KCA Track 11).
    #[must_use]
    pub fn with_community_repo(mut self, repo: CommunityRepo) -> Self {
        self.community_repo = Some(repo);
        self
    }

    /// Attach community membership handler for online community membership (KCA Track 11).
    #[must_use]
    pub fn with_community_handler(
        mut self,
        handler: Arc<dyn cognitive::services::community_membership_online::AsyncConfirmFn>,
    ) -> Self {
        self.community_handler = Some(handler);
        self
    }

    /// Attach an optional causal-edge detector.
    #[must_use]
    pub fn with_causal_detector(mut self, detector: Arc<CausalEdgeDetector>) -> Self {
        self.causal_detector = Some(detector);
        self
    }

    /// Run the pass.
    pub async fn run(&self, session_id: &str, repo_id: Option<&str>) -> Result<SessionEndReport> {
        let mut report = SessionEndReport::default();

        // 1. Hebbian bump — pull retrieval rows for this session and bump pairs.
        let invocations = self
            .utilization
            .list_for_session(session_id, 200)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("recall_invocations: {e}")))?;
        let mut all_ids: Vec<String> = invocations
            .iter()
            .flat_map(|inv| inv.result_ids.iter().map(|u: &uuid::Uuid| u.to_string()))
            .collect();
        all_ids.sort();
        all_ids.dedup();
        for i in 0..all_ids.len() {
            for j in (i + 1)..all_ids.len() {
                let (lo, hi) = if all_ids[i] < all_ids[j] {
                    (&all_ids[i], &all_ids[j])
                } else {
                    (&all_ids[j], &all_ids[i])
                };
                self.co_activation
                    .increment_pair(lo, hi)
                    .await
                    .map_err(|e| KlyntbotError::Storage(format!("co_activation: {e}")))?;
                report.pairs_bumped += 1;
            }
        }

        // 2. Within-session dedup of fix-attempts by problem_hash.
        let pool = self.summaries.pool().clone();
        let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());
        let rows: Vec<(String, String, Option<String>, f32)> = sqlx::query_as(
            "SELECT id, content, metadata, importance \
             FROM episodic_memories \
             WHERE scope_id = ?1 AND kind = 'fix_attempt'",
        )
        .bind(session_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("dedup query: {e}")))?;

        let mut buckets: HashMap<String, Vec<(String, f32)>> = HashMap::new();
        for (id, _content, metadata, importance) in rows {
            let hash = metadata
                .as_deref()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .and_then(|v| {
                    v.get("problem_hash")
                        .and_then(|h| h.as_str().map(String::from))
                });
            let Some(hash) = hash else { continue };
            buckets.entry(hash).or_default().push((id, importance));
        }

        for (_, mut group) in buckets {
            if group.len() < 2 {
                continue;
            }
            group.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let survivor = group.remove(0).0;
            // Annotate survivor with attempts_count and delete the rest.
            let count = (group.len() + 1) as u32;
            let extra = group.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
            sqlx::query(
                "UPDATE episodic_memories \
                 SET metadata = json_patch(COALESCE(metadata,'{}'), \
                     json_object('attempts_count', ?1, 'merged_ids', json(?2))) \
                 WHERE id = ?3",
            )
            .bind(count as i64)
            .bind(serde_json::to_string(&extra).unwrap_or_else(|_| "[]".into()))
            .bind(&survivor)
            .execute(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("dedup update: {e}")))?;
            for (id, _) in &group {
                ep_repo
                    .delete_by_id(id)
                    .await
                    .map_err(|e| KlyntbotError::Storage(format!("dedup delete: {e}")))?;
                report.deduped_attempts += 1;
            }
        }

        // 3. Causal-edge detection.
        if let Some(detector) = &self.causal_detector {
            let n = detector
                .detect_for_session(session_id)
                .await
                .map_err(|e| KlyntbotError::Storage(format!("causal detection: {e}")))?;
            report.causal_edges_detected = n;
        }

        // 4. Stale-candidate resolution — mark low-stability facts as superseded.
        let stale_count = resolve_stale_candidates(&ep_repo, repo_id).await?;
        report.deduped_attempts += stale_count; // reuse counter for now

        // 5. Online community membership (KCA Track 11).
        if let (Some(entity_repo), Some(community_repo), Some(handler)) = (
            &self.entity_repo,
            &self.community_repo,
            &self.community_handler,
        ) {
            let touched_ids = collect_touched_entity_ids(entity_repo, &all_ids).await;
            if !touched_ids.is_empty() {
                cognitive::services::community_membership_online::run_for_session(
                    entity_repo,
                    community_repo,
                    touched_ids,
                    handler.clone(),
                )
                .await;
            }
        }

        // 6. Build deterministic ≤200-token summary.
        let summary = build_summary_md(session_id, repo_id, &invocations).await;
        let token_count = estimate_tokens(&summary);
        report.summary_tokens = token_count;

        let row = SessionSummaryRow {
            id: SessionSummaryRepo::new_row_id(),
            session_id: session_id.to_string(),
            repo_id: repo_id.map(String::from),
            summarised_at: Timestamp::now(),
            summary_md: summary,
            token_count,
        };
        self.summaries.insert(&row).await?;

        Ok(report)
    }
}

async fn build_summary_md(
    session_id: &str,
    repo_id: Option<&str>,
    invocations: &[crate::recall::telemetry::RecallInvocationRow],
) -> String {
    let mut out = String::new();
    out.push_str("## Session summary\n");
    out.push_str(&format!("- Session: {session_id}\n"));
    if let Some(repo) = repo_id {
        out.push_str(&format!("- Repo: {repo}\n"));
    }
    out.push_str(&format!("- Recall invocations: {}\n", invocations.len()));
    out.push_str("\n_Summary auto-generated by SessionEndPass._\n");
    if estimate_tokens(&out) > 200 {
        truncate_to_tokens(&out, 200)
    } else {
        out
    }
}

/// Resolve stale candidates: facts with stability < 0.2 and no access in 30 days.
async fn resolve_stale_candidates(
    ep_repo: &EpisodicMemoryRepo,
    repo_id: Option<&str>,
) -> Result<u32> {
    let pool = ep_repo.pool().clone();
    let cutoff = Timestamp::now()
        .checked_sub(jiff::ToSpan::days(30))
        .unwrap_or(Timestamp::MIN)
        .to_string();
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM episodic_memories \
         WHERE (scope_repo_id = ?1 OR (?1 IS NULL AND scope_repo_id IS NULL)) \
         AND stability < 0.2 AND COALESCE(last_accessed, '1970-01-01') < ?2",
    )
    .bind(repo_id)
    .bind(&cutoff)
    .fetch_all(&pool)
    .await
    .map_err(|e| KlyntbotError::Storage(format!("stale query: {e}")))?;
    let mut count = 0;
    for (id,) in rows {
        if let Err(e) = ep_repo.delete_by_id(&id).await {
            tracing::warn!(id, error = %e, "stale candidate delete failed");
        } else {
            count += 1;
        }
    }
    Ok(count)
}

/// Collect entity IDs touched by recalled facts in this session (KCA Track 11).
async fn collect_touched_entity_ids(entity_repo: &EntityRepo, fact_ids: &[String]) -> Vec<String> {
    if fact_ids.is_empty() {
        return Vec::new();
    }
    let fact_repo = cognitive::repos::SemanticFactRepo::new(entity_repo.pool().clone());
    let refs: Vec<&str> = fact_ids.iter().map(|s| s.as_str()).collect();
    let facts = match fact_repo.get_batch(&refs).await {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e, "collect_touched_entity_ids: get_batch failed");
            return Vec::new();
        }
    };

    let mut names: std::collections::HashSet<String> = Default::default();
    for f in &facts {
        names.insert(f.subject.clone());
        names.insert(f.object.clone());
    }

    let mut entity_ids: std::collections::HashSet<String> = Default::default();
    for name in names {
        match entity_repo.find_by_name(&name).await {
            Ok(rows) => {
                for row in rows {
                    entity_ids.insert(row.id);
                }
            }
            Err(e) => tracing::warn!(error = %e, name = %name, "find_by_name failed"),
        }
    }
    entity_ids.into_iter().collect()
}

fn estimate_tokens(s: &str) -> u32 {
    // Cheap heuristic — chars / 4 (matches `HeuristicBudgeter` in Phase 4).
    ((s.chars().count() as f32 / 4.0).ceil()) as u32
}

fn truncate_to_tokens(s: &str, budget: u32) -> String {
    let max_chars = (budget as usize) * 4;
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect()
}
