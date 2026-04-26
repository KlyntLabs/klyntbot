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

use crate::recall::telemetry::RecallInvocationRepo;
use crate::reforge::session_summary_repo::{SessionSummaryRepo, SessionSummaryRow};
use cognitive::{CoActivationRepo, EpisodicMemoryRepo};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde_json::Value;
use std::collections::HashMap;

/// Outcome of one pass — surfaced to telemetry.
#[derive(Debug, Clone, Default)]
pub struct SessionEndReport {
    /// Number of co-activation pairs bumped.
    pub pairs_bumped: u32,
    /// Number of duplicate fix-attempt episodes removed.
    pub deduped_attempts: u32,
    /// Token count of the summary.
    pub summary_tokens: u32,
}

/// Session-end pass.
#[derive(Debug, Clone)]
pub struct SessionEndPass {
    summaries: SessionSummaryRepo,
    co_activation: CoActivationRepo,
    utilization: RecallInvocationRepo,
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
        }
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

        // 3. Build deterministic ≤200-token summary.
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
