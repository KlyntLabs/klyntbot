//! Markdown renderers for passive injection.
//!
//! Both renderers are total-budget-bounded — they truncate section by section,
//! then global-truncate at the end. The token counter is pluggable via
//! `CodingRecallService::budgeter` (currently held inside the service via
//! the `IndexBuilder`).

use crate::recall::budget::{default_budgeter, HeuristicBudgeter, TokenBudgeter};
use crate::recall::{CodingRecallService, RecallQuery};
use cognitive::repos::entity::{EdgeType, EntityRepo};
use std::fmt::Write;
use std::sync::Arc;

/// Token budget for SessionStart injection (design §8).
pub const SESSION_START_BUDGET_TOKENS: u32 = 800;
/// Token budget for UserPromptSubmit injection (design §8).
pub const USER_PROMPT_BUDGET_TOKENS: u32 = 1500;

/// Render the SessionStart injection block for a given repo.
///
/// Sections in order:
/// 1. `## Project memory — <repo_id>`
/// 2. `### What you need to know about this repo` (RepoContext, top 6)
/// 3. `### Your preferences (relevant)` (StylePreference)
/// 4. `### Recent activity (last 7 days)` (table)
/// 5. `### Open threads` (last unfinished turn traces)
pub async fn render_session_start_block(
    svc: &Arc<CodingRecallService>,
    repo: Option<&str>,
) -> common::Result<String> {
    let budgeter: Arc<dyn TokenBudgeter> = default_budgeter();
    let header = format!("## Project memory — {}\n\n", repo.unwrap_or("(no repo)"));

    // Section 1 — repo context facts.
    let repo_ctx = svc
        .recall_index("repository architecture overview", repo, None, None, 6)
        .await?;
    let mut s1 = String::from("### What you need to know about this repo\n");
    for r in repo_ctx.results.iter().take(6) {
        s1.push_str(&format!("- {}\n", r.title));
    }
    s1.push('\n');

    // Section 2 — preferences.
    let prefs = svc
        .recall_index("style preference convention", repo, None, None, 4)
        .await?;
    let mut s2 = String::from("### Your preferences (relevant)\n");
    for r in prefs.results.iter().take(4) {
        s2.push_str(&format!("- {}\n", r.title));
    }
    s2.push('\n');

    // Section 3 — recent activity (last 7 days).
    let recent = svc
        .recall_timeline(RecallQuery::Text("recent".into()), repo, 7)
        .await?;
    let mut s3 =
        String::from("### Recent activity (last 7 days)\n| when | what | id |\n|---|---|---|\n");
    for e in recent.iter().take(8) {
        s3.push_str(&format!(
            "| {} | {} | `{}` |\n",
            e.when,
            crop(&e.snippet, 60),
            short_id(&e.id.to_string())
        ));
    }
    s3.push('\n');

    // Section 4 — open threads.
    let threads = svc.open_threads(repo, 7, 5).await.unwrap_or_default();
    let mut s4 = String::from("### Open threads\n");
    if threads.is_empty() {
        s4.push_str("_(none captured this phase)_\n");
    } else {
        for t in threads {
            s4.push_str(&format!(
                "- `{}` {}: {}\n",
                short_id(&t.episode_id),
                t.when,
                crop(&t.last_user_prompt, 80)
            ));
        }
    }
    s4.push('\n');

    // Concatenate + global truncate.
    let full = format!("{header}{s1}{s2}{s3}{s4}*Call `recall_fetch(ids=[...])` for details.*\n");
    let truncated = budgeter.truncate_to(&full, SESSION_START_BUDGET_TOKENS as usize);
    debug_assert!(
        HeuristicBudgeter.count(&truncated) <= SESSION_START_BUDGET_TOKENS as usize + 50,
        "render_session_start_block exceeded budget"
    );
    Ok(truncated)
}

