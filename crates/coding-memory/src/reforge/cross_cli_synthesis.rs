//! KCA Track 10 — cross-CLI cognitive transfer.
//!
//! Reforge phase 2.6: detect rules observed in CLI source A that are also
//! supported (without yet being a rule) by signals in CLI source B. Propose
//! promotion to a source-agnostic rule when confidence ≥ 0.85.

use cognitive::repos::episodic_memory::EpisodicMemoryRepo;
use cognitive::repos::procedural_rule::ProceduralRuleRepo;

/// A rule that appears transferable from one CLI source to another.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TransferableCandidate {
    /// Rule ID in `procedural_rules`.
    pub rule_id: String,
    /// Text of the rule.
    pub rule_text: String,
    /// CLI sources that provided supporting episodic evidence.
    pub supporting_sources: Vec<String>,
    /// Total count of supporting episodics across all new sources.
    pub support_strength: u32,
}

/// Find rules observed only in one CLI source whose pattern is supported by
/// recent episodics from other sources.
pub async fn find_transferable_rules(
    rule_repo: &ProceduralRuleRepo,
    ep_repo: &EpisodicMemoryRepo,
    min_episodic_support: f64,
) -> common::Result<Vec<TransferableCandidate>> {
    let rules = rule_repo
        .list_all_active()
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
        .into_iter()
        .take(200)
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();

    for r in &rules {
        // Scope guard: only repo-agnostic rules transfer.
        if r.scope_repo_id.is_some() {
            continue;
        }
        if r.confidence < 0.7 {
            continue;
        }

        let observed = rule_repo
            .list_observed_sources(&r.id)
            .await
            .unwrap_or_default();
        if observed.is_empty() {
            continue;
        }

        // Heuristic: split rule_text into keywords, find episodics from OTHER sources matching ≥2 keywords.
        let kw = keywords_from(&r.rule_text);
        if kw.len() < 2 {
            continue;
        }

        let recent = ep_repo
            .list_recent(500)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let mut by_source: std::collections::HashMap<String, u32> = Default::default();
        for ep in &recent {
            // Skip episodics from already-observed sources.
            let actor = ep.actor_id.as_deref().unwrap_or("");
            let actor_norm = normalize_actor_to_source(actor);
            if observed.iter().any(|o| o.eq_ignore_ascii_case(&actor_norm)) {
                continue;
            }
            // Match: ≥2 keywords in episodic content.
            let combined = format!("{} {}", ep.content, ep.summary.as_deref().unwrap_or(""));
            let lower = combined.to_lowercase();
            let hits = kw.iter().filter(|k| lower.contains(*k)).count();
            if hits >= 2 {
                *by_source.entry(actor_norm).or_insert(0) += 1;
            }
        }

        let supporting: Vec<(String, u32)> = by_source
            .into_iter()
            .filter(|(_, n)| (*n as f64) / 5.0 >= min_episodic_support)
            .collect();
        if supporting.is_empty() {
            continue;
        }

        let total: u32 = supporting.iter().map(|(_, n)| n).sum();
        candidates.push(TransferableCandidate {
            rule_id: r.id.clone(),
            rule_text: r.rule_text.clone(),
            supporting_sources: supporting.iter().map(|(s, _)| s.clone()).collect(),
            support_strength: total,
        });
    }

    Ok(candidates)
}

fn keywords_from(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4 && !STOPWORDS.contains(w))
        .map(|w| w.to_string())
        .collect()
}

fn normalize_actor_to_source(actor: &str) -> String {
    match actor.to_lowercase().as_str() {
        "claude_code" | "claudecode" | "claude-code" => "ClaudeCode".into(),
        "codex" => "Codex".into(),
        "kimi" | "kimi_cli" | "kimi-cli" => "KimiCli".into(),
        "opencode" | "open_code" => "OpenCode".into(),
        _ => "Unknown".into(),
    }
}

const STOPWORDS: &[&str] = &[
    "with", "from", "this", "that", "have", "your", "their", "after", "before", "when", "then",
    "into", "user", "always", "would", "should",
];

#[cfg(test)]
mod tests {
    use super::*;
    use cognitive::types::{EpisodicMemory, ProceduralRule};
    use cognitive::EpisodicMemoryRepo;
    use cognitive::ProceduralRuleRepo;
    use storage::StoragePool;

    #[tokio::test]
    async fn find_transferable_rules_matches_pattern_across_sources() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
            .await
            .unwrap();
        StoragePool::run_feature_migrations(pool.inner(), &crate::coding_memory_migrations())
            .await
            .unwrap();
        let rule_repo = ProceduralRuleRepo::new(pool.inner().clone());

        // ClaudeCode-only rule.
        rule_repo
            .upsert(&ProceduralRule {
                id: "r_cc".into(),
                domain: "coding".into(),
                rule_text: "After cargo nextest passes, run clippy".into(),
                confidence: 0.85,
                signal_count: 6,
                source: "reflected".into(),
                active: true,
                ..Default::default()
            })
            .await
            .unwrap();
        // Tag rule as observed in ClaudeCode only (via metadata).
        rule_repo
            .set_observed_sources("r_cc", &["ClaudeCode"])
            .await
            .unwrap();

        // Add 4 episodics from Codex sessions matching the same pattern.
        let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());
        for i in 0..4 {
            ep_repo
                .insert(&EpisodicMemory {
                    id: format!("ep_codex_{i}"),
                    domain: "coding".into(),
                    content: format!(
                        "user ran cargo nextest then cargo clippy in codex session {i}"
                    ),
                    summary: Some("user ran nextest then clippy".into()),
                    importance: 0.7,
                    stability: 1.0,
                    tier: "raw".into(),
                    actor_id: Some("codex".into()),
                    ..Default::default()
                })
                .await
                .unwrap();
        }

        let candidates = find_transferable_rules(&rule_repo, &ep_repo, 0.7)
            .await
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].rule_id, "r_cc");
        assert!(candidates[0]
            .supporting_sources
            .contains(&"Codex".to_string()));
        assert!(candidates[0].support_strength >= 4);
    }
}
