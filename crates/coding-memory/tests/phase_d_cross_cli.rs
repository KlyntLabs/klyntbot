//! KCA Phase D — full cross-CLI fixture: ClaudeCode rule, Codex episodics → promotion.

use coding_memory::reforge::cross_cli_synthesis::*;
use cognitive::repos::episodic_memory::EpisodicMemoryRepo;
use cognitive::repos::procedural_rule::ProceduralRuleRepo;
use cognitive::types::{EpisodicMemory, ProceduralRule};
use storage::StoragePool;

#[tokio::test]
async fn full_fixture_promotes_rule_observed_cross_cli() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::repos::cognitive_migrations())
        .await
        .unwrap();
    let rule_repo = ProceduralRuleRepo::new(pool.inner().clone());
    let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());

    // ClaudeCode rule.
    rule_repo
        .upsert(&ProceduralRule {
            id: "r_cc".into(),
            domain: "coding".into(),
            rule_text: "After cargo nextest passes, run clippy".into(),
            confidence: 0.9,
            signal_count: 8,
            source: "reflected".into(),
            active: true,
            ..Default::default()
        })
        .await
        .unwrap();
    rule_repo
        .set_observed_sources("r_cc", &["ClaudeCode"])
        .await
        .unwrap();

    // 6 Codex episodics matching the pattern.
    for i in 0..6 {
        ep_repo
            .insert(&EpisodicMemory {
                id: format!("ep_codex_{i}"),
                domain: "coding".into(),
                content: format!("user ran cargo nextest then cargo clippy in codex {i}"),
                summary: Some("nextest then clippy".into()),
                importance: 0.7,
                stability: 1.0,
                tier: "raw".into(),
                actor_id: Some("codex".into()),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let cands = find_transferable_rules(&rule_repo, &ep_repo, 0.6)
        .await
        .unwrap();
    assert!(
        !cands.is_empty(),
        "transfer should detect cross-CLI support"
    );
    assert!(cands[0]
        .supporting_sources
        .iter()
        .any(|s| s.eq_ignore_ascii_case("Codex")));
}