/// Render causal + related context from the entity graph, grouped by edge type.
///
/// Causal edges are shown under `### Causal Context`; everything else
/// (structural, temporal, correlational) under `### Related Context`.
pub async fn render_causal_context(
    entity_repo: &EntityRepo,
    seed_names: &[&str],
) -> common::Result<String> {
    let mut causal = Vec::new();
    let mut other = Vec::new();

    for name in seed_names {
        let entities = entity_repo.find_by_name(name).await?;
        for node in &entities {
            let edges = entity_repo
                .get_neighborhood_with_edges(&node.id, 1)
                .await
                .unwrap_or_default();
            for edge in edges {
                let line = format!(
                    "- {} —[{}]→ {}",
                    node.name, edge.relationship_type, edge.neighbor.name
                );
                if edge.edge_type == EdgeType::Causal {
                    causal.push(line);
                } else {
                    other.push(line);
                }
            }
        }
    }

    let mut s = String::new();
    if !causal.is_empty() {
        s.push_str("### Causal Context\n");
        for c in causal {
            let _ = writeln!(s, "{c}");
        }
    }
    if !other.is_empty() {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str("### Related Context\n");
        for c in other {
            let _ = writeln!(s, "{c}");
        }
    }
    Ok(s)
}

/// Render the UserPromptSubmit injection block.
pub async fn render_user_prompt_block(
    svc: &Arc<CodingRecallService>,
    query: &str,
    repo: Option<&str>,
) -> common::Result<String> {
    let budgeter: Arc<dyn TokenBudgeter> = default_budgeter();

    // QueryPipeline enrichment — if configured, run the prompt through PRF + expansion.
    let enhanced_query = if let Some(ref pipeline) = svc.query_pipeline() {
        let ctx = context_engine::RetrievalContext {
            active_skill: None,
            active_task: None,
            recent_user_messages: vec![query.to_string()],
            situation: None,
            active_view: None,
            recent_correction: None,
        };
        let budget = context_engine::EnhancementBudget::normal();
        let out = pipeline.enhance(query, &ctx, &budget).await;
        out.query.primary
    } else {
        query.to_string()
    };

    // Dead-end check first — placed at top if matches found.
    let dead_ends = svc.check_dead_ends(&enhanced_query, repo).await?;
    let warn = if dead_ends.aggregate_confidence > 0.7 && !dead_ends.matches.is_empty() {
        let m = &dead_ends.matches[0];
        format!(
            "### ⚠️ Heads-up\nYou previously tried **{}** ({}) — abandoned because {}.\n\n",
            m.approach, m.when, m.reason
        )
    } else {
        String::new()
    };

    // Likely-relevant memories.
    let idx = svc
        .recall_index(&enhanced_query, repo, None, None, 6)
        .await?;
    let mut likely = String::from("### Likely relevant\n");
    for r in idx.results.iter().take(6) {
        likely.push_str(&format!(
            "- [`{}`] {} {}\n",
            short_id(&r.id.to_string()),
            r.kind,
            crop(&r.title, 80)
        ));
    }
    likely.push('\n');

    // Causal / related context from the entity graph.
    let mut causal = String::new();
    let seed_names: Vec<&str> = idx
        .results
        .iter()
        .take(3)
        .map(|r| r.title.as_str())
        .collect();
    if let Some(entity_repo) = svc.entity_repo() {
        match render_causal_context(entity_repo, &seed_names).await {
            Ok(ctx) if !ctx.is_empty() => {
                causal.push_str(&ctx);
                causal.push('\n');
            }
            Ok(_) => {}
            Err(e) => {
                tracing::debug!(error = %e, "render_causal_context failed in render_user_prompt_block");
            }
        }
    }

    let footer = if !idx.results.is_empty() {
        format!(
            "*Fetch details: `recall_fetch(ids=[{}])`*",
            idx.results
                .iter()
                .take(3)
                .map(|r| format!("\"{}\"", r.id))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        String::new()
    };

    let full = format!("## Relevant memory for this turn\n\n{warn}{likely}{causal}{footer}\n");
    Ok(budgeter.truncate_to(&full, USER_PROMPT_BUDGET_TOKENS as usize))
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn crop(s: &str, n: usize) -> String {
    let mut out: String = s.chars().take(n).collect();
    if out.len() < s.len() {
        out.push('…');
    }
    out
}
