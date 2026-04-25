//! Markdown renderers for passive injection.
//!
//! Both renderers are total-budget-bounded — they truncate section by section,
//! then global-truncate at the end. The token counter is pluggable via
//! `CodingRecallService::budgeter` (currently held inside the service via
//! the `IndexBuilder`).

use crate::recall::budget::{default_budgeter, HeuristicBudgeter, TokenBudgeter};
use crate::recall::{CodingRecallService, RecallQuery};
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
    let header = format!(
        "## Project memory — {}\n\n",
        repo.unwrap_or("(no repo)")
    );

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
    let mut s3 = String::from("### Recent activity (last 7 days)\n| when | what | id |\n|---|---|---|\n");
    for e in recent.iter().take(8) {
        s3.push_str(&format!(
            "| {} | {} | `{}` |\n",
            e.when.to_string(),
            crop(&e.snippet, 60),
            short_id(&e.id.to_string())
        ));
    }
    s3.push('\n');

    // Section 4 — open threads. Phase 4 stub: empty list with caveat.
    let s4 = "### Open threads\n_(none captured this phase)_\n\n";

    // Concatenate + global truncate.
    let full = format!("{header}{s1}{s2}{s3}{s4}*Call `recall_fetch(ids=[...])` for details.*\n");
    let truncated = budgeter.truncate_to(&full, SESSION_START_BUDGET_TOKENS as usize);
    debug_assert!(
        HeuristicBudgeter.count(&truncated) <= SESSION_START_BUDGET_TOKENS as usize + 50,
        "render_session_start_block exceeded budget"
    );
    Ok(truncated)
}

/// Render the UserPromptSubmit injection block.
pub async fn render_user_prompt_block(
    svc: &Arc<CodingRecallService>,
    query: &str,
    repo: Option<&str>,
) -> common::Result<String> {
    let budgeter: Arc<dyn TokenBudgeter> = default_budgeter();

    // Dead-end check first — placed at top if matches found.
    let dead_ends = svc.check_dead_ends(query, repo).await?;
    let warn = if dead_ends.aggregate_confidence > 0.5 && !dead_ends.matches.is_empty() {
        let m = &dead_ends.matches[0];
        format!(
            "### ⚠️ Heads-up\nYou previously tried **{}** ({}) — abandoned because {}.\n\n",
            m.approach, m.when, m.reason
        )
    } else {
        String::new()
    };

    // Likely-relevant memories.
    let idx = svc.recall_index(query, repo, None, None, 6).await?;
    let mut likely = String::from("### Likely relevant\n");
    for r in idx.results.iter().take(6) {
        likely.push_str(&format!("- [`{}`] {} {}\n", short_id(&r.id.to_string()), r.kind, crop(&r.title, 80)));
    }
    likely.push('\n');

    // Causal context — empty until Phase 6, but stub the section.
    let causal = "### Causal context\n_(populated when causal edges are seeded — Phase 6.)_\n\n";

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

    let full = format!(
        "## Relevant memory for this turn\n\n{warn}{likely}{causal}{footer}\n"
    );
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
